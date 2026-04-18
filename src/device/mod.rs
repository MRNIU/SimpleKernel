//! 设备管理框架——设备注册、查找、VirtIO 子系统。
//!
//! 本模块通过 FDT 枚举发现设备，调用对应驱动探测函数，
//! 并将成功探测的设备注册到全局 `DeviceManager`。
//!
//! 架构：
//! - `hal.rs`：`virtio-drivers` crate 的 HAL 实现
//! - `manager.rs`：设备注册/查找
//! - `platform_bus.rs`：FDT 遍历 → 驱动匹配
//! - `virtio.rs`：VirtIO 设备探测与管理

pub mod hal;
pub mod manager;
pub mod platform_bus;
pub mod virtio;

use core::fmt;

/// 设备子系统错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    /// VirtIO MMIO header 魔数无效
    InvalidMagic,
    /// 设备探测失败
    ProbeFailed,
    /// 不支持的设备类型
    UnsupportedDevice,
    /// DMA 分配失败
    DmaAllocFailed,
    /// 设备 I/O 错误
    IoError,
    /// FDT 中未找到设备
    DeviceNotFound,
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for DeviceError {}

/// 设备类型分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 块设备（磁盘/分区）
    Block,
    /// 控制台/串口
    Console,
    /// 网络接口
    Net,
    /// 其他设备
    Other,
}

/// 设备 trait——所有设备驱动实现此接口。
pub trait Device: Send + Sync {
    /// 设备名称（如 "virtio-blk0"）。
    fn name(&self) -> &str;

    /// 设备类型。
    fn device_type(&self) -> DeviceType;
}

/// 初始化设备子系统——扫描 FDT 并探测所有设备。
///
/// 在页表激活和中断初始化之后调用。
pub fn device_init() {
    log::info!("DeviceInit: scanning FDT...");
    manager::init();
    platform_bus::probe_all();
    let count = manager::device_count();
    log::info!("DeviceInit complete: {} devices enumerated", count);
}
