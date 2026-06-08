// Copyright The SimpleKernel Contributors

//! VirtIO 设备探测与管理——利用 `virtio-drivers` crate。
//!
//! 通过 MMIO transport 探测 VirtIO 设备类型，对支持的设备（当前仅块设备）
//! 执行完整初始化并注册到 DeviceManager。
//!
//! [VirtIO spec §4.2 Virtio Over MMIO](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)

use alloc::boxed::Box;
use alloc::format;

use core::ptr::NonNull;

use memory_types::PhysAddr;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, Transport};

use super::block::{self, BlockDevice};
use super::hal::SimpleKernelHal;
use super::{DeviceError, manager};

/// VirtIO MMIO 设备标准寄存器空间大小（字节）。
const VIRTIO_MMIO_SIZE: usize = 0x200;

/// VirtIO block 设备的扇区大小。
const VIRTIO_BLOCK_SECTOR_SIZE: usize = 512;

/// 当前 VirtIO block 全局锁类型。
pub type VirtIOBlockLock = sync::SpinLock<VirtIOBlk<SimpleKernelHal, MmioTransport<'static>>>;

/// VirtIO 块设备包装——实现 `Device` trait 以注册到 DeviceManager。
pub struct VirtIOBlockDevice {
    name: alloc::string::String,
}

impl super::Device for VirtIOBlockDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_type(&self) -> super::DeviceType {
        super::DeviceType::Block
    }
}

/// 全局 VirtIO 块设备引用（供文件系统层使用）。
///
/// 使用 `spin::Once` 保证只初始化一次。
/// 存储 `SpinLock` 包装以支持多核并发访问。
static VIRTIO_BLK: spin::Once<VirtIOBlockLock> = spin::Once::new();

/// 获取全局 VirtIO 块设备引用。
///
/// 返回 `None` 表示尚未探测到块设备。
pub fn virtio_blk() -> Option<&'static VirtIOBlockLock> {
    VIRTIO_BLK.get()
}

impl BlockDevice for VirtIOBlockLock {
    fn sector_size(&self) -> usize {
        VIRTIO_BLOCK_SECTOR_SIZE
    }

    fn sector_count(&self) -> u64 {
        self.lock().capacity()
    }

    fn read_sector(&self, sector: u64, buf: &mut [u8]) -> Result<(), DeviceError> {
        let mut blk = self.lock();
        let sector_count = blk.capacity();
        block::validate_sector_io(sector, buf.len(), VIRTIO_BLOCK_SECTOR_SIZE, sector_count)?;
        let sector_index =
            usize::try_from(sector).map_err(|_| DeviceError::BlockSectorOutOfRange {
                sector,
                sector_count,
            })?;

        blk.read_blocks(sector_index, buf).map_err(|e| {
            log::warn!("VirtIO 块设备读取失败 (sector={}): {:?}", sector, e);
            DeviceError::IoError
        })
    }

    fn write_sector(&self, sector: u64, buf: &[u8]) -> Result<(), DeviceError> {
        let mut blk = self.lock();
        let sector_count = blk.capacity();
        block::validate_sector_io(sector, buf.len(), VIRTIO_BLOCK_SECTOR_SIZE, sector_count)?;
        let sector_index =
            usize::try_from(sector).map_err(|_| DeviceError::BlockSectorOutOfRange {
                sector,
                sector_count,
            })?;

        blk.write_blocks(sector_index, buf).map_err(|e| {
            log::warn!("VirtIO 块设备写入失败 (sector={}): {:?}", sector, e);
            DeviceError::IoError
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
    let mmio_size = size.max(VIRTIO_MMIO_SIZE);

    let region = memory::MmioRegion::map(paddr, mmio_size);

    let header =
        NonNull::new(region.base().as_mut_ptr::<VirtIOHeader>()).expect("MMIO vaddr 不应为空");

    // SAFETY: vaddr 指向已映射的 VirtIO MMIO 区域，生命周期为 'static（MMIO 映射永久存在）
    let transport = match unsafe { MmioTransport::new(header, mmio_size) } {
        Ok(t) => t,
        Err(e) => {
            log::debug!("VirtIO probe: MmioTransport::new 失败: {:?}", e);
            return Err(DeviceError::InvalidMagic);
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
        DeviceType::Block => init_block_device(transport, paddr),
        _ => {
            log::debug!("VirtIO: {:?} 设备暂不支持，跳过", device_type);
            Ok(())
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
) -> Result<(), DeviceError> {
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

    // 注册到设备管理器
    let dev_name = format!("virtio-blk@{}", paddr);
    let device = Box::new(VirtIOBlockDevice { name: dev_name });
    manager::register_device(device);

    // 存储全局引用供文件系统使用
    VIRTIO_BLK.call_once(|| sync::SpinLock::new(blk, "virtio_blk", sync::lock_level::UNSPECIFIED));
    let blk = VIRTIO_BLK
        .get()
        .expect("VirtIO block Once 刚初始化后应可取得全局引用");
    block::register_block_device(blk);

    Ok(())
}
