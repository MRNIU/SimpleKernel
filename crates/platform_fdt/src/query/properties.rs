// Copyright The SimpleKernel Contributors

use crate::{FdtError, PlatformFdt};

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
