//! 仿射类型映射——move-only 的 VA→PA 映射所有权。
//!
//! [`MappedPages`] 同时持有虚拟页（[`AllocatedPages`]）和物理帧（[`AllocatedFrames`]）
//! 的所有权。Drop 时自动 unmap PTE、回收帧、归还虚拟页。
//!
//! 永久映射使用 `ManuallyDrop<MappedPages>` 阻止 Drop。

use core::mem::ManuallyDrop;

use config::PAGE_SIZE;
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;
use page_allocator::AllocatedPages;

use crate::{PteFlags, PteFlagsOps};

/// unmap_and_release 每次处理的最大页数。
const UNMAP_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 仿射类型映射——持有虚拟页和物理帧所有权。
///
/// 不可 Clone、不可 Copy。Drop 时 unmap PTE 并回收帧，
/// 虚拟页归还 page_allocator。
///
/// SAS 架构下通过全局内核页表操作 PTE，不持有页表引用。
pub struct MappedPages {
    pages: AllocatedPages,
    frames: AllocatedFrames,
    flags: PteFlags,
}

impl MappedPages {
    /// 唯一的创建路径——消费 pages 和 frames 的所有权建立映射。
    ///
    /// 调用方决定 VA/PA 的对应关系：
    ///   - identity map: 确保 pages.start_vaddr() == frames.start_paddr()
    ///   - 匿名映射: pages 和 frames 地址可以不同
    pub fn map(pages: AllocatedPages, frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = pages.count();
        assert_eq!(
            page_count,
            frames.count(),
            "MappedPages::map: pages 和 frames 数量不一致"
        );
        assert!(page_count > 0, "MappedPages::map: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let va_start = pages.start_vaddr();
        let pa_start = frames.start_paddr();

        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * config::PAGE_SIZE;
            let pa = pa_start + i * config::PAGE_SIZE;
            guard
                .map_page(va, pa, flags)
                .expect("MappedPages::map: map_page 失败");
        }
        drop(guard);

        Self {
            pages,
            frames,
            flags,
        }
    }

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

    /// 返回构造时的请求权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 返回持有的虚拟页引用。
    #[must_use]
    pub fn pages(&self) -> &AllocatedPages {
        &self.pages
    }

    /// 返回持有的物理帧引用。
    #[must_use]
    pub fn frames(&self) -> &AllocatedFrames {
        &self.frames
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
        let frames = unsafe { core::ptr::read(&md.frames) };

        let (left_pages, right_pages) = pages.split(page_index);

        let mid_frame = memory_types::Frame::new(frames.start().as_usize() + page_index);
        let (left_frames, right_frames) = frames.split_at(mid_frame);

        (
            MappedPages {
                pages: left_pages,
                frames: left_frames,
                flags,
            },
            MappedPages {
                pages: right_pages,
                frames: right_frames,
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
        let self_frames = unsafe { core::ptr::read(&self_md.frames) };
        let other_frames = unsafe { core::ptr::read(&other_md.frames) };
        let flags = self_md.flags;

        match self_pages.merge(other_pages) {
            Ok(merged_pages) => match self_frames.merge(other_frames) {
                Ok(merged_frames) => Ok(MappedPages {
                    pages: merged_pages,
                    frames: merged_frames,
                    flags,
                }),
                Err((sf, of)) => {
                    let mid = sf.count();
                    let (sp, op) = merged_pages.split(mid);
                    Err((
                        MappedPages {
                            pages: sp,
                            frames: sf,
                            flags,
                        },
                        MappedPages {
                            pages: op,
                            frames: of,
                            flags,
                        },
                    ))
                }
            },
            Err((sp, op)) => Err((
                MappedPages {
                    pages: sp,
                    frames: self_frames,
                    flags,
                },
                MappedPages {
                    pages: op,
                    frames: other_frames,
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

    /// 手动解除映射并取回所有权。
    ///
    /// 返回虚拟页和物理帧供调用方重用或释放。
    pub fn unmap(self) -> (AllocatedPages, AllocatedFrames) {
        let md = ManuallyDrop::new(self);
        // SAFETY: md 不会 Drop，我们手动接管所有权
        let pages = unsafe { core::ptr::read(&md.pages) };
        let frames = unsafe { core::ptr::read(&md.frames) };
        let page_count = pages.count();
        let va_start = pages.start_vaddr();

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * config::PAGE_SIZE;
            guard
                .unmap_page(va)
                .expect("MappedPages::unmap: unmap_page 失败");
        }
        drop(guard);

        {
            let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
        }

        (pages, frames)
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
        let page_count = self.pages.count();
        let va_start = self.pages.start_vaddr();
        let pt = crate::kernel_page_table();

        let mut offset = 0;
        while offset < page_count {
            let n = (page_count - offset).min(UNMAP_CHUNK);
            {
                let mut guard = pt.lock();
                for i in 0..n {
                    let va = va_start + (offset + i) * config::PAGE_SIZE;
                    guard.unmap_page(va).unwrap_or_else(|e| {
                        panic!("MappedPages::drop: unmap {va} 失败: {e}");
                    });
                }
            }
            {
                let flush_va = va_start + offset * config::PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            }
            offset += n;
        }
        // self.frames 和 self.pages 在 Drop 返回后自动 drop
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
    use memory_types::VirtAddr;
    use page_allocator::AllocatedPages;

    fn alloc_pages_at(va: usize, count: usize) -> AllocatedPages {
        AllocatedPages::alloc_at(VirtAddr::new(va), count).expect("alloc_pages_at")
    }

    fn alloc_frames(count: usize) -> AllocatedFrames {
        AllocatedFrames::alloc(count).expect("alloc_frames")
    }

    // 测试地址范围: page_allocator 测试 init 覆盖 [0x1000_0000, 0x1010_0000)
    // 每个测试使用不同的偏移避免并行冲突。

    #[test]
    fn map_basic() {
        crate::ensure_test_init();
        let va_base = 0x1000_0000;
        let pages = alloc_pages_at(va_base, 1);
        let frames = alloc_frames(1);
        let pa = frames.start_paddr();
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (got_pa, _) = guard
            .get_mapping(VirtAddr::new(va_base))
            .expect("映射应存在");
        assert_eq!(got_pa, pa);
        assert_eq!(mp.size(), config::PAGE_SIZE);
    }

    #[test]
    fn map_multi_page() {
        crate::ensure_test_init();
        let va_base = 0x1000_2000;
        let pages = alloc_pages_at(va_base, 3);
        let frames = alloc_frames(3);
        let _mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());
        let guard = crate::kernel_page_table().lock();
        for i in 0..3 {
            assert!(
                guard
                    .get_mapping(VirtAddr::new(va_base + i * config::PAGE_SIZE))
                    .is_some()
            );
        }
    }

    #[test]
    fn drop_unmaps() {
        crate::ensure_test_init();
        let va_base = 0x1000_6000;
        let pages = alloc_pages_at(va_base, 2);
        let va = VirtAddr::new(va_base);
        let frames = alloc_frames(2);
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        {
            let guard = crate::kernel_page_table().lock();
            assert!(guard.get_mapping(va).is_some());
        }
        drop(mp);
        let guard = crate::kernel_page_table().lock();
        assert!(guard.get_mapping(va).is_none());
    }

    #[test]
    fn split_basic() {
        crate::ensure_test_init();
        let va_base = 0x1000_9000;
        let pages = alloc_pages_at(va_base, 4);
        let frames = alloc_frames(4);
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let (left, right) = mp.split(2);
        assert_eq!(left.page_count(), 2);
        assert_eq!(right.page_count(), 2);
        assert_eq!(left.vaddr(), VirtAddr::new(va_base));
        assert_eq!(
            right.vaddr(),
            VirtAddr::new(va_base + 2 * config::PAGE_SIZE)
        );
    }

    #[test]
    fn mprotect_changes_flags() {
        crate::ensure_test_init();
        let va_base = 0x1000_E000;
        let pages = alloc_pages_at(va_base, 1);
        let va = VirtAddr::new(va_base);
        let frames = alloc_frames(1);
        let mut mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_writable());
        drop(guard);

        mp.mprotect(PteFlags::kernel_ro());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(!flags.is_writable());
    }
}
