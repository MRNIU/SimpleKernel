// Copyright The SimpleKernel Contributors

use core::fmt;

/// 设备子系统错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceError {
    /// VirtIO MMIO header 魔数无效
    InvalidMagic,
    /// VirtIO MMIO transport 初始化失败
    TransportInitFailed,
    /// 设备探测失败
    ProbeFailed,
    /// 不支持的设备类型
    UnsupportedDevice,
    /// FDT 或平台资源描述非法
    InvalidResource,
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
