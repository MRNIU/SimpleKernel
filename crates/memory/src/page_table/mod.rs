//! 多级页表与架构原生 PTE 标志位。
//!
//! 核心实现位于独立的 `page_table` crate，本模块提供：
//! - 所有公共类型的 re-export（保持 `memory::page_table::*` 路径不变）
//! - 裸机环境下 `NodeFrameOps` 的具体实现（[`NodeFrame`] newtype）
//! - 类型别名 `PageTable`——隐藏泛型参数

pub use page_table_crate::error::PageTableError;
pub use page_table_crate::{
    ENTRIES_PER_TABLE, LEVEL_INFO, LevelInfo, PageTableEntry, PteFlags, PteFlagsOps, PteOps,
    page_size_at_level,
};

#[cfg(any(test, target_os = "none"))]
pub use page_table_crate::NodeFrameOps;

/// 页表节点帧——包装 `AllocatedFrames` 以实现 `NodeFrameOps`。
///
/// Newtype 存在的原因：`AllocatedFrames` 定义在 `frame_allocator` crate，
/// `NodeFrameOps` 定义在 `page_table` crate，孤儿规则禁止在第三方 crate
/// 为两个外部类型实现 trait。`NodeFrame` 是 `memory` crate 的本地类型，
/// 绕过此限制，同时保持 `frame_allocator` 和 `page_table` 互不依赖。
#[cfg(target_os = "none")]
pub struct NodeFrame(frame_allocator::AllocatedFrames);

#[cfg(target_os = "none")]
impl page_table_crate::NodeFrameOps for NodeFrame {
    fn alloc() -> Result<Self, PageTableError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|_| PageTableError::AllocationFailed)
    }
    fn paddr(&self) -> address::PhysAddr {
        self.0.start_paddr()
    }
}

/// 具体化的页表类型——隐藏泛型参数 `F`。
///
/// - 裸机：`PageTable<NodeFrame>`（物理帧分配器）
/// - 测试：`PageTable<HeapNodeFrame>`（堆分配模拟）
#[cfg(target_os = "none")]
pub type PageTable = page_table_crate::PageTable<NodeFrame>;
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
