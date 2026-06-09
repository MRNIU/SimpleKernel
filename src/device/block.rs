// Copyright The SimpleKernel Contributors

//! 本地块设备能力门面。
//!
//! 本模块是 `device_core::CapabilityRegistry` 的上层兼容门面。上层文件系统继续通过
//! `block_device()` 取得默认块设备，但默认选择已经由 registry 中首个成功注册的
//! `DeviceCapability::Block` 决定。

pub use device_core::{BlockDevice, BlockError, BlockResult, validate_sector_io};

use device_core::{
    CapabilityRegistry, DeviceCapability, DeviceId, DeviceSource, DeviceType, RegistryError,
};

/// 启动期 typed capability registry。
static CAPABILITY_REGISTRY: sync::SpinLock<CapabilityRegistry> = sync::SpinLock::new(
    CapabilityRegistry::new(),
    "dev_caps",
    sync::lock_level::UNSPECIFIED,
);

/// 注册启动阶段发现的默认块设备。
///
/// # Errors
///
/// 设备实例或 capability 数量超过第一版固定容量时返回 [`RegistryError`]。
pub fn register_block_device(
    name: &'static str,
    source: DeviceSource,
    device: &'static dyn BlockDevice,
) -> Result<DeviceId, RegistryError> {
    let mut registry = CAPABILITY_REGISTRY.lock();
    let had_default = registry.default_block_device().is_some();
    let device_id = registry.register_device(name, DeviceType::Block, source)?;
    registry.register_capability(device_id, DeviceCapability::Block(device))?;

    if had_default {
        log::info!(
            "BlockDevice: registered additional block device id={} name={} (sector_size={}, sectors={})",
            device_id.raw(),
            name,
            device.sector_size(),
            device.sector_count()
        );
    } else {
        log::info!(
            "BlockDevice: registered default block device id={} name={} (sector_size={}, sectors={})",
            device_id.raw(),
            name,
            device.sector_size(),
            device.sector_count()
        );
    }

    Ok(device_id)
}

/// 获取默认块设备门面。
#[must_use]
pub fn block_device() -> Option<&'static dyn BlockDevice> {
    CAPABILITY_REGISTRY
        .lock()
        .default_block_device()
        .map(|(_, device)| device)
}

/// 返回 registry 当前记录的默认块设备 id。
#[must_use]
pub fn default_block_device_id() -> Option<DeviceId> {
    CAPABILITY_REGISTRY
        .lock()
        .default_block_device()
        .map(|(device_id, _)| device_id)
}
