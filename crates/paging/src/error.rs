//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// walk 路径上遇到大页冲突
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 物理帧分配失败
    FrameAllocFailed,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::FrameAllocFailed => write!(f, "physical frame allocation failed"),
        }
    }
}

impl core::error::Error for PagingError {}
