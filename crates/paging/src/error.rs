//! 分页子系统错误定义。

use core::fmt;

/// 页表操作错误。
///
/// 此类型 `pub` 是因为 [`NodeFrameOps::alloc`](crate::NodeFrameOps::alloc)
/// 签名需要引用它。PageTable 的 mutating 方法本身是 `pub(crate)` 的，
/// 外部无法直接调用返回此错误的方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableError {
    /// 节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
    /// walk 路径上遇到大页
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 无效地址范围（start >= end）
    InvalidRange,
}

impl fmt::Display for PageTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::AlreadyMapped => write!(f, "virtual address already mapped"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::InvalidRange => write!(f, "invalid address range"),
        }
    }
}

impl core::error::Error for PageTableError {}

/// 分页操作错误——对外公开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
    /// walk 路径上遇到大页冲突
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 无效地址范围
    InvalidRange,
    /// 物理帧分配失败
    FrameAllocFailed,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::AlreadyMapped => write!(f, "virtual address already mapped"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::InvalidRange => write!(f, "invalid address range"),
            Self::FrameAllocFailed => write!(f, "physical frame allocation failed"),
        }
    }
}

impl core::error::Error for PagingError {}

impl From<PageTableError> for PagingError {
    fn from(e: PageTableError) -> Self {
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped => Self::AlreadyMapped,
            PageTableError::HugePageConflict => Self::HugePageConflict,
            PageTableError::PageNotMapped => Self::PageNotMapped,
            PageTableError::InvalidRange => Self::InvalidRange,
        }
    }
}

impl From<frame_allocator::FrameAllocError> for PagingError {
    fn from(_: frame_allocator::FrameAllocError) -> Self {
        Self::FrameAllocFailed
    }
}
