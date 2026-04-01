//! 各状态专属的 impl 块——状态转换方法与分配接口。

use address::{FrameRange, PageSize};
use config::PAGE_SIZE;

use crate::FrameAllocError;
use crate::alloc::alloc_from_buddy;
use crate::state::{AllocatedFrames, FreeFrames, MappedFrames, UnmappedFrames};

impl FreeFrames {
    /// 消费 Free 帧，转换为 Allocated 状态。
    pub fn into_allocated(self) -> AllocatedFrames {
        self.into_state()
    }
}

impl<P: PageSize> AllocatedFrames<P> {
    /// 分配一个 P 大小的物理帧，内容清零。
    pub fn alloc_one() -> Result<Self, FrameAllocError> {
        Self::alloc(1)
    }

    /// 分配 `count` 个连续的 P 大小物理帧，内容清零。
    ///
    /// 内部路径：buddy allocator（4K 粒度）→ `FreeFrames` → `AllocatedFrames<P>`。
    /// 对于大页（P != Page4K），请求的 4K 帧数 = `count × P::NUM_4K_PAGES`。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        let count_4k = count * P::NUM_4K_PAGES;
        let free = alloc_from_buddy(count_4k)?;

        // SAFETY: 通过 phys_to_virt 将物理地址转换为虚拟地址后写入。
        // 帧刚从分配器获取，不存在其他引用。
        unsafe {
            let ptr = address::phys_to_virt(free.start_paddr()).as_mut_ptr::<u8>();
            core::ptr::write_bytes(ptr, 0, count_4k * PAGE_SIZE);
        }

        let range = free.range();
        core::mem::forget(free);

        assert!(
            P::NUM_4K_PAGES == 1 || range.start().as_usize() % P::NUM_4K_PAGES == 0,
            "AllocatedFrames::alloc: buddy 返回的帧未对齐到 P 边界"
        );

        Ok(Self::from_range(range))
    }

    /// 消费 Allocated 帧，转换为 Mapped 状态。
    ///
    /// 在帧被写入页表后调用。
    pub fn into_mapped(self) -> MappedFrames<P> {
        self.into_state()
    }
}

impl<P: PageSize> MappedFrames<P> {
    /// 消费 Mapped 帧，转换为 Unmapped 状态。
    ///
    /// 在帧从页表中 unmap 后调用。
    pub fn into_unmapped(self) -> UnmappedFrames<P> {
        self.into_state()
    }
}

impl<P: PageSize> UnmappedFrames<P> {
    /// 从 4K 帧范围构造 `UnmappedFrames`——用于 EXCLUSIVE unmap 路径。
    ///
    /// # Safety
    ///
    /// 调用方必须确保该帧范围刚从页表 unmap，且 PTE 的 EXCLUSIVE 位已确认
    /// 我们拥有该帧的唯一引用。Drop 时帧将归还分配器。
    pub unsafe fn from_unmapped_range(range: FrameRange) -> Self {
        Self::from_range(range)
    }

    /// 消费 Unmapped 帧，转换回 Allocated 状态（可重新映射到其他页表）。
    pub fn into_allocated(self) -> AllocatedFrames<P> {
        self.into_state()
    }

    /// 显式释放帧——等价于 `drop(self)`。
    ///
    /// 通常不需要手动调用——Drop 会自动完成。
    /// 仅在需要显式控制释放时机时使用。
    pub fn release(self) {
        // Drop 自动处理 dealloc_to_buddy
    }
}

#[cfg(test)]
mod tests {
    use address::PhysPageNum;

    use crate::alloc::alloc_from_buddy;
    use crate::ensure_test_init;
    use crate::state::{AllocatedFrames, UnmappedFrames};

    type Alloc = AllocatedFrames;
    type Unmap = UnmappedFrames;

    /// 分配单帧后帧计数应为 1，地址应页对齐。
    #[test]
    fn alloc_one_frame() {
        ensure_test_init();
        let frame = Alloc::alloc_one().expect("alloc_one 应成功");
        assert_eq!(frame.count(), 1);
        assert!(frame.start_paddr().is_aligned());
    }

    /// 分配多帧后帧计数应正确。
    #[test]
    fn alloc_multiple_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4) 应成功");
        assert_eq!(frames.count(), 4);
    }

    /// 帧 drop 后应能重新分配（归还到分配器）。
    #[test]
    fn alloc_dealloc_realloc() {
        ensure_test_init();
        {
            let _frame = Alloc::alloc_one().expect("分配");
        }
        // frame 已 drop，帧应归还
        let frame2 = Alloc::alloc_one().expect("重新分配应成功");
        assert!(frame2.start_paddr().is_aligned());
    }

    /// Allocated → Mapped → Unmapped 状态转换链。
    #[test]
    fn typestate_transitions() {
        ensure_test_init();
        let allocated = Alloc::alloc_one().expect("分配");
        let pa = allocated.start_paddr();

        let mapped = allocated.into_mapped();
        assert_eq!(mapped.start_paddr(), pa);

        let unmapped = mapped.into_unmapped();
        assert_eq!(unmapped.start_paddr(), pa);

        // Unmapped drop 自动归还分配器
    }

    /// Unmapped → Allocated 回转。
    #[test]
    fn unmapped_back_to_allocated() {
        ensure_test_init();
        let allocated = Alloc::alloc_one().expect("分配");
        let mapped = allocated.into_mapped();
        let unmapped = mapped.into_unmapped();
        let reallocated = unmapped.into_allocated();
        assert_eq!(reallocated.count(), 1);
    }

    /// Unmapped 显式释放后应能重新分配。
    #[test]
    fn unmapped_release_reclaims() {
        ensure_test_init();
        let allocated = Alloc::alloc_one().expect("分配");
        let pa = allocated.start_paddr();
        let mapped = allocated.into_mapped();
        let unmapped = mapped.into_unmapped();
        assert_eq!(unmapped.start_paddr(), pa);
        unmapped.release();
        // release 后帧归还分配器，重新分配应成功
        let _frame2 = Alloc::alloc_one().expect("release 后应能重新分配");
    }

    /// Free → Allocated 显式转换。
    #[test]
    fn free_into_allocated() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let pa = free.start_paddr();
        let allocated = free.into_allocated();
        assert_eq!(allocated.start_paddr(), pa);
        assert_eq!(allocated.count(), 1);
    }

    /// 完整生命周期：Free → Allocated → Mapped → Unmapped → Free。
    #[test]
    fn full_lifecycle() {
        ensure_test_init();
        let free = alloc_from_buddy(2).expect("buddy 分配");
        let pa = free.start_paddr();

        let allocated = free.into_allocated();
        assert_eq!(allocated.start_paddr(), pa);

        let mapped = allocated.into_mapped();
        assert_eq!(mapped.start_paddr(), pa);

        let unmapped = mapped.into_unmapped();
        assert_eq!(unmapped.start_paddr(), pa);

        assert_eq!(unmapped.start_paddr(), pa);
        assert_eq!(unmapped.count(), 2);
        unmapped.release();
        // release 归还分配器
    }

    /// split_at 应正确分割帧范围。
    #[test]
    fn split_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4)");
        let mid = PhysPageNum::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        assert_eq!(left.count(), 2);
        assert_eq!(right.count(), 2);
    }

    /// merge 相邻帧应成功。
    #[test]
    fn merge_adjacent_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4)");
        let mid = PhysPageNum::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        let merged = left
            .merge(right)
            .unwrap_or_else(|_| panic!("相邻帧 merge 应成功"));
        assert_eq!(merged.count(), 4);
    }

    /// `UnmappedFrames::from_range` 构造后 drop 应归还分配器。
    #[test]
    fn from_range_reclaims() {
        ensure_test_init();
        let frame = Alloc::alloc_one().expect("分配");
        let range = frame.range();
        let mapped = frame.into_mapped();
        core::mem::forget(mapped);
        // SAFETY: 帧已 forget（模拟 EXCLUSIVE unmap 路径），手动重建 Unmapped 以回收
        let _unmapped = unsafe { Unmap::from_unmapped_range(range) };
        // drop 归还分配器，后续分配应成功
        let _frame2 = Alloc::alloc_one().expect("from_range 回收后应能重新分配");
    }

    /// Mapped 帧 drop 时应 panic。
    #[test]
    #[should_panic(expected = "Frames<Mapped> dropped without unmapping")]
    fn mapped_drop_panics() {
        ensure_test_init();
        let allocated = Alloc::alloc_one().expect("分配");
        let _mapped = allocated.into_mapped();
        // _mapped drop 时应 panic
    }

    /// 合并不相邻的帧应失败并归还双方所有权。
    #[test]
    fn merge_non_adjacent_fails() {
        ensure_test_init();
        let a = Alloc::alloc_one().expect("分配 a");
        let b = Alloc::alloc_one().expect("分配 b");
        // 两次独立分配的帧不一定相邻
        let a_start = a.start();
        let b_start = b.start();
        if a_start.as_usize().abs_diff(b_start.as_usize()) > 1 {
            match a.merge(b) {
                Err((returned_a, returned_b)) => {
                    assert_eq!(returned_a.start(), a_start);
                    assert_eq!(returned_b.start(), b_start);
                }
                Ok(_) => panic!("不相邻帧 merge 应失败"),
            }
        }
        // 即使相邻也不出错——测试 merge 本身不 panic
    }
}
