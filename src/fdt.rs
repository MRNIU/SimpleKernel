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
}

impl fmt::Display for FdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for FdtError {}

/// FDT 基地址（`early_init` 中初始化，中断子系统解析 PLIC/GIC 时使用）
pub static FDT_ADDR: spin::Once<usize> = spin::Once::new();

#[derive(Debug)]
pub struct KernelFdt<'a> {
    fdt_addr: usize,
    _marker: PhantomData<&'a [u8]>,
}

macro_rules! parse_fdt {
    ($addr:expr) => {{
        // SAFETY: fdt_addr 已在 KernelFdt::new() 中校验
        unsafe { fdt::Fdt::from_ptr_unaligned_fallible($addr as *const u8) }.map_err(|e| {
            log::warn!("FDT 解析失败 (addr={:#x}): {:?}", $addr, e);
            FdtError::InvalidHeader
        })
    }};
}

impl<'a> KernelFdt<'a> {
    pub fn new(fdt_addr: usize) -> Result<Self, FdtError> {
        // SAFETY: fdt_addr 由调用方校验（引导加载程序通过 DTB 传入）
        unsafe { fdt::Fdt::from_ptr_unaligned(fdt_addr as *const u8) }.map_err(|e| {
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
        Ok((region.address, region.len))
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
    #[expect(dead_code, reason = "公开 API，供设备驱动匹配单节点多 reg 区域")]
    pub fn find_compatible_reg(&self, compat: &str) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历所有节点失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        for node_result in nodes {
            let Ok((_depth, node)) = node_result else {
                continue;
            };
            // 检查 compatible 属性
            let Ok(Some(prop)) = node.raw_property("compatible") else {
                continue;
            };
            // compatible 是 null-terminated 字符串列表
            if !compatible_contains(prop.value, compat) {
                continue;
            }
            // 读取 reg 属性——简化处理：假设 #address-cells=2, #size-cells=2
            let Ok(Some(reg)) = node.raw_property("reg") else {
                continue;
            };
            if reg.value.len() >= 16 {
                let addr = u64::from_be_bytes(reg.value[0..8].try_into().map_err(|e| {
                    log::warn!(
                        "FDT reg addr 切片转换失败 (len={}): {:?}",
                        reg.value.len(),
                        e
                    );
                    FdtError::ParseFailed
                })?);
                let size = u64::from_be_bytes(reg.value[8..16].try_into().map_err(|e| {
                    log::warn!(
                        "FDT reg size 切片转换失败 (len={}): {:?}",
                        reg.value.len(),
                        e
                    );
                    FdtError::ParseFailed
                })?) as usize;
                return Ok((addr, size));
            }
        }

        Err(FdtError::NodeNotFound)
    }

    /// 与 `find_compatible_reg` 相同，但返回 `reg` 属性的第 N 组 (address, size)。
    ///
    /// `index=0` 等价于 `find_compatible_reg`。
    /// 用于 GICv3 等节点的 `reg` 属性包含多组区域的情况。
    #[expect(dead_code, reason = "公开 API，供 GICv3 等单节点多 reg 区域解析")]
    pub fn find_compatible_reg_nth(
        &self,
        compat: &str,
        index: usize,
    ) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历所有节点失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        let entry_size = 16; // 每组 (addr[8] + size[8])
        let offset = index * entry_size;
        let required_len = offset + entry_size;

        for node_result in nodes {
            let Ok((_depth, node)) = node_result else {
                continue;
            };
            let Ok(Some(prop)) = node.raw_property("compatible") else {
                continue;
            };
            if !compatible_contains(prop.value, compat) {
                continue;
            }
            let Ok(Some(reg)) = node.raw_property("reg") else {
                continue;
            };
            if reg.value.len() >= required_len {
                let addr =
                    u64::from_be_bytes(reg.value[offset..offset + 8].try_into().map_err(|e| {
                        log::warn!(
                            "FDT reg addr 切片转换失败 (offset={}, len={}): {:?}",
                            offset,
                            reg.value.len(),
                            e
                        );
                        FdtError::ParseFailed
                    })?);
                let size = u64::from_be_bytes(
                    reg.value[offset + 8..offset + 16].try_into().map_err(|e| {
                        log::warn!(
                            "FDT reg size 切片转换失败 (offset={}, len={}): {:?}",
                            offset + 8,
                            reg.value.len(),
                            e
                        );
                        FdtError::ParseFailed
                    })?,
                ) as usize;
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
    #[expect(
        dead_code,
        reason = "公开 API，供 PlatformBus 枚举同 compatible 的多个节点"
    )]
    pub fn find_compatible_node_nth(
        &self,
        compat: &str,
        node_index: usize,
    ) -> Result<(u64, usize), FdtError> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|e| {
            log::warn!("FDT 遍历所有节点失败: {:?}", e);
            FdtError::ParseFailed
        })?;

        let mut count = 0usize;

        for node_result in nodes {
            let Ok((_depth, node)) = node_result else {
                continue;
            };
            let Ok(Some(prop)) = node.raw_property("compatible") else {
                continue;
            };
            if !compatible_contains(prop.value, compat) {
                continue;
            }
            if count == node_index {
                let Ok(Some(reg)) = node.raw_property("reg") else {
                    return Err(FdtError::PropertyNotFound);
                };
                if reg.value.len() >= 16 {
                    let addr = u64::from_be_bytes(reg.value[0..8].try_into().map_err(|e| {
                        log::warn!(
                            "FDT reg addr 切片转换失败 (len={}): {:?}",
                            reg.value.len(),
                            e
                        );
                        FdtError::ParseFailed
                    })?);
                    let size = u64::from_be_bytes(reg.value[8..16].try_into().map_err(|e| {
                        log::warn!(
                            "FDT reg size 切片转换失败 (len={}): {:?}",
                            reg.value.len(),
                            e
                        );
                        FdtError::ParseFailed
                    })?) as usize;
                    return Ok((addr, size));
                }
                return Err(FdtError::InvalidPropertySize);
            }
            count += 1;
        }

        Err(FdtError::NodeNotFound)
    }
}

/// 检查 FDT `compatible` 属性值是否包含指定字符串。
///
/// `compatible` 是以 null 分隔的字符串列表（例如 `"sifive,plic-1.0.0\0riscv,plic0\0"`）。
fn compatible_contains(value: &[u8], needle: &str) -> bool {
    let needle_bytes = needle.as_bytes();
    for entry in value.split(|&b| b == 0) {
        if entry == needle_bytes {
            return true;
        }
    }
    false
}
