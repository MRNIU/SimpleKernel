// Copyright The SimpleKernel Contributors

use heapless::Vec;
use platform_fdt::FdtNodeId;

pub(super) struct FdtNodeSet<const N: usize> {
    nodes: Vec<FdtNodeId, N>,
}

impl<const N: usize> FdtNodeSet<N> {
    pub(super) const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub(super) fn contains(&self, node_id: FdtNodeId) -> bool {
        self.nodes.contains(&node_id)
    }

    pub(super) fn insert(&mut self, node_id: FdtNodeId, driver_name: &'static str) {
        if self.contains(node_id) {
            return;
        }
        self.nodes.push(node_id).unwrap_or_else(|_| {
            panic!(
                "PlatformBus: driver={} FDT node set 超出容量: capacity={}",
                driver_name, N
            )
        });
    }
}
