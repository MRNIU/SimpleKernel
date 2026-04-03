//! 各状态专属的 impl 块——状态转换方法与分配接口。

use config::PAGE_SIZE;
use memory_types::PageSize;

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
    /// 内部路径：buddy allocator（4K 粒度）-> `FreeFrames` -> `AllocatedFrames<P>`。
    /// 对于大页（P != Page4K），请求的 4K 帧数 = `count * P::NUM_4K_PAGES`。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        let count_4k = count << P::NUM_4K_PAGES_SHIFT;
        let free = alloc_from_buddy(count_4k)?;

        // SAFETY: 通过 to_virt 将物理地址转换为虚拟地址后写入。
        // 帧刚从分配器获取，不存在其他引用。
        unsafe {
            let ptr = free.start_paddr().to_virt().as_mut_ptr::<u8>();
            core::ptr::write_bytes(ptr, 0, count_4k * PAGE_SIZE);
        }

        let range = free.range();
        core::mem::forget(free);

        assert!(
            P::NUM_4K_PAGES == 1 || range.start().as_usize() & (P::NUM_4K_PAGES - 1) == 0,
            "AllocatedFrames::alloc: buddy 返回的帧未对齐到 P 边界"
        );

        Ok(Self::from_range(range))
    }

    /// 消费 Allocated 帧，转换为 Mapped 状态——表示帧已写入页表。
    ///
    /// 调用方在将帧映射到页表后调用此方法。
    /// `MappedFrames` 的 Drop 会 panic，强制要求必须先 unmap 再释放。
    pub fn into_mapped(self) -> MappedFrames<P> {
        self.into_state()
    }
}

impl<P: PageSize> MappedFrames<P> {
    /// 消费 Mapped 帧，转换为 Unmapped 状态——表示帧已从页表移除。
    ///
    /// 调用方在从页表 unmap 后调用此方法。
    /// `UnmappedFrames` 的 Drop 安全地归还帧到 buddy allocator。
    pub fn into_unmapped(self) -> UnmappedFrames<P> {
        self.into_state()
    }
}

impl<P: PageSize> UnmappedFrames<P> {
    /// 消费 Unmapped 帧，转换回 Allocated 状态——用于重新映射到其他页表。
    pub fn into_allocated(self) -> AllocatedFrames<P> {
        self.into_state()
    }
}

impl UnmappedFrames {
    /// 消费 Unmapped 帧（4K 粒度），转换为 Free 状态。
    pub fn into_free(self) -> FreeFrames {
        self.into_state()
    }
}

#[cfg(test)]
mod tests {
    use crate::alloc::alloc_from_buddy;
    use crate::ensure_test_init;
    use crate::state::AllocatedFrames;

    type Alloc = AllocatedFrames;

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

    /// Free -> Allocated 显式转换。
    #[test]
    fn free_into_allocated() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let pa = free.start_paddr();
        let allocated = free.into_allocated();
        assert_eq!(allocated.start_paddr(), pa);
        assert_eq!(allocated.count(), 1);
    }

    /// Allocated -> Mapped -> Unmapped -> Free 完整生命周期。
    #[test]
    fn full_lifecycle() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let pa = free.start_paddr();
        let allocated = free.into_allocated();

        let mapped = allocated.into_mapped();
        assert_eq!(mapped.start_paddr(), pa);

        let unmapped = mapped.into_unmapped();
        assert_eq!(unmapped.start_paddr(), pa);

        let _free = unmapped.into_free();
        // free drop 后帧归还 buddy
    }

    /// Unmapped -> Allocated（重新映射路径）。
    #[test]
    fn unmapped_into_allocated() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let pa = free.start_paddr();
        let allocated = free.into_allocated();

        let mapped = allocated.into_mapped();
        let unmapped = mapped.into_unmapped();
        let reallocated = unmapped.into_allocated();
        assert_eq!(reallocated.start_paddr(), pa);
        // reallocated drop 归还 buddy
    }

    /// MappedFrames drop 应 panic。
    #[test]
    #[should_panic(expected = "Frames<Mapped> dropped without unmap")]
    fn mapped_drop_panics() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let _mapped = free.into_allocated().into_mapped();
        // _mapped drop -> panic
    }
}
