// Copyright The SimpleKernel Contributors

//! 设备核心模型。
//!
//! 本 crate 只承载驱动 descriptor、probe 语义、稳定设备身份和 typed capability
//! registry。它不拥有 DTB 生命周期，不扫描全局 FDT，也不依赖 VirtIO、MMIO、DMA 或 FAT。

#![no_std]

pub mod capability;
pub mod descriptor;
pub mod registry;

pub use capability::{BlockDevice, BlockError, BlockResult, CapabilityType, DeviceCapability};
pub use descriptor::{
    DriverDescriptor, FdtProbeContext, ProbeContext, ProbeFailure, ProbeFailureKind, ProbeFn,
    ProbeKind, ProbeLevel, ProbeOutcome, ProbePriority, ProbeRequirement, ProbeSkipReason,
};
pub use registry::{
    CapabilityRegistry, DeviceId, DeviceSource, DeviceType, DriverProbeStats, DriverRegistry,
    MAX_DEVICE_CAPABILITIES, MAX_DRIVER_DESCRIPTORS, MAX_REGISTERED_DEVICES, RegisteredCapability,
    RegisteredDevice, RegistryError,
};
