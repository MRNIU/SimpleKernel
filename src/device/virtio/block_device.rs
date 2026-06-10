// Copyright The SimpleKernel Contributors

//! VirtIO block 设备的本地 BlockDevice 门面。

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use device_core::{BlockError, DeviceSource};
use memory_types::PhysAddr;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::MmioTransport;

use super::super::block::{self, BlockDevice};
use super::super::hal::SimpleKernelHal;
use super::super::{DeviceError, manager};

/// VirtIO block 设备的扇区大小。
const VIRTIO_BLOCK_SECTOR_SIZE: usize = 512;

/// 当前 VirtIO block 全局锁类型。
pub type VirtIOBlockLock = sync::SpinLock<VirtIOBlk<SimpleKernelHal, MmioTransport<'static>>>;

/// VirtIO 块设备兼容记录——实现旧 `Device` trait 以注册到 DeviceManager。
pub struct VirtIOBlockRecord {
    name: String,
}

impl super::super::Device for VirtIOBlockRecord {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> super::super::DeviceType {
        super::super::DeviceType::Block
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

/// 初始化 VirtIO 块设备。
///
/// 创建 `VirtIOBlk` 实例，执行读测试验证设备功能，
/// 注册 VirtIO block 设备记录和块设备 capability。
pub(super) fn init_block_device(
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

    // 读取第 0 扇区验证设备可用。
    {
        let mut buf = [0u8; 512];
        blk.read_blocks(0, &mut buf).map_err(|e| {
            log::warn!("VirtIO 块设备读测试失败 (sector 0): {:?}", e);
            DeviceError::IoError
        })?;
        log::info!("VirtIO: block read test OK (sector 0)");
    }

    let dev_name: &'static str = Box::leak(format!("virtio-blk@{}", paddr).into_boxed_str());

    // 注册到旧设备管理器，迁移期继续支撑内部枚举日志。
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

    Ok(registry_device_id)
}
