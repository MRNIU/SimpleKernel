// Copyright The SimpleKernel Contributors

use core::fmt;
use core::marker::PhantomData;

/// FDT 解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdtError {
    /// FDT 头部无效
    InvalidHeader,
    /// 找不到所需节点
    NodeNotFound,
    /// 找不到所需属性
    PropertyNotFound,
    /// FDT 解析失败
    ParseFailed,
    /// 属性大小不匹配
    InvalidPropertySize,
    /// 当前不支持该硬件布局
    UnsupportedLayout,
}

impl fmt::Display for FdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for FdtError {}

/// 内核自有 FDT 副本基地址（`early_init` 中初始化，中断子系统解析 PLIC/GIC 时使用）
pub static FDT_ADDR: spin::Once<usize> = spin::Once::new();

#[derive(Debug)]
pub struct KernelFdt<'a> {
    fdt_addr: usize,
    _marker: PhantomData<&'a [u8]>,
}

macro_rules! parse_fdt {
    ($addr:expr) => {{
        // SAFETY: fdt_addr 已在 KernelFdt::new() 中校验
        unsafe { fdt_parser::Fdt::from_ptr_unaligned_fallible($addr as *const u8) }.map_err(|e| {
            log::warn!("FDT 解析失败 (addr={:#x}): {:?}", $addr, e);
            FdtError::InvalidHeader
        })
    }};
}

impl<'a> KernelFdt<'a> {
    pub fn new(fdt_addr: usize) -> Result<Self, FdtError> {
        // SAFETY: fdt_addr 由调用方校验（引导加载程序通过 DTB 传入）
        unsafe { fdt_parser::Fdt::from_ptr_unaligned(fdt_addr as *const u8) }.map_err(|e| {
            log::warn!("FDT 头部校验失败 (addr={:#x}): {:?}", fdt_addr, e);
            FdtError::InvalidHeader
        })?;
        Ok(Self {
            fdt_addr,
            _marker: PhantomData,
        })
    }

    pub fn core_count(&self) -> Result<usize, FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
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
        let count = iter.filter_map(|c| c.ok()).count();
        if count == 0 {
            return Err(FdtError::NodeNotFound);
        }
        Ok(count)
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
        let fdt = parse_fdt!(self.fdt_addr)?;
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

    pub fn memory(&self) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
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
    pub fn firmware_reserved_memory(&self) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
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
    pub fn timebase_frequency(&self) -> Result<u32, FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
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
    /// 解析失败时返回错误而非静默返回 0。
    pub fn node_count(&self) -> Result<usize, FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历所有节点失败: {:?}", e);
            FdtError::ParseFailed
        })?;
        Ok(nodes.filter_map(|n| n.ok()).count())
    }

    /// 在 FDT 中查找第一个 `compatible` 属性包含 `compat` 字符串的节点，
    /// 返回其 `reg` 属性的第一组 (address, size)。
    ///
    /// 用于从 FDT 动态获取 PLIC/GIC 等中断控制器的基地址。
    pub fn find_compatible_reg(&self, compat: &str) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let compatibles = [compat];
        let nodes = fdt.all_compatible(&compatibles).map_err(|e| {
            log::warn!("FDT 查找 compatible={} 节点失败: {:?}", compat, e);
            FdtError::ParseFailed
        })?;

        for node_result in nodes {
            let node = node_result.map_err(|e| {
                log::warn!("FDT compatible={} 节点解析失败: {:?}", compat, e);
                FdtError::ParseFailed
            })?;
            let Ok(Some(reg)) = node.reg() else {
                continue;
            };
            if let Some(region) = reg.iter::<u64, usize>().next() {
                let region = region.map_err(|e| {
                    log::warn!("FDT compatible={} reg 解析失败: {:?}", compat, e);
                    FdtError::ParseFailed
                })?;
                let addr = region.address;
                let size = region.len;
                return Ok((addr, size));
            }
        }

        Err(FdtError::NodeNotFound)
    }

    /// 与 `find_compatible_reg` 相同，但返回 `reg` 属性的第 N 组 (address, size)。
    ///
    /// `index=0` 等价于 `find_compatible_reg`。
    /// 用于 GICv3 等节点的 `reg` 属性包含多组区域的情况。
    pub fn find_compatible_reg_nth(
        &self,
        compat: &str,
        index: usize,
    ) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let compatibles = [compat];
        let nodes = fdt.all_compatible(&compatibles).map_err(|e| {
            log::warn!("FDT 查找 compatible={} 节点失败: {:?}", compat, e);
            FdtError::ParseFailed
        })?;

        for node_result in nodes {
            let node = node_result.map_err(|e| {
                log::warn!("FDT compatible={} 节点解析失败: {:?}", compat, e);
                FdtError::ParseFailed
            })?;
            let Ok(Some(reg)) = node.reg() else {
                continue;
            };
            if let Some(region) = reg.iter::<u64, usize>().nth(index) {
                let region = region.map_err(|e| {
                    log::warn!("FDT compatible={} reg #{} 解析失败: {:?}", compat, index, e);
                    FdtError::ParseFailed
                })?;
                let addr = region.address;
                let size = region.len;
                return Ok((addr, size));
            }
        }

        Err(FdtError::NodeNotFound)
    }
    /// 查找第 N 个 `compatible` 匹配的**节点**，返回其 `reg` 第一组 (address, size)。
    ///
    /// 与 `find_compatible_reg_nth` 不同：此方法按**节点序号**索引
    /// （多个 `virtio,mmio` 节点各有一个 `reg`），
    /// 而 `find_compatible_reg_nth` 按单节点内的 `reg` 条目索引
    /// （GICv3 单节点多 `reg` 区域）。
    pub fn find_compatible_node_nth(
        &self,
        compat: &str,
        node_index: usize,
    ) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let compatibles = [compat];
        let nodes = fdt.all_compatible(&compatibles).map_err(|e| {
            log::warn!("FDT 查找 compatible={} 节点失败: {:?}", compat, e);
            FdtError::ParseFailed
        })?;

        for (count, node_result) in nodes.enumerate() {
            let node = node_result.map_err(|e| {
                log::warn!("FDT compatible={} 节点解析失败: {:?}", compat, e);
                FdtError::ParseFailed
            })?;
            if count == node_index {
                let reg = node.reg().map_err(|e| {
                    log::warn!(
                        "FDT compatible={} 节点 #{} reg 属性解析失败: {:?}",
                        compat,
                        node_index,
                        e
                    );
                    FdtError::ParseFailed
                })?;
                let reg = reg.ok_or(FdtError::PropertyNotFound)?;
                let region = reg
                    .iter::<u64, usize>()
                    .next()
                    .ok_or(FdtError::InvalidPropertySize)?
                    .map_err(|e| {
                        log::warn!(
                            "FDT compatible={} 节点 #{} reg 第一项解析失败: {:?}",
                            compat,
                            node_index,
                            e
                        );
                        FdtError::ParseFailed
                    })?;
                let addr = region.address;
                let size = region.len;
                return Ok((addr, size));
            }
        }

        Err(FdtError::NodeNotFound)
    }
}
