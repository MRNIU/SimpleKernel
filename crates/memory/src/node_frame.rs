//! NodeFrame 桥接与 PageTable 类型别名。
//!
//! `NodeFrame` 是 `memory` crate 的本地 newtype，桥接
//! `frame_allocator::AllocatedFrames`（外部类型）和
//! `paging::NodeFrameOps`（外部 trait），绕过孤儿规则。

/// 页表节点帧——包装 `AllocatedFrames` 以实现 `NodeFrameOps`。
///
/// Newtype 存在的原因：`AllocatedFrames` 定义在 `frame_allocator` crate，
/// `NodeFrameOps` 定义在 `paging` crate，孤儿规则禁止在第三方 crate
/// 为两个外部类型实现 trait。`NodeFrame` 是 `memory` crate 的本地类型，
/// 绕过此限制，同时保持 `frame_allocator` 和 `paging` 互不依赖。
#[cfg(target_os = "none")]
pub struct NodeFrame(frame_allocator::AllocatedFrames);

#[cfg(target_os = "none")]
impl paging::NodeFrameOps for NodeFrame {
    fn alloc() -> Result<Self, paging::error::PageTableError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|_| paging::error::PageTableError::AllocationFailed)
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
pub type PageTable = paging::PageTable<NodeFrame>;
#[cfg(test)]
pub type PageTable = paging::PageTable<paging::HeapNodeFrame>;

impl From<paging::error::PageTableError> for crate::error::MemoryError {
    fn from(e: paging::error::PageTableError) -> Self {
        use paging::error::PageTableError;
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped
            | PageTableError::HugePageConflict
            | PageTableError::InvalidRange => Self::MapFailed,
            PageTableError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
