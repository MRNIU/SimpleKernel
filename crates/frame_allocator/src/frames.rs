// Copyright The SimpleKernel Contributors

//! 已分配帧类型——RAII 物理帧所有权。

use memory_types::PhysAddr;

use crate::FrameSpan;
use crate::alloc::dealloc_to_backend;

/// 已分配的连续物理帧——持有所有权，Drop 时归还分配器。
///
/// 不可 Clone、不可 Copy。帧内容**未初始化**——调用方按需初始化。
///
/// SAS 全量映射下帧始终可通过 identity mapping 访问（`PA.to_virt()`）。
pub struct AllocatedFrames {
    pub(crate) range: FrameSpan,
}

impl core::fmt::Debug for AllocatedFrames {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "AllocatedFrames({}-{})",
            self.range.start(),
            self.range.end()
        )
    }
}

impl AllocatedFrames {
    /// 分配 `count` 个连续 4K 物理帧。**内容未初始化**，调用方负责初始化。
    ///
    /// # Errors
    ///
    /// 帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, crate::FrameAllocError> {
        crate::alloc::alloc_from_backend(count)
    }

    /// 分配一个 4K 物理帧。**内容未初始化**。
    ///
    /// # Errors
    ///
    /// 帧耗尽返回 `OutOfMemory`。
    pub fn alloc_one() -> Result<Self, crate::FrameAllocError> {
        Self::alloc(1)
    }

    /// 范围内 4K 帧的数量。
    #[inline]
    pub fn page_count(&self) -> usize {
        self.range.size()
    }

    /// 起始物理地址。
    #[inline]
    pub fn start_paddr(&self) -> PhysAddr {
        self.range.start().start_addr()
    }

    /// 从 FrameSpan 构造（crate 内部使用）。
    #[inline]
    pub(crate) fn from_range(range: FrameSpan) -> Self {
        Self { range }
    }
}

impl Drop for AllocatedFrames {
    fn drop(&mut self) {
        dealloc_to_backend(self.range);
    }
}
