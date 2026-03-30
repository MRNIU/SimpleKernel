//! NodeFrame 桥接与 PageTable 类型别名。
//!
//! `NodeFrame` 是 `memory` crate 的本地 newtype，桥接
//! `frame_allocator::AllocatedFrames`（外部类型）和
//! `page_table::NodeFrameOps`（外部 trait），绕过孤儿规则。

/// 页表节点帧——包装 `AllocatedFrames` 以实现 `NodeFrameOps`。
///
/// Newtype 存在的原因：`AllocatedFrames` 定义在 `frame_allocator` crate，
/// `NodeFrameOps` 定义在 `page_table` crate，孤儿规则禁止在第三方 crate
/// 为两个外部类型实现 trait。`NodeFrame` 是 `memory` crate 的本地类型，
/// 绕过此限制，同时保持 `frame_allocator` 和 `page_table` 互不依赖。
#[cfg(target_os = "none")]
pub struct NodeFrame(frame_allocator::AllocatedFrames);

#[cfg(target_os = "none")]
impl page_table::NodeFrameOps for NodeFrame {
    fn alloc() -> Result<Self, page_table::error::PageTableError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|_| page_table::error::PageTableError::AllocationFailed)
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
pub type PageTable = page_table::PageTable<NodeFrame>;
#[cfg(test)]
pub type PageTable = page_table::PageTable<page_table::HeapNodeFrame>;

impl From<page_table::error::PageTableError> for crate::error::MemoryError {
    fn from(e: page_table::error::PageTableError) -> Self {
        use page_table::error::PageTableError;
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped
            | PageTableError::HugePageConflict
            | PageTableError::InvalidRange => Self::MapFailed,
            PageTableError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
