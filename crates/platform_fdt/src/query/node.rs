// Copyright The SimpleKernel Contributors

//! FDT 节点到内核查询视图的转换逻辑。

use fdt_parser::helpers::UnalignedFallibleNode;
use fdt_parser::properties::Compatible;

use crate::FdtError;

use super::{
    FdtCompatibleList, FdtNodeId, FdtNodeName, FdtNodeView, FdtReg, MAX_NODE_COMPATIBLES,
    MAX_NODE_REGIONS,
};

/// 将 parser 节点转换为稳定查询视图。
///
/// # Errors
///
/// 节点名称、compatible 或 reg 属性解析失败，或本 crate 固定容量不足时返回错误。
pub(super) fn node_to_view<'fdt>(
    node: UnalignedFallibleNode<'fdt>,
    id: FdtNodeId,
    matched_compatible: Option<&str>,
    context: &str,
) -> Result<FdtNodeView<'fdt>, FdtError> {
    let name = node.name().map_err(|e| {
        log::warn!("FDT {} 节点名称解析失败: {:?}", context, e);
        FdtError::ParseFailed
    })?;
    let name = FdtNodeName {
        name: name.name,
        unit_address: name.unit_address,
    };
    let compatibles = collect_compatibles(&node, name, context)?;
    let matched_compatible = matched_compatible
        .and_then(|requested| compatibles.iter().copied().find(|item| *item == requested));
    let regs = collect_regs(&node, name, context)?;

    Ok(FdtNodeView::new(
        id,
        name,
        matched_compatible,
        compatibles,
        regs,
    ))
}

/// 读取节点名称并转换为轻量视图。
///
/// # Errors
///
/// parser 无法解析节点名称时返回错误。
pub(super) fn node_name<'fdt>(
    node: &UnalignedFallibleNode<'fdt>,
    context: &str,
) -> Result<FdtNodeName<'fdt>, FdtError> {
    let name = node.name().map_err(|e| {
        log::warn!("FDT {} 节点名称解析失败: {:?}", context, e);
        FdtError::ParseFailed
    })?;
    Ok(FdtNodeName {
        name: name.name,
        unit_address: name.unit_address,
    })
}

fn collect_compatibles<'fdt>(
    node: &UnalignedFallibleNode<'fdt>,
    name: FdtNodeName<'fdt>,
    context: &str,
) -> Result<FdtCompatibleList<'fdt>, FdtError> {
    let mut compatibles = FdtCompatibleList::new();
    let Some(compatible) = node.property::<Compatible<'fdt>>().map_err(|e| {
        log::warn!(
            "FDT {} 节点 {} compatible 解析失败: {:?}",
            context,
            name.name,
            e
        );
        FdtError::ParseFailed
    })?
    else {
        return Ok(compatibles);
    };

    for item in compatible.all().filter(|item| !item.is_empty()) {
        compatibles.push(item).map_err(|overflow_item| {
            log::warn!(
                "FDT {} 节点 {} compatible 数量超过 {}: rejected={}",
                context,
                name.name,
                MAX_NODE_COMPATIBLES,
                overflow_item
            );
            FdtError::UnsupportedLayout
        })?;
    }
    Ok(compatibles)
}

/// 判断节点是否包含目标 compatible 字符串。
///
/// # Errors
///
/// compatible 属性存在但 parser 无法解析时返回错误。
pub(super) fn node_matches_compatible<'fdt>(
    node: &UnalignedFallibleNode<'fdt>,
    compatible: &str,
    context: &str,
) -> Result<bool, FdtError> {
    let Some(node_compatible) = node.property::<Compatible<'fdt>>().map_err(|e| {
        log::warn!("FDT {} compatible 解析失败: {:?}", context, e);
        FdtError::ParseFailed
    })?
    else {
        return Ok(false);
    };

    Ok(node_compatible.compatible_with(compatible))
}

fn collect_regs<'fdt>(
    node: &UnalignedFallibleNode<'fdt>,
    name: FdtNodeName<'fdt>,
    context: &str,
) -> Result<heapless::Vec<FdtReg, MAX_NODE_REGIONS>, FdtError> {
    let mut regs = heapless::Vec::new();
    let Some(reg) = node.reg().map_err(|e| {
        log::warn!("FDT {} 节点 {} reg 解析失败: {:?}", context, name.name, e);
        FdtError::ParseFailed
    })?
    else {
        return Ok(regs);
    };

    for region in reg.iter::<u64, usize>() {
        let region = region.map_err(|e| {
            log::warn!(
                "FDT {} 节点 {} reg entry 解析失败: {:?}",
                context,
                name.name,
                e
            );
            FdtError::ParseFailed
        })?;
        regs.push(FdtReg {
            address: region.address,
            size: region.len,
        })
        .map_err(|overflow_region| {
            log::warn!(
                "FDT {} 节点 {} reg 区域数量超过 {}: rejected_addr={:#x}, rejected_size={:#x}",
                context,
                name.name,
                MAX_NODE_REGIONS,
                overflow_region.address,
                overflow_region.size
            );
            FdtError::UnsupportedLayout
        })?;
    }
    Ok(regs)
}
