//! 页分配器错误定义。

use core::fmt;

/// 页分配器错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAllocError {
    /// 页分配器未初始化时尝试分配
    AllocationFailed,
    /// 虚拟地址空间耗尽
    OutOfVirtualSpace,
    /// 指定的虚拟地址不可用（已被占用或不在空闲范围内）
    AddressNotAvailable,
}

impl fmt::Display for PageAllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for PageAllocError {}
