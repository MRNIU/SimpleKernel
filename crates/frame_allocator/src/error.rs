//! 帧分配器错误定义。

use core::fmt;

/// 帧分配器错误——只有物理帧耗尽一种。
///
/// 分配器未初始化、重复初始化等都是**内核 bug**，由调用点直接 panic，
/// 不经此类型传递（SAS 设计约束：预期外错误 fail-fast）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocError {
    /// 物理帧耗尽
    OutOfMemory,
}

impl fmt::Display for FrameAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for FrameAllocError {}
