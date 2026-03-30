//! 内存子系统错误定义。

use core::fmt;

/// 内存子系统错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// 帧分配器未初始化时尝试分配
    AllocationFailed,
    /// 物理帧耗尽
    OutOfMemory,
    /// 页表映射失败（如重复映射）
    MapFailed,
    /// 目标虚拟页未映射
    PageNotMapped,
    /// 全局内核页表未初始化
    InvalidPageTable,
    /// VMA 区域与已有区域重叠
    RegionOverlap,
    /// 未找到包含指定地址的 VMA 区域
    RegionNotFound,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for MemoryError {}

#[cfg(any(test, target_os = "none"))]
impl From<frame_allocator::FrameAllocError> for MemoryError {
    fn from(e: frame_allocator::FrameAllocError) -> Self {
        match e {
            frame_allocator::FrameAllocError::AllocationFailed => Self::AllocationFailed,
            frame_allocator::FrameAllocError::OutOfMemory => Self::OutOfMemory,
        }
    }
}

#[cfg(any(test, target_os = "none"))]
impl From<page_allocator::PageAllocError> for MemoryError {
    fn from(e: page_allocator::PageAllocError) -> Self {
        match e {
            page_allocator::PageAllocError::AllocationFailed => Self::AllocationFailed,
            page_allocator::PageAllocError::OutOfMemory => Self::OutOfMemory,
        }
    }
}
