// Copyright The SimpleKernel Contributors

//! 设备管理器——全局设备注册与查找。

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;

use device_core::DeviceId;
use sync::SpinLock;

use super::Device;

/// 全局设备管理器。
static DEVICE_MANAGER: SpinLock<Vec<Box<dyn Device>>> =
    SpinLock::new(Vec::new(), "dev_mgr", sync::lock_level::UNSPECIFIED);

/// 初始化设备管理器。
pub(crate) fn init() {
    // Vec 已在 static 中初始化，此处仅作标记
    log::debug!("DeviceManager: initialized");
}

/// 注册一个已探测成功的设备。
///
/// # Panics
///
/// 设备数量超过 [`DeviceId`] 当前可表达范围时 panic。
pub(crate) fn register_device(device: Box<dyn Device>) -> DeviceId {
    let name = device.name().to_string();
    let dtype = device.device_type();
    let mut devices = DEVICE_MANAGER.lock();
    let raw_id = u32::try_from(devices.len()).unwrap_or_else(|_| {
        panic!(
            "DeviceManager: 设备数量超过 DeviceId 可表达范围: count={}",
            devices.len()
        )
    });
    let id = DeviceId::new(raw_id);
    devices.push(device);
    log::info!("DeviceManager: registered {:?} \"{}\"", dtype, name);
    id
}
