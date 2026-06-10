// Copyright The SimpleKernel Contributors

//! FDT 查询返回值与选择器类型。

use crate::FdtError;

use config::{MAX_NODE_COMPATIBLES, MAX_NODE_REGIONS, MAX_QUERY_NODES};

/// FDT 查询入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtSelector<'query> {
    /// 按 FDT 绝对路径查询，如 `/cpus`。
    Path(&'query str),
    /// 按 `compatible` 字符串查询。
    Compatible(&'query str),
}

/// 同一 DTB view 内稳定的节点序号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdtNodeId {
    ordinal: usize,
}

impl FdtNodeId {
    /// 从全树 DFS 遍历序号构造节点 id。
    ///
    /// 该构造函数主要供 `platform_fdt` 查询层和静态测试 fixture 使用；真实设备 probe
    /// 应优先复用 [`FdtNodeView::id`] 返回的值。
    pub const fn from_stable_ordinal(ordinal: usize) -> Self {
        Self { ordinal }
    }

    /// 返回节点在同一 DTB view 全树 DFS 遍历中的稳定序号。
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }
}

/// FDT 节点名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdtNodeName<'fdt> {
    /// `@` 前面的节点名。
    pub name: &'fdt str,
    /// `@` 后面的 unit address。
    pub unit_address: Option<&'fdt str>,
}

/// FDT `reg` 区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdtReg {
    /// MMIO 或内存区域起始物理地址。
    pub address: u64,
    /// 区域长度。
    pub size: usize,
}

/// 单节点 `compatible` 字符串列表。
pub type FdtCompatibleList<'fdt> = heapless::Vec<&'fdt str, MAX_NODE_COMPATIBLES>;

/// 单个 FDT 节点的轻量借用视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdtNodeView<'fdt> {
    id: FdtNodeId,
    name: FdtNodeName<'fdt>,
    matched_compatible: Option<&'fdt str>,
    compatibles: FdtCompatibleList<'fdt>,
    regs: heapless::Vec<FdtReg, MAX_NODE_REGIONS>,
}

impl<'fdt> FdtNodeView<'fdt> {
    pub(crate) fn new(
        id: FdtNodeId,
        name: FdtNodeName<'fdt>,
        matched_compatible: Option<&'fdt str>,
        compatibles: FdtCompatibleList<'fdt>,
        regs: heapless::Vec<FdtReg, MAX_NODE_REGIONS>,
    ) -> Self {
        Self {
            id,
            name,
            matched_compatible,
            compatibles,
            regs,
        }
    }

    /// 返回节点在同一 DTB view 全树 DFS 遍历中的稳定序号。
    pub const fn id(&self) -> FdtNodeId {
        self.id
    }

    /// 返回节点名。
    pub const fn name(&self) -> FdtNodeName<'fdt> {
        self.name
    }

    /// 返回命中本节点的 `compatible` 字符串。
    pub const fn matched_compatible(&self) -> Option<&'fdt str> {
        self.matched_compatible
    }

    /// 返回节点声明的 `compatible` 列表。
    pub fn compatibles(&self) -> &[&'fdt str] {
        self.compatibles.as_slice()
    }

    /// 返回节点声明的所有 `reg` 区域。
    pub fn regs(&self) -> &[FdtReg] {
        self.regs.as_slice()
    }

    /// 返回第一个 `reg` 区域。
    pub fn reg(&self) -> Option<FdtReg> {
        self.reg_nth(0)
    }

    /// 返回指定序号的 `reg` 区域。
    pub fn reg_nth(&self, index: usize) -> Option<FdtReg> {
        self.regs.get(index).copied()
    }

    /// 返回第一个 `reg` 区域；缺失时返回结构化错误。
    ///
    /// # Errors
    /// 当节点没有 `reg` 属性或 `reg` 为空时返回 [`FdtError::PropertyNotFound`]。
    pub fn reg_required(&self) -> Result<FdtReg, FdtError> {
        self.reg().ok_or(FdtError::PropertyNotFound)
    }
}

/// FDT 查询结果。
///
/// 该类型是小结果集的 bounded snapshot；需要遍历数量由平台决定的 compatible 节点时，
/// 优先使用 [`crate::PlatformFdt::visit_nodes`]，避免把平台实例数量写进固定栈容量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdtNodeList<'fdt> {
    nodes: heapless::Vec<FdtNodeView<'fdt>, MAX_QUERY_NODES>,
}

impl<'fdt> FdtNodeList<'fdt> {
    pub(crate) fn new() -> Self {
        Self {
            nodes: heapless::Vec::new(),
        }
    }

    /// 追加一个查询命中节点。
    ///
    /// # Errors
    ///
    /// 查询结果超过 [`MAX_QUERY_NODES`] 时返回 [`FdtError::UnsupportedLayout`]。
    pub(crate) fn push(&mut self, node: FdtNodeView<'fdt>) -> Result<(), FdtError> {
        self.nodes.push(node).map_err(|overflow_node| {
            let name = overflow_node.name();
            log::warn!(
                "FDT 查询结果超过 MAX_QUERY_NODES {}: rejected_node_id={}, rejected_name={}@{}",
                MAX_QUERY_NODES,
                overflow_node.id().ordinal(),
                name.name,
                name.unit_address.unwrap_or("<none>")
            );
            FdtError::UnsupportedLayout
        })
    }

    /// 返回匹配节点数量。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 返回查询结果是否为空。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 返回第一个匹配节点。
    pub fn first(&self) -> Option<&FdtNodeView<'fdt>> {
        self.nodes.first()
    }

    /// 迭代所有匹配节点。
    pub fn iter(&self) -> core::slice::Iter<'_, FdtNodeView<'fdt>> {
        self.nodes.iter()
    }
}
