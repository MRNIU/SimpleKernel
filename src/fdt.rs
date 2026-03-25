use crate::error::{ErrorCode, KResult};
use core::marker::PhantomData;

#[derive(Debug)]
pub struct KernelFdt<'a> {
    fdt_addr: usize,
    _marker: PhantomData<&'a [u8]>,
}

macro_rules! parse_fdt {
    ($addr:expr) => {{
        // SAFETY: fdt_addr 已在 KernelFdt::new() 中校验
        unsafe { fdt::Fdt::from_ptr_unaligned_fallible($addr as *const u8) }
            .map_err(|_| ErrorCode::FdtInvalidHeader)
    }};
}

impl<'a> KernelFdt<'a> {
    pub fn new(fdt_addr: usize) -> KResult<Self> {
        // SAFETY: fdt_addr 由调用方校验（引导加载程序通过 DTB 传入）
        unsafe { fdt::Fdt::from_ptr_unaligned(fdt_addr as *const u8) }
            .map_err(|_| ErrorCode::FdtInvalidHeader)?;
        Ok(Self {
            fdt_addr,
            _marker: PhantomData,
        })
    }

    pub fn core_count(&self) -> KResult<usize> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let root = fdt.root().map_err(|_| ErrorCode::FdtParseFailed)?;
        let cpus = root.cpus().map_err(|_| ErrorCode::FdtNodeNotFound)?;
        let iter = cpus.iter().map_err(|_| ErrorCode::FdtParseFailed)?;
        let count = iter.filter_map(|c| c.ok()).count();
        if count == 0 {
            return Err(ErrorCode::FdtNodeNotFound);
        }
        Ok(count)
    }

    pub fn memory(&self) -> KResult<(u64, usize)> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let root = fdt.root().map_err(|_| ErrorCode::FdtParseFailed)?;
        let memory = root.memory().map_err(|_| ErrorCode::FdtNodeNotFound)?;
        let mut regions = memory
            .reg()
            .map_err(|_| ErrorCode::FdtPropertyNotFound)?
            .iter::<u64, usize>();
        let region = regions
            .next()
            .ok_or(ErrorCode::FdtNodeNotFound)?
            .map_err(|_| ErrorCode::FdtParseFailed)?;
        Ok((region.address, region.len))
    }

    /// TODO(P4): 定时器初始化时读取此值
    #[allow(dead_code)]
    pub fn timebase_frequency(&self) -> KResult<u32> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let cpus = fdt
            .find_node("/cpus")
            .map_err(|_| ErrorCode::FdtParseFailed)?
            .ok_or(ErrorCode::FdtNodeNotFound)?;
        let prop = cpus
            .raw_property("timebase-frequency")
            .map_err(|_| ErrorCode::FdtParseFailed)?
            .ok_or(ErrorCode::FdtPropertyNotFound)?;
        let bytes: [u8; 4] = prop
            .value
            .try_into()
            .map_err(|_| ErrorCode::FdtInvalidPropertySize)?;
        Ok(u32::from_be_bytes(bytes))
    }

    /// 返回 FDT 中的节点总数。
    ///
    /// 解析失败时返回错误而非静默返回 0。
    pub fn node_count(&self) -> KResult<usize> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|_| ErrorCode::FdtParseFailed)?;
        Ok(nodes.filter_map(|n| n.ok()).count())
    }

    /// 在 FDT 中查找第一个 `compatible` 属性包含 `compat` 字符串的节点，
    /// 返回其 `reg` 属性的第一组 (address, size)。
    ///
    /// 用于从 FDT 动态获取 PLIC/GIC 等中断控制器的基地址。
    #[allow(dead_code)]
    pub fn find_compatible_reg(&self, compat: &str) -> KResult<(u64, usize)> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|_| ErrorCode::FdtParseFailed)?;

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
                let addr = u64::from_be_bytes(
                    reg.value[0..8]
                        .try_into()
                        .map_err(|_| ErrorCode::FdtParseFailed)?,
                );
                let size = u64::from_be_bytes(
                    reg.value[8..16]
                        .try_into()
                        .map_err(|_| ErrorCode::FdtParseFailed)?,
                ) as usize;
                return Ok((addr, size));
            }
        }

        Err(ErrorCode::FdtNodeNotFound)
    }

    /// 与 `find_compatible_reg` 相同，但返回 `reg` 属性的第 N 组 (address, size)。
    ///
    /// `index=0` 等价于 `find_compatible_reg`。
    /// 用于 GICv3 等节点的 `reg` 属性包含多组区域的情况。
    #[allow(dead_code)]
    pub fn find_compatible_reg_nth(&self, compat: &str, index: usize) -> KResult<(u64, usize)> {
        let fdt = parse_fdt!(self.fdt_addr)?;
        let nodes = fdt.all_nodes().map_err(|_| ErrorCode::FdtParseFailed)?;

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
                let addr = u64::from_be_bytes(
                    reg.value[offset..offset + 8]
                        .try_into()
                        .map_err(|_| ErrorCode::FdtParseFailed)?,
                );
                let size = u64::from_be_bytes(
                    reg.value[offset + 8..offset + 16]
                        .try_into()
                        .map_err(|_| ErrorCode::FdtParseFailed)?,
                ) as usize;
                return Ok((addr, size));
            }
        }

        Err(ErrorCode::FdtNodeNotFound)
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
