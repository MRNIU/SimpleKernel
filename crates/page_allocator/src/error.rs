//! 虚拟页分配器错误定义。

use core::fmt;

/// 虚拟页分配器错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAllocError {
    /// 分配器未初始化时尝试分配
    AllocationFailed,
    /// 虚拟页耗尽
    OutOfMemory,
}

impl fmt::Display for PageAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for PageAllocError {}
