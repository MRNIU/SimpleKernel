// Copyright The SimpleKernel Contributors

use fdt_parser::helpers::UnalignedFallibleNode;
use fdt_parser::properties::Compatible;

use crate::{FdtError, PlatformFdt};

/// 单次查询最多返回的节点数量。
pub const MAX_QUERY_NODES: usize = 16;

/// 单节点最多保留的 `compatible` 字符串数量。
pub const MAX_NODE_COMPATIBLES: usize = 4;

/// 单节点最多保留的 `reg` 区域数量。
pub const MAX_NODE_REGIONS: usize = 4;

/// FDT 查询入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtSelector<'query> {
    /// 按 FDT 绝对路径查询，如 `/cpus`。
    Path(&'query str),
    /// 按 `compatible` 字符串查询。
    Compatible(&'query str),
}

/// 查询结果中的节点序号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdtNodeId {
    ordinal: usize,
}

impl FdtNodeId {
    const fn new(ordinal: usize) -> Self {
        Self { ordinal }
    }

    /// 返回节点在本次查询结果中的序号。
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
    /// 返回节点在本次查询结果中的序号。
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdtNodeList<'fdt> {
    nodes: heapless::Vec<FdtNodeView<'fdt>, MAX_QUERY_NODES>,
}

impl<'fdt> FdtNodeList<'fdt> {
    fn new() -> Self {
        Self {
            nodes: heapless::Vec::new(),
        }
    }

    fn push(&mut self, node: FdtNodeView<'fdt>) -> Result<(), FdtError> {
        self.nodes.push(node).map_err(|_| {
            log::warn!("FDT 查询结果超过 MAX_QUERY_NODES {}", MAX_QUERY_NODES);
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

macro_rules! parse_fdt {
    ($platform_fdt:expr) => {{
        fdt_parser::Fdt::new_unaligned_fallible($platform_fdt.bytes()).map_err(|e| {
            log::warn!(
                "FDT 解析失败 (addr={:#x}): {:?}",
                $platform_fdt.storage_addr(),
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
    pub fn query_nodes(&self, selector: FdtSelector<'_>) -> Result<FdtNodeList<'_>, FdtError> {
        let fdt = parse_fdt!(self)?;
        let mut list = FdtNodeList::new();

        match selector {
            FdtSelector::Path(path) => {
                let Some(node) = fdt.find_node(path).map_err(|e| {
                    log::warn!("FDT 查找路径 {} 失败: {:?}", path, e);
                    FdtError::ParseFailed
                })?
                else {
                    return Ok(list);
                };
                list.push(node_to_view(node, FdtNodeId::new(0), None, "path")?)?;
            }
            FdtSelector::Compatible(compatible) => {
                let nodes = fdt.all_nodes().map_err(|e| {
                    log::warn!("FDT 遍历节点以查询 compatible={} 失败: {:?}", compatible, e);
                    FdtError::ParseFailed
                })?;

                let mut ordinal = 0;
                for node_result in nodes {
                    let (_, node) = node_result.map_err(|e| {
                        log::warn!("FDT compatible={} 节点解析失败: {:?}", compatible, e);
                        FdtError::ParseFailed
                    })?;
                    if !node_matches_compatible(&node, compatible, compatible)? {
                        continue;
                    }
                    list.push(node_to_view(
                        node,
                        FdtNodeId::new(ordinal),
                        Some(compatible),
                        compatible,
                    )?)?;
                    ordinal += 1;
                }
            }
        }

        Ok(list)
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
            let view = node_to_view(node, FdtNodeId::new(0), Some(compatible), compatible)?;
            return view.reg_nth(reg_index).ok_or(FdtError::PropertyNotFound);
        }

        Err(FdtError::NodeNotFound)
    }

    /// 返回 `/cpus/cpu*` 节点声明的硬件 CPU id 列表。
    ///
    /// RISC-V 下该值对应 hart id；AArch64 下该值对应 MPIDR affinity 编码。
    /// 当前调用方只用它校验平台满足 dense CPU id 契约。
    ///
    /// # Errors
    /// 当 FDT 根节点或 `/cpus` 节点解析失败、CPU `reg` 属性缺失/无效、
    /// CPU id 重复，或 CPU 数量超过 [`config::MAX_CORE_COUNT`] 时返回错误。
    pub fn cpu_hardware_ids(
        &self,
    ) -> Result<heapless::Vec<usize, { config::MAX_CORE_COUNT }>, FdtError> {
        let fdt = parse_fdt!(self)?;
        let root = fdt.root().map_err(|e| {
            log::warn!("FDT root 节点解析失败: {:?}", e);
            FdtError::ParseFailed
        })?;
        let cpus = root.cpus().map_err(|e| {
            log::warn!("FDT cpus 节点未找到: {:?}", e);
            FdtError::NodeNotFound
        })?;
        let iter = cpus.iter().map_err(|e| {
            log::warn!("FDT cpus 迭代失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        let mut ids = heapless::Vec::<usize, { config::MAX_CORE_COUNT }>::new();
        for cpu_result in iter {
            let cpu = cpu_result.map_err(|e| {
                log::warn!("FDT cpu 节点解析失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            let reg = cpu.reg::<u64>().map_err(|e| {
                log::warn!("FDT cpu reg 属性解析失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            let hardware_id = reg.first().map_err(|e| {
                log::warn!("FDT cpu reg id 收集失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            let hardware_id = usize::try_from(hardware_id).map_err(|_| {
                log::warn!("FDT cpu hardware id 超出 usize: {}", hardware_id);
                FdtError::UnsupportedLayout
            })?;
            if ids.contains(&hardware_id) {
                log::warn!("FDT cpu hardware id 重复: {}", hardware_id);
                return Err(FdtError::UnsupportedLayout);
            }
            ids.push(hardware_id).map_err(|_| {
                log::warn!("FDT cpu 数量超过 MAX_CORE_COUNT {}", config::MAX_CORE_COUNT);
                FdtError::UnsupportedLayout
            })?;
        }

        if ids.is_empty() {
            return Err(FdtError::NodeNotFound);
        }
        Ok(ids)
    }

    /// 返回当前支持的单段 RAM 起点和长度。
    ///
    /// # Errors
    /// 当 `/memory` 节点缺失、`reg` 属性非法，或平台声明多段 RAM 时返回错误。
    pub fn memory(&self) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self)?;
        let root = fdt.root().map_err(|e| {
            log::warn!("FDT root 节点解析失败: {:?}", e);
            FdtError::ParseFailed
        })?;
        let memory = root.memory().map_err(|e| {
            log::warn!("FDT memory 节点未找到: {:?}", e);
            FdtError::NodeNotFound
        })?;
        let mut regions = memory
            .reg()
            .map_err(|e| {
                log::warn!("FDT memory reg 属性未找到: {:?}", e);
                FdtError::PropertyNotFound
            })?
            .iter::<u64, usize>();
        let region = regions.next().ok_or(FdtError::NodeNotFound)?.map_err(|e| {
            log::warn!("FDT memory region 解析失败: {:?}", e);
            FdtError::ParseFailed
        })?;
        if let Some(second) = regions.next() {
            let second = second.map_err(|e| {
                log::warn!("FDT 第二段 memory region 解析失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            log::warn!(
                "FDT memory reg 包含多段 RAM：第一段 addr={:#x}, size={:#x}；第二段 addr={:#x}, size={:#x}。当前只支持单段 RAM，拒绝静默忽略后续 bank",
                region.address,
                region.len,
                second.address,
                second.len
            );
            return Err(FdtError::UnsupportedLayout);
        }
        Ok((region.address, region.len))
    }

    /// 从 `/reserved-memory` 查找固件/bootloader 保留区。
    ///
    /// 当前匹配两类描述：
    /// - 节点名为 `firmware@...` / `bootloader@...` / `opensbi@...` / `u-boot@...`
    /// - `compatible` 包含 `simplekernel,firmware-reserved`
    ///
    /// QEMU 原生 DTB 通常不包含该节点，`xtask` 会在导出 DTB 后补充
    /// `firmware@...` 节点；非 QEMU 平台可由 bootloader 直接提供同等节点。
    ///
    /// # Errors
    /// 当 `/reserved-memory` 缺失、没有匹配节点、匹配节点 `reg` 非法，
    /// 或固件保留区包含多段 region 时返回错误。
    pub fn firmware_reserved_memory(&self) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self)?;
        let root = fdt.root().map_err(|e| {
            log::warn!("FDT root 节点解析失败: {:?}", e);
            FdtError::ParseFailed
        })?;
        let reserved = root.reserved_memory().map_err(|e| {
            log::debug!("FDT /reserved-memory 节点未找到或解析失败: {:?}", e);
            FdtError::NodeNotFound
        })?;
        let children = reserved.children().map_err(|e| {
            log::warn!("FDT /reserved-memory 子节点迭代失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        for child_result in children {
            let child = child_result.map_err(|e| {
                log::warn!("FDT /reserved-memory 子节点解析失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            let name = child.name().map_err(|e| {
                log::warn!("FDT /reserved-memory 子节点名称解析失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            let compatible_match = child
                .compatible()
                .map_err(|e| {
                    log::warn!("FDT /reserved-memory/{} compatible 解析失败: {:?}", name, e);
                    FdtError::ParseFailed
                })?
                .is_some_and(|compatible| {
                    compatible.compatible_with("simplekernel,firmware-reserved")
                });
            let name_match = matches!(name.name, "firmware" | "bootloader" | "opensbi" | "u-boot");
            if !compatible_match && !name_match {
                continue;
            }

            let reg = child
                .reg()
                .map_err(|e| {
                    log::warn!("FDT /reserved-memory/{} reg 解析失败: {:?}", name, e);
                    FdtError::ParseFailed
                })?
                .ok_or(FdtError::PropertyNotFound)?;
            let mut regions = reg.iter::<u64, usize>();
            let region = regions.next().ok_or(FdtError::NodeNotFound)?.map_err(|e| {
                log::warn!("FDT /reserved-memory/{} region 解析失败: {:?}", name, e);
                FdtError::ParseFailed
            })?;
            if regions.next().is_some() {
                log::warn!(
                    "FDT /reserved-memory/{} 包含多段 region；当前固件保留区只支持单段",
                    name
                );
                return Err(FdtError::UnsupportedLayout);
            }
            return Ok((region.address, region.len));
        }

        Err(FdtError::NodeNotFound)
    }

    /// 从 FDT `/cpus` 节点读取 `timebase-frequency` 属性。
    ///
    /// RISC-V 平台必须提供此属性；AArch64 的 FDT 通常不含此属性，
    /// 返回 `Err` 后由调用方回退到 `CNTFRQ_EL0`。
    ///
    /// # Errors
    /// 当 `/cpus` 节点缺失、属性缺失或属性长度不是 4 字节时返回错误。
    pub fn timebase_frequency(&self) -> Result<u32, FdtError> {
        let fdt = parse_fdt!(self)?;
        let cpus = fdt
            .find_node("/cpus")
            .map_err(|e| {
                log::warn!("FDT 查找 /cpus 节点失败: {:?}", e);
                FdtError::ParseFailed
            })?
            .ok_or(FdtError::NodeNotFound)?;
        let prop = cpus
            .raw_property("timebase-frequency")
            .map_err(|e| {
                log::warn!("FDT 读取 timebase-frequency 属性失败: {:?}", e);
                FdtError::ParseFailed
            })?
            .ok_or(FdtError::PropertyNotFound)?;
        let bytes: [u8; 4] = prop.value.try_into().map_err(|e| {
            log::warn!(
                "FDT timebase-frequency 属性大小不匹配 (len={}): {:?}",
                prop.value.len(),
                e
            );
            FdtError::InvalidPropertySize
        })?;
        Ok(u32::from_be_bytes(bytes))
    }

    /// 返回 FDT 中的节点总数。
    ///
    /// # Errors
    /// 当遍历任何节点失败时返回错误。
    pub fn node_count(&self) -> Result<usize, FdtError> {
        let fdt = parse_fdt!(self)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历所有节点失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        let mut count = 0;
        for node_result in nodes {
            node_result.map_err(|e| {
                log::warn!("FDT 节点计数时解析节点失败: {:?}", e);
                FdtError::ParseFailed
            })?;
            count += 1;
        }
        Ok(count)
    }
}

fn node_to_view<'fdt>(
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

    Ok(FdtNodeView {
        id,
        name,
        matched_compatible,
        compatibles,
        regs,
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

fn node_matches_compatible<'fdt>(
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
