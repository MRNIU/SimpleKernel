// Copyright The SimpleKernel Contributors

//! VirtIO MMIO transport 的 FDT probe 与初始化。

use core::ptr::NonNull;

use device_core::{
    DeviceSource, FdtProbeContext, ProbeContext, ProbeFailure, ProbeFailureKind, ProbeOutcome,
    ProbeSkipReason,
};
use memory_types::PhysAddr;
use virtio_drivers::transport::mmio::{MmioError, MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, DeviceTypeError, Transport};

use super::super::DeviceError;
use super::block_device::init_block_device;

/// VirtIO MMIO transport 的 FDT `compatible` 字符串。
const FDT_COMPATIBLE_MMIO: &str = "virtio,mmio";

/// VirtIO MMIO descriptor 声明的 FDT compatible 集合。
pub(in crate::device) const FDT_COMPATIBLES_MMIO: &[&str] = &[FDT_COMPATIBLE_MMIO];

/// VirtIO MMIO typed probe 结果。
pub enum VirtioMmioProbeResult {
    /// 成功绑定为块设备。
    Block {
        /// capability registry 分配的设备 id。
        device_id: device_core::DeviceId,
    },
    /// 识别到资源但不由当前 VirtIO block 路径绑定。
    Skipped {
        /// 跳过原因。
        reason: ProbeSkipReason,
    },
}

/// 探测单个 VirtIO MMIO 设备。
///
/// 映射 MMIO 区域后读取设备头部，根据设备类型决定是否继续初始化。
/// 当前仅支持 Block 设备，其他类型记录日志后跳过。
///
/// # Errors
///
/// 魔数无效、设备初始化失败时返回对应错误。
/// MMIO 映射失败由 [`memory::MmioRegion::map`] panic（boot 时配置错误是内核 bug）。
pub fn probe_mmio_device(paddr: PhysAddr, size: usize) -> Result<(), DeviceError> {
    match probe_mmio_at(paddr, size, DeviceSource::Static)? {
        VirtioMmioProbeResult::Block { .. } | VirtioMmioProbeResult::Skipped { .. } => Ok(()),
    }
}

/// VirtIO MMIO descriptor adapter。
pub(in crate::device) fn probe_mmio_descriptor(
    context: ProbeContext,
) -> Result<ProbeOutcome, ProbeFailure> {
    let ProbeContext::Fdt(context) = context else {
        return Err(ProbeFailure::new(
            ProbeFailureKind::InvalidResource,
            "virtio-mmio descriptor requires FDT context",
        ));
    };

    match probe_mmio_block(context) {
        Ok(VirtioMmioProbeResult::Block { device_id }) => Ok(ProbeOutcome::Bound { device_id }),
        Ok(VirtioMmioProbeResult::Skipped { reason }) => Ok(ProbeOutcome::Skipped { reason }),
        Err(error) => {
            log_probe_error(context, error);
            Err(probe_failure_from_device_error(error))
        }
    }
}

/// 从 FDT probe 上下文探测 VirtIO MMIO block 设备。
///
/// # Errors
///
/// FDT `reg` 地址超出当前地址宽度、transport 初始化失败、block 初始化失败或读测试失败时返回
/// [`DeviceError`]。
pub fn probe_mmio_block(context: FdtProbeContext) -> Result<VirtioMmioProbeResult, DeviceError> {
    let addr = usize::try_from(context.reg.address).map_err(|error| {
        log::warn!(
            "VirtIO MMIO FDT 地址超出 usize: node_id={}, name={}@{}, compatible={}, addr={:#x}, size={:#x}, error={error}",
            context.node_id.ordinal(),
            context.node_name.name,
            context.node_name.unit_address.unwrap_or("<none>"),
            context.matched_compatible,
            context.reg.address,
            context.reg.size
        );
        DeviceError::InvalidResource
    })?;
    let paddr = PhysAddr::new(addr);
    probe_mmio_at(paddr, context.reg.size, DeviceSource::Fdt(context))
}

fn probe_mmio_at(
    paddr: PhysAddr,
    size: usize,
    source: DeviceSource,
) -> Result<VirtioMmioProbeResult, DeviceError> {
    if size == 0 {
        log::warn!("VirtIO probe: MMIO resource size 为 0: paddr={}", paddr);
        return Err(DeviceError::InvalidResource);
    }

    let region = memory::MmioRegion::map(paddr, size);
    let header = NonNull::new(region.base().as_mut_ptr::<VirtIOHeader>()).unwrap_or_else(|| {
        panic!(
            "VirtIO MMIO: 映射后的 header vaddr 为空: paddr={}, size={:#x}",
            paddr, size
        )
    });

    // SAFETY: vaddr 指向已映射的 VirtIO MMIO 区域，生命周期为 'static（MMIO 映射永久存在）。
    let transport = match unsafe { MmioTransport::new(header, size) } {
        Ok(transport) => transport,
        Err(MmioError::InvalidDeviceID(DeviceTypeError::InvalidDeviceType(0))) => {
            log::debug!("VirtIO: {} 是空 MMIO slot，跳过", paddr);
            return Ok(VirtioMmioProbeResult::Skipped {
                reason: ProbeSkipReason::NotApplicable,
            });
        }
        Err(MmioError::InvalidDeviceID(error)) => {
            log::debug!("VirtIO: {} device id 暂不支持: {:?}", paddr, error);
            return Ok(VirtioMmioProbeResult::Skipped {
                reason: ProbeSkipReason::UnsupportedDevice,
            });
        }
        Err(MmioError::BadMagic(error)) => {
            log::debug!("VirtIO probe: MMIO magic 无效: {:#x}", error);
            return Err(DeviceError::InvalidMagic);
        }
        Err(MmioError::MmioRegionTooSmall) => {
            log::warn!(
                "VirtIO probe: MMIO resource size 太小: paddr={}, size={:#x}",
                paddr,
                size
            );
            return Err(DeviceError::InvalidResource);
        }
        Err(error) => {
            log::debug!("VirtIO probe: MmioTransport::new 失败: {:?}", error);
            return Err(DeviceError::TransportInitFailed);
        }
    };

    let device_type = transport.device_type();
    log::info!(
        "VirtIO: found {:?} at {}, size={:#x}",
        device_type,
        paddr,
        size
    );

    match device_type {
        DeviceType::Block => {
            let device_id = init_block_device(transport, paddr, source)?;
            Ok(VirtioMmioProbeResult::Block { device_id })
        }
        _ => {
            log::debug!("VirtIO: {:?} 设备暂不支持，跳过", device_type);
            Ok(VirtioMmioProbeResult::Skipped {
                reason: ProbeSkipReason::UnsupportedDevice,
            })
        }
    }
}

fn probe_failure_from_device_error(error: DeviceError) -> ProbeFailure {
    let kind = match error {
        DeviceError::InvalidMagic | DeviceError::TransportInitFailed => {
            ProbeFailureKind::TransportInitFailed
        }
        DeviceError::InvalidResource | DeviceError::DeviceNotFound => {
            ProbeFailureKind::InvalidResource
        }
        DeviceError::ProbeFailed | DeviceError::DmaAllocFailed => {
            ProbeFailureKind::DeviceInitFailed
        }
        DeviceError::UnsupportedDevice => ProbeFailureKind::Unsupported,
        DeviceError::IoError => ProbeFailureKind::IoFailed,
    };
    ProbeFailure::new(kind, "virtio-mmio probe failed")
}

fn log_probe_error(context: FdtProbeContext, error: DeviceError) {
    log::warn!(
        "VirtIO MMIO descriptor probe 失败: node_id={}, name={}@{}, compatible={}, reg_addr={:#x}, reg_size={:#x}, error={:?}",
        context.node_id.ordinal(),
        context.node_name.name,
        context.node_name.unit_address.unwrap_or("<none>"),
        context.matched_compatible,
        context.reg.address,
        context.reg.size,
        error
    );
}
