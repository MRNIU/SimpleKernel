// Copyright The SimpleKernel Contributors

//! VirtIO 设备探测与管理——利用 `virtio-drivers` crate。
//!
//! 通过 MMIO transport 探测 VirtIO 设备类型，对支持的设备（当前仅块设备）
//! 执行完整初始化并注册到 DeviceManager。
//!
//! [VirtIO spec §4.2 Virtio Over MMIO](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use core::ptr::NonNull;

use device_core::{
    BlockError, DeviceSource, FdtProbeContext, ProbeContext, ProbeFailure, ProbeFailureKind,
    ProbeOutcome, ProbeSkipReason,
};
use memory_types::PhysAddr;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioError, MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, DeviceTypeError, Transport};

use super::block::{self, BlockDevice};
use super::hal::SimpleKernelHal;
use super::{DeviceError, manager};

/// VirtIO MMIO 设备标准寄存器空间大小（字节）。
const VIRTIO_MMIO_SIZE: usize = 0x200;

/// VirtIO MMIO transport 的 FDT `compatible` 字符串。
pub(super) const FDT_COMPATIBLE_MMIO: &str = "virtio,mmio";

/// VirtIO MMIO descriptor 声明的 FDT compatible 集合。
pub(super) const FDT_COMPATIBLES_MMIO: &[&str] = &[FDT_COMPATIBLE_MMIO];

/// VirtIO block 设备的扇区大小。
const VIRTIO_BLOCK_SECTOR_SIZE: usize = 512;

/// 当前 VirtIO block 全局锁类型。
pub type VirtIOBlockLock = sync::SpinLock<VirtIOBlk<SimpleKernelHal, MmioTransport<'static>>>;

/// VirtIO 块设备兼容记录——实现旧 `Device` trait 以注册到 DeviceManager。
pub struct VirtIOBlockRecord {
    name: alloc::string::String,
}

impl super::Device for VirtIOBlockRecord {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> super::DeviceType {
        super::DeviceType::Block
    }
}

/// VirtIO block typed capability 实例。
pub struct VirtIOBlockDevice {
    lock: VirtIOBlockLock,
}

impl VirtIOBlockDevice {
    fn new(block: VirtIOBlk<SimpleKernelHal, MmioTransport<'static>>) -> Self {
        Self {
            lock: sync::SpinLock::new(block, "virtio_blk", sync::lock_level::UNSPECIFIED),
        }
    }

    fn lock(&self) -> &VirtIOBlockLock {
        &self.lock
    }
}

/// 默认 VirtIO 块设备兼容引用。
///
/// D2c 后真实所有权由 capability registry 记录；本入口只保留迁移期兼容语义，返回
/// 第一个成功注册的 VirtIO block。
static VIRTIO_BLK: spin::Once<&'static VirtIOBlockLock> = spin::Once::new();

/// 获取全局 VirtIO 块设备引用。
///
/// 返回 `None` 表示尚未探测到块设备。
pub fn virtio_blk() -> Option<&'static VirtIOBlockLock> {
    VIRTIO_BLK.get().copied()
}

/// VirtIO MMIO typed probe 结果。
pub enum VirtioMmioProbeResult {
    /// 成功绑定为块设备。
    Block {
        /// 迁移期沿用旧 `DeviceManager` 分配的设备 id。
        device_id: device_core::DeviceId,
    },
    /// 识别到资源但不由当前 VirtIO block 路径绑定。
    Skipped {
        /// 跳过原因。
        reason: ProbeSkipReason,
    },
}

impl BlockDevice for VirtIOBlockDevice {
    fn sector_size(&self) -> usize {
        VIRTIO_BLOCK_SECTOR_SIZE
    }

    fn sector_count(&self) -> u64 {
        self.lock.lock().capacity()
    }

    fn read_sector(&self, sector: u64, buf: &mut [u8]) -> device_core::BlockResult<()> {
        let mut blk = self.lock.lock();
        let sector_count = blk.capacity();
        block::validate_sector_io(sector, buf.len(), VIRTIO_BLOCK_SECTOR_SIZE, sector_count)?;
        let sector_index = usize::try_from(sector).map_err(|_| BlockError::SectorOutOfRange {
            sector,
            sector_count,
        })?;

        blk.read_blocks(sector_index, buf).map_err(|e| {
            log::warn!("VirtIO 块设备读取失败 (sector={}): {:?}", sector, e);
            BlockError::Io
        })
    }

    fn write_sector(&self, sector: u64, buf: &[u8]) -> device_core::BlockResult<()> {
        let mut blk = self.lock.lock();
        let sector_count = blk.capacity();
        block::validate_sector_io(sector, buf.len(), VIRTIO_BLOCK_SECTOR_SIZE, sector_count)?;
        let sector_index = usize::try_from(sector).map_err(|_| BlockError::SectorOutOfRange {
            sector,
            sector_count,
        })?;

        blk.write_blocks(sector_index, buf).map_err(|e| {
            log::warn!("VirtIO 块设备写入失败 (sector={}): {:?}", sector, e);
            BlockError::Io
        })
    }
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
pub(super) fn probe_mmio_descriptor(context: ProbeContext) -> Result<ProbeOutcome, ProbeFailure> {
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
    let addr = usize::try_from(context.reg.address).map_err(|_| {
        log::warn!(
            "VirtIO MMIO FDT 地址超出 usize: node_id={}, name={}@{}, compatible={}, addr={:#x}, size={:#x}",
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
    let mmio_size = size.max(VIRTIO_MMIO_SIZE);

    let region = memory::MmioRegion::map(paddr, mmio_size);

    let header =
        NonNull::new(region.base().as_mut_ptr::<VirtIOHeader>()).expect("MMIO vaddr 不应为空");

    // SAFETY: vaddr 指向已映射的 VirtIO MMIO 区域，生命周期为 'static（MMIO 映射永久存在）
    let transport = match unsafe { MmioTransport::new(header, mmio_size) } {
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
        mmio_size
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

/// 初始化 VirtIO 块设备。
///
/// 创建 `VirtIOBlk` 实例，执行读测试验证设备功能，
/// 注册到 DeviceManager 并存储全局引用。
fn init_block_device(
    transport: MmioTransport<'static>,
    paddr: PhysAddr,
    source: DeviceSource,
) -> Result<device_core::DeviceId, DeviceError> {
    let mut blk = VirtIOBlk::<SimpleKernelHal, _>::new(transport).map_err(|e| {
        log::warn!("VirtIO 块设备初始化失败: {:?}", e);
        DeviceError::ProbeFailed
    })?;

    let capacity = blk.capacity();
    let capacity_mb = capacity * 512 / (1024 * 1024);
    log::info!(
        "VirtIO: block device, capacity={}MB ({} sectors)",
        capacity_mb,
        capacity
    );

    // 读测试：读取第 0 扇区验证设备可用
    {
        let mut buf = [0u8; 512];
        blk.read_blocks(0, &mut buf).map_err(|e| {
            log::warn!("VirtIO 块设备读测试失败 (sector 0): {:?}", e);
            DeviceError::IoError
        })?;
        log::info!("VirtIO: block read test OK (sector 0)");
    }

    let dev_name: &'static str = Box::leak(format!("virtio-blk@{}", paddr).into_boxed_str());

    // 注册到旧设备管理器，迁移期继续支撑 device_count() 和旧查询路径。
    let device = Box::new(VirtIOBlockRecord {
        name: String::from(dev_name),
    });
    let device_id = manager::register_device(device);

    let block_device = Box::leak(Box::new(VirtIOBlockDevice::new(blk)));
    let registry_device_id =
        block::register_block_device(dev_name, source, block_device).map_err(|error| {
            log::warn!(
                "VirtIO 块设备注册到 capability registry 失败: name={}, old_device_id={}, error={:?}",
                dev_name,
                device_id.raw(),
                error
            );
            DeviceError::InvalidResource
        })?;

    VIRTIO_BLK.call_once(|| block_device.lock());

    Ok(registry_device_id)
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
