//! 各状态专属的 impl 块——状态转换方法与分配接口。

use memory_types::PageSize;

use crate::FrameAllocError;
use crate::alloc::alloc_from_backend;
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

    /// 分配 `count` 个连续的 P 大小物理帧。
    ///
    /// 帧内容**未清零**——调用方在 [`OwnedPages::map`] 建立映射后会自动清零。
    /// 分配后帧处于 Allocated 状态，尚未映射到页表，不可直接访问。
    ///
    /// 内部路径：bitmap allocator（4K 粒度）-> `FreeFrames` -> `AllocatedFrames<P>`。
    /// 对于大页（P != Page4K），请求的 4K 帧数 = `count * P::NUM_4K_PAGES`。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        let count_4k = count << P::NUM_4K_PAGES_SHIFT;
        let free = alloc_from_backend(count_4k)?;

        let range = free.range();
        core::mem::forget(free);

        assert!(
            P::NUM_4K_PAGES == 1 || range.start().as_usize() & (P::NUM_4K_PAGES - 1) == 0,
            "AllocatedFrames::alloc: 分配器返回的帧未对齐到 P 边界"
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
    /// `UnmappedFrames` 的 Drop 安全地归还帧到 bitmap allocator。
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
