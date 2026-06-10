// Copyright The SimpleKernel Contributors

//! 启动期 DTB 生命周期管理。
//!
//! 本 crate 负责把 bootloader 传入的 DTB 从外部 blob 复制到内核自有、
//! 页对齐的固定 storage，并提供平台无关的 FDT 查询 API。

#![cfg_attr(not(test), no_std)]
#![feature(sync_unsafe_cell)]

mod error;
mod query;
mod storage;

pub use error::FdtError;
pub use query::{
    FdtCompatibleList, FdtNodeId, FdtNodeList, FdtNodeName, FdtNodeView, FdtReg, FdtSelector,
    MAX_NODE_COMPATIBLES, MAX_NODE_REGIONS, MAX_QUERY_NODES,
};
pub use storage::{MAX_DTB_SIZE, PlatformFdt, StorageRegion, get, init_from_raw, storage_region};
