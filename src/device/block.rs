// Copyright The SimpleKernel Contributors

//! 本地块设备能力门面。
//!
//! 本模块只表达上层文件系统需要的块设备契约，不负责驱动 probe、priority 或
//! descriptor registry。D1 阶段只注册启动过程中发现的第一块设备，后续 D2+ 再把
//! 多设备选择和 typed capability registry 纳入设备框架。

use super::DeviceError;

/// 本地块设备能力接口。
///
/// 上层模块依赖此 trait，而不是直接依赖 VirtIO、`rdif-*` 或第三方设备句柄。
pub trait BlockDevice: Send + Sync {
    /// 单个扇区大小，单位为字节。
    fn sector_size(&self) -> usize;

    /// 设备总扇区数。
    fn sector_count(&self) -> u64;

    /// 设备容量，单位为字节。
    fn capacity(&self) -> u64 {
        self.sector_count()
            .saturating_mul(self.sector_size() as u64)
    }

    /// 读取一个完整扇区。
    ///
    /// # Errors
    ///
    /// `buf.len()` 与 [`sector_size`](Self::sector_size) 不一致、`sector` 越界、
    /// 或底层设备返回 I/O 错误时返回 [`DeviceError`]。
    fn read_sector(&self, sector: u64, buf: &mut [u8]) -> Result<(), DeviceError>;

    /// 写入一个完整扇区。
    ///
    /// # Errors
    ///
    /// `buf.len()` 与 [`sector_size`](Self::sector_size) 不一致、`sector` 越界、
    /// 或底层设备返回 I/O 错误时返回 [`DeviceError`]。
    fn write_sector(&self, sector: u64, buf: &[u8]) -> Result<(), DeviceError>;
}

/// D1 阶段的默认块设备。
static BLOCK_DEVICE: spin::Once<&'static dyn BlockDevice> = spin::Once::new();

/// 注册启动阶段发现的默认块设备。
///
/// D1 只暴露第一块设备；若后续又探测到块设备，本函数会保留既有门面并记录诊断日志。
pub fn register_block_device(device: &'static dyn BlockDevice) {
    if BLOCK_DEVICE.get().is_some() {
        log::warn!("BlockDevice: 默认块设备已注册，跳过重复注册");
        return;
    }

    BLOCK_DEVICE.call_once(|| device);
    log::info!(
        "BlockDevice: registered default block device (sector_size={}, sectors={})",
        device.sector_size(),
        device.sector_count()
    );
}

/// 获取默认块设备门面。
#[must_use]
pub fn block_device() -> Option<&'static dyn BlockDevice> {
    BLOCK_DEVICE.get().map(|device| *device)
}

/// 校验整扇区 I/O 的缓冲区长度和扇区边界。
///
/// # Errors
///
/// 缓冲区长度不是完整扇区，或扇区号不在设备容量范围内时返回 [`DeviceError`]。
pub fn validate_sector_io(
    sector: u64,
    buf_len: usize,
    sector_size: usize,
    sector_count: u64,
) -> Result<(), DeviceError> {
    if buf_len != sector_size {
        return Err(DeviceError::InvalidBlockBuffer {
            len: buf_len,
            sector_size,
        });
    }

    if sector >= sector_count {
        return Err(DeviceError::BlockSectorOutOfRange {
            sector,
            sector_count,
        });
    }

    Ok(())
}
