// Copyright The SimpleKernel Contributors

use fdt_parser::helpers::UnalignedFallibleNode;
use fdt_parser::properties::Compatible;

use crate::FdtError;

use super::{
    FdtCompatibleList, FdtNodeId, FdtNodeName, FdtNodeView, FdtReg, MAX_NODE_COMPATIBLES,
    MAX_NODE_REGIONS,
};

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
        compatibles.push(item).map_err(|_| {
            log::warn!(
                "FDT {} 节点 {} compatible 数量超过 {}",
                context,
                name.name,
                MAX_NODE_COMPATIBLES
            );
            FdtError::UnsupportedLayout
        })?;
    }
    Ok(compatibles)
}

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
        .map_err(|_| {
            log::warn!(
                "FDT {} 节点 {} reg 区域数量超过 {}",
                context,
                name.name,
                MAX_NODE_REGIONS
            );
            FdtError::UnsupportedLayout
        })?;
    }
    Ok(regs)
}
