//! 内存子系统错误定义。

use core::fmt;

/// 内存子系统错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// 帧分配器未初始化时尝试分配
    AllocationFailed,
    /// 物理帧耗尽
    OutOfMemory,
    /// 页表映射失败（参数无效、size 为 0 等）
    MapFailed,
    /// 目标 VA 已被相同 PA+flags 映射——幂等重复，调用方可安全忽略
    AlreadyMappedIdentical,
    /// 目标 VA 已被映射但 PA 或 flags 不同——真正的冲突
    AlreadyMappedConflict,
    /// 目标虚拟页未映射
    PageNotMapped,
    /// VMA 区域与已有区域完全重合——幂等重复，调用方可安全忽略
    RegionIdentical,
    /// VMA 区域与已有区域部分重叠——真正的冲突
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

impl From<frame_allocator::FrameAllocError> for MemoryError {
    fn from(e: frame_allocator::FrameAllocError) -> Self {
        match e {
            frame_allocator::FrameAllocError::AllocationFailed => Self::AllocationFailed,
            frame_allocator::FrameAllocError::OutOfMemory => Self::OutOfMemory,
        }
    }
}

impl From<paging::error::PagingError> for MemoryError {
    fn from(e: paging::error::PagingError) -> Self {
        use paging::error::PagingError;
        match e {
            PagingError::AllocationFailed | PagingError::FrameAllocFailed => Self::AllocationFailed,
            PagingError::AlreadyMappedIdentical => Self::AlreadyMappedIdentical,
            PagingError::AlreadyMappedConflict => Self::AlreadyMappedConflict,
            PagingError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
