//! 各状态专属的 impl 块——状态转换方法与分配接口。

use config::PAGE_SIZE;

use crate::FrameAllocError;
use crate::alloc::alloc_from_backend;
use crate::state::{AllocatedFrames, FreeFrames};

impl FreeFrames {
    /// 消费 Free 帧，转换为 Allocated 状态。
    pub(crate) fn into_allocated(self) -> AllocatedFrames {
        self.into_state()
    }
}

impl AllocatedFrames {
    /// 分配一个 4K 物理帧，内容清零。
    pub fn alloc_one() -> Result<Self, FrameAllocError> {
        Self::alloc(1)
    }

    /// 分配 `count` 个连续的 4K 物理帧，内容清零。
    ///
    /// SAS 全量映射下帧始终可访问（identity mapping），分配后立即清零防止泄漏旧数据。
    ///
    /// 内部路径：buddy allocator -> `FreeFrames` -> `AllocatedFrames`。
    /// buddy 内部将帧数向上取整到 2 的幂次，
    /// 实际分配的帧数可能多于请求数，但 `FrameSpan` 仅跟踪请求的帧。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        let free = alloc_from_backend(count)?;

        // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚从分配器取出，无其他引用
        unsafe {
            let ptr = free.start_paddr().to_virt().as_mut_ptr::<u8>();
            core::ptr::write_bytes(ptr, 0, count * PAGE_SIZE);
        }

        Ok(free.into_allocated())
    }
}
