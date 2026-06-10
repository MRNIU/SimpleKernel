// Copyright The SimpleKernel Contributors

//! `PlatformFdt` 的节点查询实现。

use fdt_parser::nodes::AsNode;

use crate::{FdtError, PlatformFdt};

use super::node::{node_matches_compatible, node_name, node_to_view};
use super::path::NodePath;
use super::{FDT_ROOT_NODE_ID, FdtNodeId, FdtNodeList, FdtNodeView, FdtReg, FdtSelector};

macro_rules! parse_fdt {
    ($platform_fdt:expr) => {{
        fdt_parser::Fdt::new_unaligned_fallible($platform_fdt.bytes()).map_err(|e| {
            log::warn!(
                "FDT 解析失败: storage_addr={:#x}, totalsize={:#x}, error={:?}",
                $platform_fdt.storage_addr(),
                $platform_fdt.total_size(),
                e
            );
            FdtError::InvalidHeader
        })
    }};
}

impl PlatformFdt {
    /// 查询 FDT 节点。
    ///
    /// `Compatible` 查询没有匹配时返回空列表，不视为错误；已匹配节点的属性解析失败
    /// 会返回错误，避免启动流程静默跳过坏平台描述。
    ///
    /// # Errors
    /// 当 FDT 自身解析失败、匹配节点解析失败、节点属性非法，或结果超过固定容量时返回错误。
    pub fn query_nodes(&self, selector: FdtSelector<'_>) -> Result<FdtNodeList<'static>, FdtError> {
        let mut list = FdtNodeList::new();
        self.visit_nodes(selector, |node| list.push(node))?;
        Ok(list)
    }

    /// 流式访问匹配的 FDT 节点。
    ///
    /// 与 [`query_nodes`](Self::query_nodes) 不同，本方法不把全部匹配结果收集到固定容量
    /// `FdtNodeList` 中，适合用于 VirtIO MMIO 这类不同架构可声明不同实例数量的设备枚举。
    ///
    /// # Errors
    ///
    /// 当 FDT 自身解析失败、匹配节点解析失败、节点属性非法，或回调返回错误时返回错误。
    pub fn visit_nodes(
        &self,
        selector: FdtSelector<'_>,
        mut visitor: impl FnMut(FdtNodeView<'static>) -> Result<(), FdtError>,
    ) -> Result<(), FdtError> {
        let fdt = parse_fdt!(self)?;

        match selector {
            FdtSelector::Path(path) => {
                if path == "/" {
                    let root = fdt.root().map_err(|e| {
                        log::warn!("FDT root 节点解析失败: {:?}", e);
                        FdtError::ParseFailed
                    })?;
                    visitor(node_to_view(
                        root.as_node(),
                        FdtNodeId::from_stable_ordinal(FDT_ROOT_NODE_ID),
                        None,
                        "path",
                    )?)?;
                    return Ok(());
                }

                let nodes = fdt.all_nodes().map_err(|e| {
                    log::warn!("FDT 遍历节点以查询路径 {} 失败: {:?}", path, e);
                    FdtError::ParseFailed
                })?;
                let mut node_path = NodePath::new();
                for (tree_ordinal, node_result) in nodes.enumerate() {
                    let (depth, node) = node_result.map_err(|e| {
                        log::warn!("FDT path={} 节点解析失败: {:?}", path, e);
                        FdtError::ParseFailed
                    })?;
                    node_path.push(depth, node_name(&node, path)?)?;
                    if !node_path.matches(path) {
                        continue;
                    }

                    visitor(node_to_view(
                        node,
                        FdtNodeId::from_stable_ordinal(tree_ordinal + FDT_ROOT_NODE_ID + 1),
                        None,
                        "path",
                    )?)?;
                    break;
                }
            }
            FdtSelector::Compatible(compatible) => {
                let nodes = fdt.all_nodes().map_err(|e| {
                    log::warn!("FDT 遍历节点以查询 compatible={} 失败: {:?}", compatible, e);
                    FdtError::ParseFailed
                })?;

                for (tree_ordinal, node_result) in nodes.enumerate() {
                    let (_, node) = node_result.map_err(|e| {
                        log::warn!("FDT compatible={} 节点解析失败: {:?}", compatible, e);
                        FdtError::ParseFailed
                    })?;
                    if !node_matches_compatible(&node, compatible, compatible)? {
                        continue;
                    }
                    visitor(node_to_view(
                        node,
                        FdtNodeId::from_stable_ordinal(tree_ordinal + FDT_ROOT_NODE_ID + 1),
                        Some(compatible),
                        compatible,
                    )?)?;
                }
            }
        }

        Ok(())
    }

    /// 返回首个匹配 `compatible` 节点的指定 `reg` 区域。
    ///
    /// # Errors
    /// 当没有匹配节点、匹配节点缺少指定 `reg` 区域，或节点属性非法时返回错误。
    pub fn compatible_reg(&self, compatible: &str, reg_index: usize) -> Result<FdtReg, FdtError> {
        let fdt = parse_fdt!(self)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历节点以查询 compatible={} 失败: {:?}", compatible, e);
            FdtError::ParseFailed
        })?;

        for node_result in nodes {
            let (_, node) = node_result.map_err(|e| {
                log::warn!("FDT compatible={} 节点解析失败: {:?}", compatible, e);
                FdtError::ParseFailed
            })?;
            if !node_matches_compatible(&node, compatible, compatible)? {
                continue;
            }
            let view = node_to_view(
                node,
                FdtNodeId::from_stable_ordinal(0),
                Some(compatible),
                compatible,
            )?;
            return view.reg_nth(reg_index).ok_or(FdtError::PropertyNotFound);
        }

        Err(FdtError::NodeNotFound)
    }
}
