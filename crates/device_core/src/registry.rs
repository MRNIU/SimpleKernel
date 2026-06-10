// Copyright The SimpleKernel Contributors

//! descriptor 与 capability registry 的纯模型。
//!
//! 本模块按职责拆分 driver registry、capability registry、共享类型和错误；
//! 对外仍通过 `device_core::registry::*` 暴露原有 API。

mod capability_registry;
mod driver_registry;
mod error;
#[cfg(test)]
mod tests;
mod types;

pub use capability_registry::CapabilityRegistry;
pub use driver_registry::{DriverProbeStats, DriverRegistry};
pub use error::RegistryError;
pub use types::{DeviceId, DeviceSource, DeviceType, RegisteredCapability, RegisteredDevice};
