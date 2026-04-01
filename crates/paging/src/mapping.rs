//! 仿射类型映射——move-only 的 VA→PA 映射所有权。
//!
//! [`MappedPages`] 同时持有虚拟页（[`AllocatedPages`]）和通过 PTE EXCLUSIVE 位
//! 追踪的物理帧所有权。Drop 时自动 unmap PTE、回收 EXCLUSIVE 帧、归还虚拟页。
//!
//! 永久映射使用 `ManuallyDrop<MappedPages>` 阻止 Drop。

use core::mem::ManuallyDrop;

use address::{FrameRange, PhysAddr, PhysPageNum, VirtAddr};
use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, UnmappedFrames};
use page_allocator::AllocatedPages;

use crate::error::{PagingError, UnmapResult};
use crate::{PageTable, PteFlags, PteFlagsOps, PteOps};

/// map_alloc 每次分配并映射的最大页数。
const MAP_CHUNK: usize = config::MAP_CHUNK_SIZE;

/// unmap_and_release 每次处理的最大页数。
const UNMAP_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 仿射类型映射——持有虚拟页所有权和映射关系。
///
/// 不可 Clone、不可 Copy。Drop 时 unmap PTE 并回收 EXCLUSIVE 帧，
/// 虚拟页归还 page_allocator。
///
/// SAS 架构下通过全局内核页表操作 PTE，不持有页表引用。
pub struct MappedPages {
    pages: AllocatedPages,
    flags: PteFlags,
}

impl MappedPages {
    /// 返回起始虚拟地址。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.pages.start_vaddr()
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.pages.size_in_bytes()
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.count()
    }

    /// 返回构造时的请求权限（不含 EXCLUSIVE）。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 返回持有的虚拟页引用。
    #[must_use]
    pub fn pages(&self) -> &AllocatedPages {
        &self.pages
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr() + offset).align_down();
        let guard = crate::kernel_page_table().lock();
        guard
            .get_mapping(page_va)
            .expect("MappedPages::pte_flags: 映射不存在")
            .1
    }

    /// 分配帧并建立映射——设置 EXCLUSIVE 位。
    ///
    /// 帧所有权通过 PTE 的 EXCLUSIVE 位追踪，Drop 时自动回收。
    pub fn map_alloc(pages: AllocatedPages, flags: PteFlags) -> Self {
        assert!(pages.count() > 0, "MappedPages::map_alloc: 页数不能为 0");
        let exclusive_flags = flags.with_exclusive();
        let page_count = pages.count();
        let va_start = pages.start_vaddr();
        let mut offset = 0;

        while offset < page_count {
            let n = (page_count - offset).min(MAP_CHUNK);
            let mut frames: heapless::Vec<AllocatedFrames, MAP_CHUNK> = heapless::Vec::new();
            for _ in 0..n {
                let frame =
                    AllocatedFrames::alloc_one().expect("map_alloc: 帧分配失败（物理内存耗尽）");
                frames
                    .push(frame)
                    .unwrap_or_else(|_| panic!("map_alloc: 帧数不超过 MAP_CHUNK"));
            }

            let pt = crate::kernel_page_table();
            let mut guard = pt.lock();
            for (i, frame) in frames.into_iter().enumerate() {
                let pa = frame.start_paddr();
                let va = va_start + (offset + i) * PAGE_SIZE;
                guard
                    .map_page(va, pa, exclusive_flags)
                    .expect("map_alloc: map_page 失败");
                let mapped = frame.into_mapped();
                core::mem::forget(mapped);
            }
            drop(guard);
            offset += n;
        }

        Self { pages, flags }
    }

    /// Identity-map 一段物理地址区间（VA == PA）——不设置 EXCLUSIVE 位。
    pub fn map_identity(pages: AllocatedPages, flags: PteFlags) -> Self {
        let pa_start = PhysAddr::new(pages.start_vaddr().as_usize());
        let pa_end = pa_start + pages.size_in_bytes();

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        guard.identity_map_range(pa_start, pa_end, flags);
        drop(guard);

        Self { pages, flags }
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用生命周期绑定到 `&self`——编译器保证映射 drop 后无法使用。
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证映射存活
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type_mut",
        );
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "MappedPages::as_type_mut: PTE 无 WRITE 权限"
        );
        // SAFETY: 偏移和对齐已验证，PTE 可写已验证，&mut self 保证独占
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 在 page_index 处拆分为两段，消费 self。
    ///
    /// PTE 不变——拆分是纯元数据操作。两个返回值各自负责 unmap 自己的页范围。
    pub fn split(self, page_index: usize) -> (MappedPages, MappedPages) {
        let flags = self.flags;
        let md = ManuallyDrop::new(self);
        // SAFETY: md 不会 Drop，我们手动拆分
        let pages = unsafe { core::ptr::read(&md.pages) };
        let (left_pages, right_pages) = pages.split(page_index);
        (
            MappedPages {
                pages: left_pages,
                flags,
            },
            MappedPages {
                pages: right_pages,
                flags,
            },
        )
    }

    /// 合并两个相邻映射，消费 other。
    ///
    /// 要求 flags 相同且虚拟地址连续。
    pub fn merge(self, other: MappedPages) -> Result<MappedPages, (MappedPages, MappedPages)> {
        if self.flags != other.flags {
            return Err((self, other));
        }
        let self_md = ManuallyDrop::new(self);
        let other_md = ManuallyDrop::new(other);
        let self_pages = unsafe { core::ptr::read(&self_md.pages) };
        let other_pages = unsafe { core::ptr::read(&other_md.pages) };
        let flags = self_md.flags;

        match self_pages.merge(other_pages) {
            Ok(merged) => Ok(MappedPages {
                pages: merged,
                flags,
            }),
            Err((sp, op)) => Err((
                MappedPages { pages: sp, flags },
                MappedPages {
                    pages: op,
                    flags: other_md.flags,
                },
            )),
        }
    }

    /// 修改映射权限——遍历 PTE 更新标志位，刷新 TLB。
    pub fn mprotect(&mut self, new_flags: PteFlags) {
        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            guard
                .update_flags(va, new_flags)
                .expect("mprotect: update_flags 失败");
        }
        drop(guard);
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        }
        self.flags = new_flags;
    }

    /// 手动解除映射并取回所有权——不释放资源。
    ///
    /// 返回虚拟页和可能的 EXCLUSIVE 物理帧，调用者自行决定是否重用或释放。
    pub fn unmap(self) -> (AllocatedPages, Option<UnmappedFrames>) {
        let md = ManuallyDrop::new(self);
        let pages = unsafe { core::ptr::read(&md.pages) };
        let flags = md.flags;

        let mut exclusive_frames: Option<UnmappedFrames> = None;

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..pages.count() {
            let va = pages.start_vaddr() + i * PAGE_SIZE;
            match guard.unmap_to_result(va) {
                Ok(UnmapResult::Exclusive(frames)) => {
                    // 简化：只保留第一段连续帧
                    if exclusive_frames.is_none() {
                        exclusive_frames = Some(frames);
                    }
                }
                Ok(UnmapResult::NonExclusive(_)) => {}
                Err(e) => panic!("MappedPages::unmap: {va} 失败: {e}"),
            }
        }
        drop(guard);
        {
            let _flush = tlb::TlbFlushGuard::new(pages.start_vaddr().as_usize(), pages.count());
        }

        (pages, exclusive_frames)
    }

    /// Drop 内部实现——unmap PTE + 回收 EXCLUSIVE 帧 + 虚拟页自动归还。
    fn unmap_and_release(&mut self) {
        let page_count = self.pages.count();
        let va_start = self.pages.start_vaddr();
        let mut offset = 0;

        while offset < page_count {
            let n = (page_count - offset).min(UNMAP_CHUNK);
            let mut to_reclaim: heapless::Vec<UnmappedFrames, UNMAP_CHUNK> = heapless::Vec::new();

            {
                let pt = crate::kernel_page_table();
                let mut guard = pt.lock();
                for i in 0..n {
                    let va = va_start + (offset + i) * PAGE_SIZE;
                    match guard.unmap_to_result(va) {
                        Ok(UnmapResult::Exclusive(frames)) => {
                            to_reclaim
                                .push(frames)
                                .expect("exclusive 帧数不超过 UNMAP_CHUNK");
                        }
                        Ok(UnmapResult::NonExclusive(_)) => {}
                        Err(e) => {
                            panic!("MappedPages::unmap_and_release: unmap {va} 失败: {e}");
                        }
                    }
                }
            }

            {
                let flush_va = va_start + offset * PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            }

            drop(to_reclaim);
            offset += n;
        }
        // self.pages 的 Drop 在 MappedPages::drop 返回后自动执行，
        // 归还虚拟页给 page_allocator
    }
}

/// 验证偏移在映射范围内且地址对齐到 `T` 的自然边界，返回目标指针。
pub(crate) fn check_bounds_and_align<T>(
    base: usize,
    size: usize,
    offset: usize,
    fn_name: &str,
) -> *const T {
    let type_size = core::mem::size_of::<T>();
    assert!(
        type_size > 0,
        "{fn_name}: 不支持 ZST（size_of::<T>() == 0）"
    );
    assert!(
        type_size <= size && offset <= size - type_size,
        "{fn_name}: offset {:#x} + {type_size} 超出映射大小 {:#x}",
        offset,
        size,
    );
    let addr = base + offset;
    let align = core::mem::align_of::<T>();
    assert!(
        addr.is_multiple_of(align),
        "{fn_name}: 地址 {:#x} 未对齐到 {align} 字节",
        addr,
    );
    addr as *const T
}

impl Drop for MappedPages {
    fn drop(&mut self) {
        self.unmap_and_release();
    }
}

impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MappedPages({}, {} pages, {:?})",
            self.vaddr(),
            self.page_count(),
            self.flags,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use address::VirtAddr;
    use page_allocator::AllocatedPages;

    fn alloc_pages_at(va: usize, count: usize) -> AllocatedPages {
        AllocatedPages::alloc_at(VirtAddr::new(va), count).expect("alloc_pages_at")
    }

    /// map_identity 应建立正确的映射并可查询。
    #[test]
    fn map_identity_basic() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x10_0000, 1);
        let pa = PhysAddr::new(0x10_0000);
        let mp = MappedPages::map_identity(pages, PteFlags::kernel_rw());
        assert_eq!(mp.vaddr(), VirtAddr::new(0x10_0000));
        assert_eq!(mp.size(), PAGE_SIZE);

        let guard = crate::kernel_page_table().lock();
        let (got_pa, _) = guard
            .get_mapping(VirtAddr::new(0x10_0000))
            .expect("映射应存在");
        assert_eq!(got_pa, pa);
        drop(guard);

        // 转为 ManuallyDrop 阻止 drop 时 unmap（identity map 不设 EXCLUSIVE）
        let _ = ManuallyDrop::new(mp);
    }

    /// map_alloc 应分配帧并设置 EXCLUSIVE 位。
    #[test]
    fn map_alloc_sets_exclusive() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x20_0000, 1);
        let va = VirtAddr::new(0x20_0000);
        let mp = MappedPages::map_alloc(pages, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (_, got_flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(got_flags.is_exclusive());
        drop(guard);

        assert_eq!(mp.size(), PAGE_SIZE);
    }

    /// map_alloc 多页后逐页应都有 EXCLUSIVE 位。
    #[test]
    fn map_alloc_multi_page() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x30_0000, 3);
        let va = VirtAddr::new(0x30_0000);
        let _mp = MappedPages::map_alloc(pages, PteFlags::kernel_rw());
        let guard = crate::kernel_page_table().lock();
        for i in 0..3 {
            let page_va = va + i * PAGE_SIZE;
            let (_, flags) = guard.get_mapping(page_va).expect("应已映射");
            assert!(flags.is_exclusive());
        }
    }

    /// Drop map_alloc 映射应 unmap PTE 并回收帧。
    #[test]
    fn drop_alloc_unmaps() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x40_0000, 2);
        let va = VirtAddr::new(0x40_0000);
        let mp = MappedPages::map_alloc(pages, PteFlags::kernel_rw());

        {
            let guard = crate::kernel_page_table().lock();
            assert!(guard.get_mapping(va).is_some());
        }

        drop(mp);

        let guard = crate::kernel_page_table().lock();
        assert!(guard.get_mapping(va).is_none());
    }

    /// split 应拆分为两个独立的 MappedPages。
    #[test]
    fn split_basic() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x50_0000, 4);
        let mp = MappedPages::map_alloc(pages, PteFlags::kernel_rw());

        let (left, right) = mp.split(2);
        assert_eq!(left.page_count(), 2);
        assert_eq!(right.page_count(), 2);
        assert_eq!(left.vaddr(), VirtAddr::new(0x50_0000));
        assert_eq!(right.vaddr(), VirtAddr::new(0x50_0000 + 2 * PAGE_SIZE));
    }

    /// mprotect 应修改 PTE 权限。
    #[test]
    fn mprotect_changes_flags() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x60_0000, 1);
        let va = VirtAddr::new(0x60_0000);
        let mut mp = MappedPages::map_alloc(pages, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_writable());
        drop(guard);

        mp.mprotect(PteFlags::kernel_ro());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(!flags.is_writable());
        assert!(flags.is_exclusive());
    }

    /// as_type 应返回映射区域内正确偏移处的引用。
    #[test]
    fn as_type_reads_memory() {
        crate::ensure_test_init();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);

        let pages = AllocatedPages::alloc_at(va, 1).expect("alloc pages");
        // 建立 identity mapping 使 VA 指向实际的堆内存
        {
            let pt = crate::kernel_page_table();
            let mut guard = pt.lock();
            let pa = PhysAddr::new(va.as_usize());
            guard
                .map_page(va, pa, PteFlags::kernel_rw())
                .expect("map_page");
        }
        let mp = MappedPages {
            pages,
            flags: PteFlags::kernel_rw(),
        };
        let val: &u32 = mp.as_type::<u32>(0);
        assert_eq!(*val, 0);
        // ManuallyDrop 避免 double-unmap（map_page 没设 identity_map_range 的结构）
        let _ = ManuallyDrop::new(mp);
    }
}
