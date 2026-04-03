//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被相同 PA 和 flags 映射——幂等重复，调用方可安全忽略
    AlreadyMappedIdentical,
    /// 目标 VA 已被映射但 PA 或 flags 不同——真正的冲突
    AlreadyMappedConflict,
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
            Self::AlreadyMappedIdentical => {
                write!(f, "virtual address already mapped (identical, idempotent)")
            }
            Self::AlreadyMappedConflict => write!(
                f,
                "virtual address already mapped with different PA or flags"
            ),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::FrameAllocFailed => write!(f, "physical frame allocation failed"),
        }
    }
}

impl core::error::Error for PagingError {}
