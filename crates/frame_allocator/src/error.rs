//! 帧分配器错误定义。

use core::fmt;

/// 帧分配器错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocError {
    /// 帧分配器未初始化时尝试分配
    AllocationFailed,
    /// 物理帧耗尽
    OutOfMemory,
}

impl fmt::Display for FrameAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for FrameAllocError {}
