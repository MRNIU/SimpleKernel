//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
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
            Self::AlreadyMapped => write!(f, "virtual address already mapped"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::FrameAllocFailed => write!(f, "physical frame allocation failed"),
        }
    }
}

impl core::error::Error for PagingError {}

/// unmap 结果——区分独占帧和共享帧。
///
/// EXCLUSIVE 检查在 [`PageTable::unmap_page`] 内部完成，
/// 调用者无需接触 unsafe 的 `UnmappedFrames::from_unmapped_range`。
pub enum UnmapResult {
    /// PTE 有 EXCLUSIVE 位——帧已包装为 UnmappedFrames，Drop 自动回收
    Exclusive(frame_allocator::UnmappedFrames),
    /// PTE 无 EXCLUSIVE 位——非独占映射，返回物理地址供 COW 引用计数等使用
    NonExclusive(memory_types::PhysAddr),
}

impl core::fmt::Debug for UnmapResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Exclusive(_) => write!(f, "UnmapResult::Exclusive(...)"),
            Self::NonExclusive(pa) => write!(f, "UnmapResult::NonExclusive({pa})"),
        }
    }
}
