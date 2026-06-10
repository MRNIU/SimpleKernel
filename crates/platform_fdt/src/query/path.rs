// Copyright The SimpleKernel Contributors

//! FDT 遍历过程中的节点路径栈。

use crate::FdtError;

use super::{FDT_MAX_DEPTH, FdtNodeName};

pub(super) struct NodePath<'fdt> {
    components: [Option<FdtNodeName<'fdt>>; FDT_MAX_DEPTH],
    depth: usize,
}

impl<'fdt> NodePath<'fdt> {
    pub(super) const fn new() -> Self {
        Self {
            components: [None; FDT_MAX_DEPTH],
            depth: 0,
        }
    }

    /// 记录当前遍历深度上的节点名称。
    ///
    /// # Errors
    ///
    /// `depth == 0` 或超过 [`FDT_MAX_DEPTH`] 时返回错误。
    pub(super) fn push(&mut self, depth: usize, name: FdtNodeName<'fdt>) -> Result<(), FdtError> {
        if depth == 0 || depth > self.components.len() {
            log::warn!("FDT 节点深度超出支持范围: {}", depth);
            return Err(FdtError::UnsupportedLayout);
        }

        self.components[depth - 1] = Some(name);
        self.depth = depth;
        Ok(())
    }

    pub(super) fn matches(&self, path: &str) -> bool {
        if !path.starts_with('/') || path == "/" {
            return false;
        }

        let mut count = 0;
        for (index, component) in path.trim_start_matches('/').split('/').enumerate() {
            count += 1;
            let Some(Some(name)) = self.components.get(index) else {
                return false;
            };
            if !component_matches_name(component, *name) {
                return false;
            }
        }

        count == self.depth
    }
}

fn component_matches_name(component: &str, name: FdtNodeName<'_>) -> bool {
    if let Some((base, unit_address)) = component.split_once('@') {
        name.name == base && name.unit_address == Some(unit_address)
    } else {
        name.name == component
    }
}
