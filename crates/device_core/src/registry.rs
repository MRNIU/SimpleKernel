// Copyright The SimpleKernel Contributors

//! descriptor 与 capability registry 的纯模型。

use heapless::Vec;

use crate::{DeviceCapability, DriverDescriptor, FdtProbeContext, ProbeOutcome};

/// 第一版内建驱动 descriptor 数量上限。
pub const MAX_DRIVER_DESCRIPTORS: usize = 16;
/// 第一版启动期注册设备数量上限。
pub const MAX_REGISTERED_DEVICES: usize = 32;
/// 第一版启动期注册 capability 数量上限。
pub const MAX_DEVICE_CAPABILITIES: usize = 32;

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

/// 单个 descriptor 的 probe 统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverProbeStats {
    /// descriptor 名称。
    pub driver_name: &'static str,
    /// 匹配到的资源数量。
    pub matched: u32,
    /// 成功绑定数量。
    pub bound: u32,
    /// 跳过数量。
    pub skipped: u32,
    /// 失败数量。
    pub failed: u32,
}

impl DriverProbeStats {
    /// 为指定 descriptor 构造空统计。
    pub const fn new(driver_name: &'static str) -> Self {
        Self {
            driver_name,
            matched: 0,
            bound: 0,
            skipped: 0,
            failed: 0,
        }
    }

    /// 记录一次资源匹配。
    pub fn record_match(&mut self) {
        self.matched = self.matched.saturating_add(1);
    }

    /// 记录一次 probe outcome。
    pub fn record_outcome(&mut self, outcome: ProbeOutcome) {
        match outcome {
            ProbeOutcome::Bound { .. } => {
                self.bound = self.bound.saturating_add(1);
            }
            ProbeOutcome::Skipped { .. } => {
                self.skipped = self.skipped.saturating_add(1);
            }
        }
    }

    /// 记录一次 probe failure。
    pub fn record_failure(&mut self) {
        self.failed = self.failed.saturating_add(1);
    }
}

/// 排序并校验后的内建驱动集合。
pub struct DriverRegistry<'drivers> {
    drivers: Vec<&'drivers DriverDescriptor, MAX_DRIVER_DESCRIPTORS>,
    stats: Vec<DriverProbeStats, MAX_DRIVER_DESCRIPTORS>,
}

impl<'drivers> DriverRegistry<'drivers> {
    /// 构造 driver registry。
    ///
    /// # Errors
    ///
    /// descriptor 数量超过固定容量、驱动名重复，或 FDT compatible 重复声明时返回
    /// [`RegistryError`]。
    pub fn new(descriptors: &'drivers [DriverDescriptor]) -> Result<Self, RegistryError> {
        if descriptors.len() > MAX_DRIVER_DESCRIPTORS {
            return Err(RegistryError::TooManyDrivers {
                count: descriptors.len(),
                max: MAX_DRIVER_DESCRIPTORS,
            });
        }

        validate_unique_driver_names(descriptors)?;
        validate_unique_compatibles(descriptors)?;

        let mut drivers = Vec::new();
        for descriptor in descriptors {
            drivers
                .push(descriptor)
                .map_err(|_| RegistryError::TooManyDrivers {
                    count: descriptors.len(),
                    max: MAX_DRIVER_DESCRIPTORS,
                })?;
        }
        drivers
            .as_mut_slice()
            .sort_unstable_by(|left, right| compare_descriptors(left, right));

        let mut stats = Vec::new();
        for descriptor in drivers.iter() {
            stats
                .push(DriverProbeStats::new(descriptor.name))
                .map_err(|_| RegistryError::TooManyDrivers {
                    count: descriptors.len(),
                    max: MAX_DRIVER_DESCRIPTORS,
                })?;
        }

        Ok(Self { drivers, stats })
    }

    /// 返回排序后的 descriptor 列表。
    pub fn drivers(&self) -> &[&'drivers DriverDescriptor] {
        self.drivers.as_slice()
    }

    /// 返回 probe 统计。
    pub fn stats(&self) -> &[DriverProbeStats] {
        self.stats.as_slice()
    }

    /// 返回可变 probe 统计。
    pub fn stats_mut(&mut self) -> &mut [DriverProbeStats] {
        self.stats.as_mut_slice()
    }
}

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
        self.devices
            .push(device)
            .map_err(|_| RegistryError::TooManyDevices {
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

        self.capabilities
            .push(RegisteredCapability {
                device_id,
                capability,
            })
            .map_err(|_| RegistryError::TooManyCapabilities {
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

fn validate_unique_driver_names(descriptors: &[DriverDescriptor]) -> Result<(), RegistryError> {
    for (index, left) in descriptors.iter().enumerate() {
        for right in &descriptors[index + 1..] {
            if left.name == right.name {
                return Err(RegistryError::DuplicateDriverName { name: left.name });
            }
        }
    }
    Ok(())
}

fn validate_unique_compatibles(descriptors: &[DriverDescriptor]) -> Result<(), RegistryError> {
    for (left_index, left) in descriptors.iter().enumerate() {
        for left_compatible in left.probe_kind.compatibles() {
            for right in &descriptors[left_index + 1..] {
                for right_compatible in right.probe_kind.compatibles() {
                    if left_compatible == right_compatible {
                        return Err(RegistryError::DuplicateCompatible {
                            compatible: left_compatible,
                            first_driver: left.name,
                            second_driver: right.name,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn compare_descriptors(left: &DriverDescriptor, right: &DriverDescriptor) -> core::cmp::Ordering {
    left.level
        .cmp(&right.level)
        .then(left.priority.cmp(&right.priority))
        .then(left.name.cmp(right.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BlockDevice, BlockError, FdtProbeContext, ProbeContext, ProbeFailure, ProbeFailureKind,
        ProbeKind, ProbeLevel, ProbePriority, ProbeRequirement, ProbeSkipReason,
    };

    fn ok_probe(_context: ProbeContext) -> Result<ProbeOutcome, ProbeFailure> {
        Ok(ProbeOutcome::Skipped {
            reason: ProbeSkipReason::NotApplicable,
        })
    }

    const VIRTIO_COMPATIBLES: &[&str] = &["virtio,mmio"];
    const NET_COMPATIBLES: &[&str] = &["vendor,net"];
    const DUP_COMPATIBLES: &[&str] = &["virtio,mmio"];

    const VIRTIO_DRIVER: DriverDescriptor = DriverDescriptor {
        name: "virtio-mmio",
        probe_kind: ProbeKind::Fdt {
            compatibles: VIRTIO_COMPATIBLES,
        },
        requirement: ProbeRequirement::Required,
        level: ProbeLevel::Device,
        priority: ProbePriority::DEFAULT,
        probe: ok_probe,
    };

    const EARLY_DRIVER: DriverDescriptor = DriverDescriptor {
        name: "early-bus",
        probe_kind: ProbeKind::Static,
        requirement: ProbeRequirement::Required,
        level: ProbeLevel::Bus,
        priority: ProbePriority(-10),
        probe: ok_probe,
    };

    const NET_DRIVER: DriverDescriptor = DriverDescriptor {
        name: "net",
        probe_kind: ProbeKind::Fdt {
            compatibles: NET_COMPATIBLES,
        },
        requirement: ProbeRequirement::Optional,
        level: ProbeLevel::Device,
        priority: ProbePriority(10),
        probe: ok_probe,
    };

    const DUP_DRIVER: DriverDescriptor = DriverDescriptor {
        name: "other-virtio",
        probe_kind: ProbeKind::Fdt {
            compatibles: DUP_COMPATIBLES,
        },
        requirement: ProbeRequirement::Optional,
        level: ProbeLevel::Device,
        priority: ProbePriority(20),
        probe: ok_probe,
    };

    #[test]
    fn driver_registry_sorts_by_level_priority_and_name() {
        let registry = DriverRegistry::new(&[NET_DRIVER, VIRTIO_DRIVER, EARLY_DRIVER])
            .expect("descriptor 集合应有效");
        let names: heapless::Vec<&str, 3> = registry
            .drivers()
            .iter()
            .map(|descriptor| descriptor.name)
            .collect();

        assert_eq!(names.as_slice(), ["early-bus", "virtio-mmio", "net"]);
    }

    #[test]
    fn driver_registry_rejects_duplicate_names() {
        let duplicate = DriverDescriptor {
            name: "virtio-mmio",
            ..NET_DRIVER
        };

        assert_eq!(
            DriverRegistry::new(&[VIRTIO_DRIVER, duplicate]).map(|_| ()),
            Err(RegistryError::DuplicateDriverName {
                name: "virtio-mmio"
            })
        );
    }

    #[test]
    fn driver_registry_rejects_duplicate_compatibles() {
        assert_eq!(
            DriverRegistry::new(&[VIRTIO_DRIVER, DUP_DRIVER]).map(|_| ()),
            Err(RegistryError::DuplicateCompatible {
                compatible: "virtio,mmio",
                first_driver: "virtio-mmio",
                second_driver: "other-virtio",
            })
        );
    }

    #[test]
    fn driver_probe_stats_separate_bound_skipped_and_failed() {
        let mut stats = DriverProbeStats::new("virtio-mmio");
        stats.record_match();
        stats.record_outcome(ProbeOutcome::Bound {
            device_id: DeviceId::new(0),
        });
        stats.record_match();
        stats.record_outcome(ProbeOutcome::Skipped {
            reason: ProbeSkipReason::UnsupportedDevice,
        });
        stats.record_failure();

        assert_eq!(stats.matched, 2);
        assert_eq!(stats.bound, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.failed, 1);
    }

    struct DummyBlockDevice;

    impl BlockDevice for DummyBlockDevice {
        fn sector_size(&self) -> usize {
            512
        }

        fn sector_count(&self) -> u64 {
            8
        }

        fn read_sector(&self, _sector: u64, buf: &mut [u8]) -> crate::BlockResult<()> {
            if buf.len() != self.sector_size() {
                return Err(BlockError::InvalidBuffer {
                    len: buf.len(),
                    sector_size: self.sector_size(),
                });
            }
            Ok(())
        }

        fn write_sector(&self, _sector: u64, buf: &[u8]) -> crate::BlockResult<()> {
            if buf.len() != self.sector_size() {
                return Err(BlockError::InvalidBuffer {
                    len: buf.len(),
                    sector_size: self.sector_size(),
                });
            }
            Ok(())
        }
    }

    static BLOCK_A: DummyBlockDevice = DummyBlockDevice;
    static BLOCK_B: DummyBlockDevice = DummyBlockDevice;

    #[test]
    fn capability_registry_keeps_first_block_as_default() {
        let mut registry = CapabilityRegistry::new();
        let first = registry
            .register_device("virtio-blk0", DeviceType::Block, DeviceSource::Static)
            .expect("第一个设备应注册成功");
        let second = registry
            .register_device("virtio-blk1", DeviceType::Block, DeviceSource::Static)
            .expect("第二个设备应注册成功");

        registry
            .register_capability(first, DeviceCapability::Block(&BLOCK_A))
            .expect("第一个块设备能力应注册成功");
        registry
            .register_capability(second, DeviceCapability::Block(&BLOCK_B))
            .expect("第二个块设备能力应注册成功");

        let (default_id, default_block) =
            registry.default_block_device().expect("默认块设备应存在");
        assert_eq!(default_id, first);
        assert_eq!(default_block.sector_count(), 8);
    }

    #[test]
    fn capability_registry_rejects_unknown_device() {
        let mut registry = CapabilityRegistry::new();

        assert_eq!(
            registry.register_capability(DeviceId::new(42), DeviceCapability::Block(&BLOCK_A)),
            Err(RegistryError::UnknownDevice {
                device_id: DeviceId::new(42)
            })
        );
    }

    #[test]
    fn fdt_device_source_reuses_probe_context_value() {
        let context = FdtProbeContext {
            node_id: platform_fdt::FdtNodeId::from_stable_ordinal(7),
            node_name: platform_fdt::FdtNodeName {
                name: "virtio_mmio",
                unit_address: Some("10001000"),
            },
            matched_compatible: "virtio,mmio",
            reg: platform_fdt::FdtReg {
                address: 0x1000_1000,
                size: 0x200,
            },
        };
        let mut registry = CapabilityRegistry::new();

        let id = registry
            .register_device("virtio-blk0", DeviceType::Block, DeviceSource::Fdt(context))
            .expect("FDT 来源设备应注册成功");

        assert_eq!(id, DeviceId::new(0));
        assert_eq!(registry.devices()[0].source, DeviceSource::Fdt(context));
    }

    #[test]
    fn probe_failure_keeps_core_owned_error_shape() {
        let failure = ProbeFailure::new(ProbeFailureKind::TransportInitFailed, "bad magic");

        assert_eq!(failure.kind, ProbeFailureKind::TransportInitFailed);
        assert_eq!(failure.detail, "bad magic");
    }
}
