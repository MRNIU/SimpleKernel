// Copyright The SimpleKernel Contributors

//! 平台总线——通过 FDT 遍历发现并探测设备。
//!
//! 遍历设备树中的所有节点，匹配 `compatible` 属性并调用对应的驱动探测函数。
//! 参考 Linux `drivers/of/platform.c` 中 `of_platform_bus_create()` 的设计模式。

use crate::fdt::{FdtError, KernelFdt};

/// 扫描 FDT 并探测所有已知设备。
///
/// 当前支持的 compatible 字符串：
/// - `"virtio,mmio"` → VirtIO MMIO 设备
///
/// # Panics
/// Full 初始化路径要求 FDT 已由 `early_init()` 记录且可解析；若缺失或解析失败，
/// 表示启动平台契约被破坏，必须 fail-fast，不能静默跳过设备扫描。
pub fn probe_all() {
    let fdt = crate::fdt::get().expect("PlatformBus: FDT 未初始化，Full 初始化不能跳过设备扫描");

    // 探测所有 VirtIO MMIO 设备
    probe_virtio_mmio_devices(fdt);
}

/// 枚举 FDT 中所有 `virtio,mmio` compatible 的节点并逐一探测。
///
/// QEMU virt 平台通常在 0x10001000 起始地址分配多个 VirtIO MMIO 设备，
/// 每个设备占 0x200 字节寄存器空间。
fn probe_virtio_mmio_devices(fdt: &KernelFdt) {
    // 遍历查找所有 virtio,mmio 节点
    for index in 0..32 {
        match fdt.find_compatible_node_nth("virtio,mmio", index) {
            Ok((addr, size)) => {
                let paddr = memory_types::PhysAddr::new(addr as usize);
                log::debug!(
                    "PlatformBus: found \"virtio,mmio\" #{} at {}, size={:#x}",
                    index,
                    paddr,
                    size
                );
                if let Err(e) = super::virtio::probe_mmio_device(paddr, size) {
                    log::debug!("PlatformBus: virtio,mmio #{} probe skipped: {:?}", index, e);
                }
            }
            Err(FdtError::NodeNotFound) => {
                // 没有更多 virtio,mmio 节点。
                break;
            }
            Err(e) => panic!(
                "PlatformBus: virtio,mmio #{} FDT reg 解析失败: {:?}",
                index, e
            ),
        }
    }
}
