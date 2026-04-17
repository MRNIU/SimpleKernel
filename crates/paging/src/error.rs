//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 已有 PTE 的 flags 与请求冲突（同 PA + 不同 flags）
    FlagsConflict,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::FlagsConflict => {
                write!(f, "PTE flags conflict (same PA, different flags)")
            }
        }
    }
}

impl core::error::Error for PagingError {}
