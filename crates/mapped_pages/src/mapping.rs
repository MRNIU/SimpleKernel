//! 仿射类型映射——move-only 的 VA→PA 映射所有权。
//!
// TODO: 实现 `split` / `merge` 操作——`munmap` 部分区域和 `mremap` 需要。
//
// TODO: 支持 COW（Copy-on-Write）共享映射——`fork()` 需要多个进程共享同一物理帧
// + 引用计数。需引入 `Frames` 的共享状态或 per-frame 引用计数器。

use alloc::sync::Arc;

use address::{FrameRange, PhysAddr, PhysPageNum, VirtAddr};
use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, UnmappedFrames};
use page_table::{NodeFrameOps, PageTable, PteFlags, PteFlagsOps};
use sync_crate::SpinLock;

use crate::error::MappedPagesError;

/// 仿射类型映射——持有此值即证明 VA→PA 映射有效。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
/// Drop 时根据 PTE 中的 EXCLUSIVE 位决定是否释放物理帧。
pub struct MappedPages<F: NodeFrameOps> {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 映射权限
    flags: PteFlags,
    /// 永久映射标记——drop 时不 unmap
    permanent: bool,
    /// 所属页表的 `Arc` 引用——Drop 时通过此引用 unmap。
    ///
    /// 内核 `MappedPages` 持有全局内核页表的 `Arc`；
    /// 用户进程 `MappedPages` 持有对应进程页表的 `Arc`。
    /// 进程退出后其所有 `MappedPages` drop，`Arc` 引用计数归零时页表自动释放。
    page_table: Arc<SpinLock<PageTable<F>>>,
}

impl<F: NodeFrameOps> MappedPages<F> {
    /// Identity-map 一段物理地址区间（VA == PA）。
    ///
    /// **不设置 EXCLUSIVE 位**——drop 时仅 unmap PTE，不释放帧。
    /// 虚拟地址不经过页分配器（VA == PA，由物理布局决定）。
    /// 使用场景：内核启动时的 RAM identity mapping、MMIO 映射。
    ///
    /// # Errors
    ///
    /// 映射冲突时返回错误。
    pub fn map_identity(
        pt: &mut PageTable<F>,
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, MappedPagesError> {
        let va_start = VirtAddr::new(pa_start.as_usize());
        for i in 0..page_count {
            let pa = pa_start + i * PAGE_SIZE;
            let va = va_start + i * PAGE_SIZE;
            if let Err(e) = pt.map_page(va, pa, flags) {
                for j in (0..i).rev() {
                    let _ = pt.unmap_page(va_start + j * PAGE_SIZE);
                }
                return Err(e.into());
            }
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            permanent: false,
            page_table: pt_ref,
        })
    }

    /// 包装已由外部建立的映射——不执行 map 操作。
    ///
    /// 调用方必须确保：
    /// 1. `[vaddr, vaddr + page_count * PAGE_SIZE)` 范围内的映射已在
    ///    `pt_ref` 指向的页表中建立
    /// 2. 映射在 `MappedPages` 生命周期内保持有效（除非标记为 permanent）
    ///
    /// Drop 时仅 unmap PTE，帧回收由 PTE 的 EXCLUSIVE 位控制。
    pub fn new_borrowed(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        vaddr: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Self {
        Self {
            vaddr,
            page_count,
            flags,
            permanent: false,
            page_table: pt_ref,
        }
    }

    /// 分配新帧并建立映射——**设置 EXCLUSIVE 位**。
    ///
    /// 内部分配帧并逐页映射，帧所有权通过 PTE 的 EXCLUSIVE 位追踪。
    /// 虚拟地址由调用方指定（通常通过 `AddressSpace` 的 VMA 管理）。
    ///
    /// # Errors
    ///
    /// 帧分配失败或映射冲突时返回错误。
    pub fn map_alloc(
        pt: &mut PageTable<F>,
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, MappedPagesError> {
        let exclusive_flags = flags.with_exclusive();
        let mut mapped_count = 0usize;
        for i in 0..page_count {
            let frame = AllocatedFrames::alloc_one()?;
            let pa = frame.start_paddr();
            let va = va_start + i * PAGE_SIZE;
            match pt.map_page(va, pa, exclusive_flags) {
                Ok(()) => {
                    // 帧所有权转移到 PTE：forget 阻止 drop 回收
                    let mapped = frame.into_mapped();
                    core::mem::forget(mapped);
                    mapped_count += 1;
                }
                Err(e) => {
                    // 回滚已映射的页——EXCLUSIVE 帧通过 unmap 路径回收
                    for j in (0..mapped_count).rev() {
                        let va = va_start + j * PAGE_SIZE;
                        if let Ok(old_pa) = pt.unmap_page(va) {
                            reclaim_exclusive_frame(old_pa);
                        }
                    }
                    // 当前这个未映射成功的 frame 会正常 drop 回收
                    return Err(e.into());
                }
            }
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags: exclusive_flags,
            permanent: false,
            page_table: pt_ref,
        })
    }

    /// 消耗 self，标记为永久映射（drop 时不 unmap）。
    ///
    /// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    /// 返回映射起始虚拟地址。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.page_count * PAGE_SIZE
    }

    /// 返回映射权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用**生命周期绑定到 `&self`**——
    /// 编译器保证 `MappedPages` drop 后无法使用该引用（use-after-unmap 防护）。
    ///
    /// # Panics
    ///
    /// 在以下条件不满足时 panic：
    /// 1. `offset + size_of::<T>()` 不超过映射大小
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        let addr = self.vaddr.as_usize() + offset;
        assert!(
            addr.is_multiple_of(core::mem::align_of::<T>()),
            "MappedPages::as_type: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *const T = addr as *const T;
        // SAFETY: assert 已验证偏移在映射范围内且地址对齐；FromBytes 保证任意位模式均为合法 T；
        // &self 保证映射存活，引用生命周期绑定到 self
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    ///
    /// # Panics
    ///
    /// 在以下条件不满足时 panic：
    /// 1. `offset + size_of::<T>()` 不超过映射大小
    /// 2. 映射具有 WRITE 权限
    /// 3. 偏移对齐到 `T` 的自然对齐边界
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type_mut: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        assert!(
            self.flags.is_writable(),
            "MappedPages::as_type_mut: 映射无 WRITE 权限"
        );
        let addr = self.vaddr.as_usize() + offset;
        assert!(
            addr.is_multiple_of(core::mem::align_of::<T>()),
            "MappedPages::as_type_mut: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *mut T = addr as *mut T;
        // SAFETY: assert 已验证偏移在映射范围内、映射可写且地址对齐；FromBytes 保证任意位模式均为合法 T；
        // &mut self 保证映射存活且独占访问，引用生命周期绑定到 self
        unsafe { &mut *ptr }
    }

    /// 从所属页表中 unmap 所有页，EXCLUSIVE 帧自动回收。
    ///
    /// 正确的操作顺序（SMP 安全）：
    /// 1. 清除 PTE（在页表锁内）
    /// 2. TLB flush（确保所有核心的 stale TLB 失效）
    /// 3. 回收物理帧（此时没有核心持有指向这些帧的 TLB 条目）
    fn unmap_and_reclaim(&self) {
        let mut frames_to_reclaim: alloc::vec::Vec<PhysAddr> = alloc::vec::Vec::new();

        {
            let mut guard = self.page_table.lock();
            for i in 0..self.page_count {
                let va = self.vaddr + i * PAGE_SIZE;
                if let Some((pa, flags)) = guard.get_mapping(va) {
                    let _ = guard.unmap_page(va);
                    if flags.is_exclusive() {
                        frames_to_reclaim.push(pa);
                    }
                }
            }
        } // 页表锁释放

        // TLB flush——必须在帧回收之前完成
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr.as_usize(), self.page_count);
        } // TlbFlushGuard drop 触发刷新

        // 所有核心的 TLB 已刷新，安全回收帧
        for pa in frames_to_reclaim {
            reclaim_exclusive_frame(pa);
        }
    }
}

/// 从物理地址重建 `UnmappedFrames` 并 drop 回收——EXCLUSIVE unmap 的核心路径。
fn reclaim_exclusive_frame(pa: PhysAddr) {
    let ppn = PhysPageNum::from(pa);
    let range = FrameRange::new(ppn, ppn + 1);
    // SAFETY: 帧刚从页表 unmap，EXCLUSIVE 保证我们拥有该帧的唯一引用。
    // 构造 UnmappedFrames 使其 Drop 自动归还分配器。
    let _reclaimed = unsafe { UnmappedFrames::from_range(range) };
}

impl<F: NodeFrameOps> Drop for MappedPages<F> {
    fn drop(&mut self) {
        if self.permanent {
            return;
        }
        self.unmap_and_reclaim();
    }
}

impl<F: NodeFrameOps> core::fmt::Debug for MappedPages<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = if self.permanent {
            "permanent"
        } else if self.flags.is_exclusive() {
            "exclusive"
        } else {
            "borrowed"
        };
        write!(
            f,
            "MappedPages({}, {} pages, {:?}, {})",
            self.vaddr, self.page_count, self.flags, kind
        )
    }
}

/// 创建测试用 `Arc<SpinLock<PageTable>>`。
#[cfg(any(test, feature = "test-support"))]
pub fn test_pt() -> Arc<SpinLock<PageTable<page_table::HeapNodeFrame>>> {
    let pt = PageTable::create().expect("创建页表");
    Arc::new(SpinLock::new(pt, "test_pt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use page_table::HeapNodeFrame;

    type Mp = MappedPages<HeapNodeFrame>;

    /// map_identity 应建立正确的映射并可查询。
    #[test]
    fn map_identity_basic() {
        let pt_ref = test_pt();
        let mp = {
            let mut guard = pt_ref.lock();
            let pa = PhysAddr::new(0x1_0000);
            let mp = Mp::map_identity(&mut guard, pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
                .expect("map_identity 应成功");
            assert_eq!(mp.vaddr(), VirtAddr::new(0x1_0000));
            assert_eq!(mp.size(), PAGE_SIZE);
            let (got_pa, got_flags) = guard
                .get_mapping(VirtAddr::new(0x1_0000))
                .expect("应能查到映射");
            assert_eq!(got_pa, pa);
            assert!(!got_flags.is_exclusive());
            mp
        };
        let _ = mp.into_permanent();
    }

    /// map_identity 重复映射同一 VA 应失败并回滚。
    #[test]
    fn map_identity_conflict_rollback() {
        let pt_ref = test_pt();
        let mut guard = pt_ref.lock();
        let pa = PhysAddr::new(0x2_0000);
        let _mp1 = Mp::map_identity(&mut guard, pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect("首次 map 应成功")
            .into_permanent();
        let err = Mp::map_identity(&mut guard, pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect_err("重复 map 应失败");
        assert!(matches!(err, MappedPagesError::PageTable(_)));
    }

    /// new_borrowed 应创建非 EXCLUSIVE 映射。
    #[test]
    fn new_borrowed_ownership() {
        let pt_ref = test_pt();
        let mp = Mp::new_borrowed(pt_ref, VirtAddr::new(0x3_0000), 2, PteFlags::kernel_rw());
        assert_eq!(mp.vaddr(), VirtAddr::new(0x3_0000));
        assert_eq!(mp.size(), 2 * PAGE_SIZE);
        assert!(mp.flags().is_writable());
        assert!(!mp.flags().is_exclusive());
        let dbg = alloc::format!("{:?}", mp);
        assert!(dbg.contains("borrowed"));
        let _ = mp.into_permanent();
    }

    /// into_permanent 应标记为永久映射。
    #[test]
    fn into_permanent_marks_permanent() {
        let pt_ref = test_pt();
        let mp = Mp::new_borrowed(pt_ref, VirtAddr::new(0x4_0000), 1, PteFlags::kernel_rw());
        let mp = mp.into_permanent();
        let dbg = alloc::format!("{:?}", mp);
        assert!(dbg.contains("permanent"));
    }

    /// 多页 map_identity 后逐页查询应都有效。
    #[test]
    fn map_identity_multi_page() {
        let pt_ref = test_pt();
        let mut guard = pt_ref.lock();
        let pa = PhysAddr::new(0x5_0000);
        let _mp = Mp::map_identity(&mut guard, pt_ref.clone(), pa, 3, PteFlags::kernel_ro())
            .expect("多页 map 应成功")
            .into_permanent();
        for i in 0..3 {
            let va = VirtAddr::new(0x5_0000 + i * PAGE_SIZE);
            assert!(guard.get_mapping(va).is_some(), "第 {} 页应已映射", i);
        }
    }

    /// map_alloc 应分配帧并设置 EXCLUSIVE 位。
    #[test]
    fn map_alloc_sets_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let mut guard = pt_ref.lock();
        let va = VirtAddr::new(0x10_0000);
        let mp = Mp::map_alloc(&mut guard, pt_ref.clone(), va, 1, PteFlags::kernel_rw())
            .expect("map_alloc 应成功");

        let (_, got_flags) = guard.get_mapping(va).expect("应能查到映射");
        assert!(
            got_flags.is_exclusive(),
            "map_alloc 映射应设置 EXCLUSIVE 位"
        );
        assert!(mp.flags().is_exclusive());
        assert_eq!(mp.size(), PAGE_SIZE);
        let _ = mp.into_permanent();
    }

    /// map_alloc 多页后逐页应都有 EXCLUSIVE 位。
    #[test]
    fn map_alloc_multi_page_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let mut guard = pt_ref.lock();
        let va = VirtAddr::new(0x20_0000);
        let _mp = Mp::map_alloc(&mut guard, pt_ref.clone(), va, 3, PteFlags::kernel_rw())
            .expect("多页 map_alloc 应成功")
            .into_permanent();
        for i in 0..3 {
            let page_va = VirtAddr::new(0x20_0000 + i * PAGE_SIZE);
            let (_, flags) = guard.get_mapping(page_va).expect("应已映射");
            assert!(flags.is_exclusive(), "第 {} 页应有 EXCLUSIVE 位", i);
        }
    }

    /// identity_map_range 边界检查：start >= end 应返回错误。
    #[test]
    fn identity_map_range_empty_range_fails() {
        let mut pt = PageTable::<HeapNodeFrame>::create().expect("创建页表");
        let pa = PhysAddr::new(0x1000);
        let err = pt
            .identity_map_range(pa, pa, PteFlags::kernel_rw())
            .expect_err("start == end 应失败");
        assert_eq!(err, page_table::error::PageTableError::InvalidRange);
    }

    /// as_type 应返回映射区域内正确偏移处的引用。
    ///
    /// 使用 `new_borrowed` 将堆上真实内存包装为 `MappedPages`，
    /// 避免在主机测试中解引用未映射的虚拟地址。
    #[test]
    fn as_type_reads_mapped_memory() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        let mp = Mp::new_borrowed(pt_ref, va, 1, PteFlags::kernel_rw());

        let val: &u32 = mp.as_type::<u32>(0);
        assert_eq!(*val, 0);
        let _ = mp.into_permanent();
        drop(buf);
    }

    /// as_type_mut 应能写入映射区域。
    #[test]
    fn as_type_mut_writes_mapped_memory() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        let mut mp = Mp::new_borrowed(pt_ref, va, 1, PteFlags::kernel_rw());

        let val: &mut u32 = mp.as_type_mut::<u32>(0);
        *val = 0xDEAD_BEEF;
        let readback: &u32 = mp.as_type::<u32>(0);
        assert_eq!(*readback, 0xDEAD_BEEF);
        let _ = mp.into_permanent();
        drop(buf);
    }

    /// as_type 偏移越界应 panic。
    #[test]
    #[should_panic(expected = "超出映射大小")]
    fn as_type_out_of_bounds_panics() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        let mp = Mp::new_borrowed(pt_ref, va, 1, PteFlags::kernel_rw());
        let _: &u32 = mp.as_type::<u32>(PAGE_SIZE);
    }
}
