// Copyright The SimpleKernel Contributors

//! registry 共享数据类型。

use crate::{DeviceCapability, FdtProbeContext};

/// 设备实例 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId(u32);

impl DeviceId {
    /// 从 registry 分配序号构造设备 id。
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// 返回原始设备 id。
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// registry 诊断用设备分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// 块设备。
    Block,
    /// 控制台或串口设备。
    Console,
    /// 网络设备。
    Net,
    /// 其他设备。
    Other,
}

/// 设备实例来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSource {
    /// 静态 probe 产生的设备。
    Static,
    /// FDT probe 产生的设备。
    Fdt(FdtProbeContext),
}

/// 已注册设备实例。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredDevice {
    /// 设备 id。
    pub id: DeviceId,
    /// 设备诊断名。
    pub name: &'static str,
    /// 设备分类。
    pub device_type: DeviceType,
    /// 设备来源。
    pub source: DeviceSource,
}

/// 已注册 capability。
#[derive(Clone, Copy)]
pub struct RegisteredCapability {
    /// capability 归属的设备 id。
    pub device_id: DeviceId,
    /// typed capability。
    pub capability: DeviceCapability,
}
