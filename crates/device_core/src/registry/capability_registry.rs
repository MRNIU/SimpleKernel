// Copyright The SimpleKernel Contributors

//! 设备实例与 typed capability registry。

use heapless::Vec;

use crate::DeviceCapability;

use super::{
    DeviceId, DeviceSource, DeviceType, MAX_DEVICE_CAPABILITIES, MAX_REGISTERED_DEVICES,
    RegisteredCapability, RegisteredDevice, RegistryError,
};

/// 设备实例和 capability registry。
pub struct CapabilityRegistry {
    devices: Vec<RegisteredDevice, MAX_REGISTERED_DEVICES>,
    capabilities: Vec<RegisteredCapability, MAX_DEVICE_CAPABILITIES>,
    default_block_device: Option<DeviceId>,
}

impl CapabilityRegistry {
    /// 构造空 capability registry。
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            capabilities: Vec::new(),
            default_block_device: None,
        }
    }

    /// 注册设备实例并返回分配的 [`DeviceId`]。
    ///
    /// # Errors
    ///
    /// 设备数量超过固定容量时返回 [`RegistryError::TooManyDevices`]。
    pub fn register_device(
        &mut self,
        name: &'static str,
        device_type: DeviceType,
        source: DeviceSource,
    ) -> Result<DeviceId, RegistryError> {
        let id = DeviceId::new(self.devices.len() as u32);
        let device = RegisteredDevice {
            id,
            name,
            device_type,
            source,
        };
        let count = self.devices.len();
        self.devices
            .push(device)
            .map_err(|_rejected_device| RegistryError::TooManyDevices {
                count,
                max: MAX_REGISTERED_DEVICES,
            })?;
        Ok(id)
    }

    /// 注册设备 capability。
    ///
    /// # Errors
    ///
    /// `device_id` 不存在，或 capability 数量超过固定容量时返回 [`RegistryError`]。
    pub fn register_capability(
        &mut self,
        device_id: DeviceId,
        capability: DeviceCapability,
    ) -> Result<(), RegistryError> {
        if !self.devices.iter().any(|device| device.id == device_id) {
            return Err(RegistryError::UnknownDevice { device_id });
        }

        if matches!(capability, DeviceCapability::Block(_)) && self.default_block_device.is_none() {
            self.default_block_device = Some(device_id);
        }

        let count = self.capabilities.len();
        self.capabilities
            .push(RegisteredCapability {
                device_id,
                capability,
            })
            .map_err(|_rejected_capability| RegistryError::TooManyCapabilities {
                count,
                max: MAX_DEVICE_CAPABILITIES,
            })
    }

    /// 返回所有已注册设备。
    pub fn devices(&self) -> &[RegisteredDevice] {
        self.devices.as_slice()
    }

    /// 返回所有已注册 capability。
    pub fn capabilities(&self) -> &[RegisteredCapability] {
        self.capabilities.as_slice()
    }

    /// 返回默认块设备。
    pub fn default_block_device(&self) -> Option<(DeviceId, &'static dyn crate::BlockDevice)> {
        let device_id = self.default_block_device?;
        self.capabilities.iter().find_map(|entry| {
            if entry.device_id != device_id {
                return None;
            }
            match entry.capability {
                DeviceCapability::Block(device) => Some((device_id, device)),
            }
        })
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}
