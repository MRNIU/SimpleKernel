//! PageTable 类型 re-export。
//!
//! `paging` crate 的 `PageTable` 现在是非泛型类型（通过 `cfg` 选择具体帧类型），
//! 此模块仅做 re-export 以保持 `memory::node_frame::PageTable` 路径不变。

/// 页表类型——re-export `paging::PageTable`。
pub type PageTable = paging::PageTable;
