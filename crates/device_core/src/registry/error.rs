// Copyright The SimpleKernel Contributors

//! registry 错误类型。

use super::DeviceId;

/// registry 构造或注册错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    /// descriptor 数量超过第一版固定容量。
    TooManyDrivers {
        /// 实际 descriptor 数量。
        count: usize,
        /// 固定容量上限。
        max: usize,
    },
    /// descriptor name 重复。
    DuplicateDriverName {
        /// 重复的驱动名。
        name: &'static str,
    },
    /// descriptor 侧重复声明同一个 compatible。
    DuplicateCompatible {
        /// 重复的 compatible 字符串。
        compatible: &'static str,
        /// 第一个声明者。
        first_driver: &'static str,
        /// 第二个声明者。
        second_driver: &'static str,
    },
    /// 设备实例数量超过第一版固定容量。
    TooManyDevices {
        /// 固定容量上限。
        max: usize,
    },
    /// capability 数量超过第一版固定容量。
    TooManyCapabilities {
        /// 固定容量上限。
        max: usize,
    },
    /// capability 指向不存在的设备 id。
    UnknownDevice {
        /// 不存在的设备 id。
        device_id: DeviceId,
    },
}
