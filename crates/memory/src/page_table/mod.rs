//! 多级页表与架构原生 PTE 标志位。
//!
//! 核心实现位于独立的 `page_table` crate，本模块提供：
//! - 所有公共类型的 re-export（保持 `memory::page_table::*` 路径不变）
//! - 裸机环境下 `NodeFrameOps` 的具体实现（`AllocatedFrames`）
//! - 类型别名 `PageTable`——隐藏泛型参数

// Re-export 所有公共类型——保持 `memory::page_table::*` 路径不变
pub use page_table_crate::error::PageTableError;
pub use page_table_crate::{
    ENTRIES_PER_TABLE, LEVEL_INFO, LevelInfo, PageTableEntry, PteFlags, PteFlagsOps, PteOps,
    page_size_at_level,
};

#[cfg(any(test, target_os = "none"))]
pub use page_table_crate::NodeFrameOps;

// 裸机：为 AllocatedFrames 实现 NodeFrameOps
#[cfg(target_os = "none")]
impl page_table_crate::NodeFrameOps for crate::frame::AllocatedFrames {
    fn alloc() -> Result<Self, PageTableError> {
        Self::alloc_one().map_err(|_| PageTableError::AllocationFailed)
    }
    fn paddr(&self) -> address::PhysAddr {
        self.start_paddr()
    }
}

/// 具体化的页表类型——隐藏泛型参数 `F`。
///
/// - 裸机：`PageTable<AllocatedFrames>`（物理帧分配器）
/// - 测试：`PageTable<HeapNodeFrame>`（堆分配模拟）
#[cfg(target_os = "none")]
pub type PageTable = page_table_crate::PageTable<crate::frame::AllocatedFrames>;
#[cfg(test)]
pub type PageTable = page_table_crate::PageTable<page_table_crate::HeapNodeFrame>;

impl From<PageTableError> for crate::error::MemoryError {
    fn from(e: PageTableError) -> Self {
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped
            | PageTableError::HugePageConflict
            | PageTableError::InvalidRange => Self::MapFailed,
            PageTableError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
