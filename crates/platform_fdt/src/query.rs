// Copyright The SimpleKernel Contributors

//! FDT 查询子模块入口。

mod node;
mod path;
mod platform;
mod properties;
mod types;

pub use types::{
    FdtCompatibleList, FdtNodeId, FdtNodeList, FdtNodeName, FdtNodeView, FdtReg, FdtSelector,
};

pub(super) const FDT_ROOT_NODE_ID: usize = 0;
