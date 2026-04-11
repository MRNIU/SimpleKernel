//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// walk 路径上遇到非预期的叶 PTE（4KB-only 下不应出现）
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::HugePageConflict => write!(f, "unexpected leaf PTE in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
        }
    }
}

impl core::error::Error for PagingError {}
