// Copyright The SimpleKernel Contributors

//! FDT 查询子模块入口。

mod node;
mod path;
mod platform;
mod properties;
mod types;

pub use types::{
    FdtCompatibleList, FdtNodeId, FdtNodeList, FdtNodeName, FdtNodeView, FdtReg, FdtSelector,
    MAX_NODE_COMPATIBLES, MAX_NODE_REGIONS, MAX_QUERY_NODES,
};

pub(super) const FDT_ROOT_NODE_ID: usize = 0;
pub(super) const FDT_MAX_DEPTH: usize = 16;
