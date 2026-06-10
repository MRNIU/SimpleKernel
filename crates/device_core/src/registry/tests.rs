// Copyright The SimpleKernel Contributors

//! registry 单元测试。

use super::*;
use crate::{
    BlockDevice, BlockError, FdtProbeContext, ProbeContext, ProbeFailure, ProbeFailureKind,
    ProbeKind, ProbeLevel, ProbeOutcome, ProbePriority, ProbeRequirement, ProbeSkipReason,
};

fn ok_probe(_context: ProbeContext) -> Result<ProbeOutcome, ProbeFailure> {
    Ok(ProbeOutcome::Skipped {
        reason: ProbeSkipReason::NotApplicable,
    })
}

const VIRTIO_COMPATIBLES: &[&str] = &["virtio,mmio"];
const NET_COMPATIBLES: &[&str] = &["vendor,net"];
const DUP_COMPATIBLES: &[&str] = &["virtio,mmio"];

const VIRTIO_DRIVER: crate::DriverDescriptor = crate::DriverDescriptor {
    name: "virtio-mmio",
    probe_kind: ProbeKind::Fdt {
        compatibles: VIRTIO_COMPATIBLES,
    },
    requirement: ProbeRequirement::Required,
    level: ProbeLevel::Device,
    priority: ProbePriority::DEFAULT,
    probe: ok_probe,
};

const EARLY_DRIVER: crate::DriverDescriptor = crate::DriverDescriptor {
    name: "early-bus",
    probe_kind: ProbeKind::Static,
    requirement: ProbeRequirement::Required,
    level: ProbeLevel::Bus,
    priority: ProbePriority(-10),
    probe: ok_probe,
};

const NET_DRIVER: crate::DriverDescriptor = crate::DriverDescriptor {
    name: "net",
    probe_kind: ProbeKind::Fdt {
        compatibles: NET_COMPATIBLES,
    },
    requirement: ProbeRequirement::Optional,
    level: ProbeLevel::Device,
    priority: ProbePriority(10),
    probe: ok_probe,
};

const DUP_DRIVER: crate::DriverDescriptor = crate::DriverDescriptor {
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
    let duplicate = crate::DriverDescriptor {
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
        .register_capability(first, crate::DeviceCapability::Block(&BLOCK_A))
        .expect("第一个块设备能力应注册成功");
    registry
        .register_capability(second, crate::DeviceCapability::Block(&BLOCK_B))
        .expect("第二个块设备能力应注册成功");

    let (default_id, default_block) = registry.default_block_device().expect("默认块设备应存在");
    assert_eq!(default_id, first);
    assert_eq!(default_block.sector_count(), 8);
}

#[test]
fn capability_registry_rejects_unknown_device() {
    let mut registry = CapabilityRegistry::new();

    assert_eq!(
        registry.register_capability(DeviceId::new(42), crate::DeviceCapability::Block(&BLOCK_A)),
        Err(RegistryError::UnknownDevice {
            device_id: DeviceId::new(42)
        })
    );
}

#[test]
fn block_validation_rejects_partial_or_out_of_range_io() {
    assert_eq!(
        crate::validate_sector_io(0, 128, 512, 8),
        Err(BlockError::InvalidBuffer {
            len: 128,
            sector_size: 512,
        })
    );
    assert_eq!(
        crate::validate_sector_io(8, 512, 512, 8),
        Err(BlockError::SectorOutOfRange {
            sector: 8,
            sector_count: 8,
        })
    );
    assert_eq!(crate::validate_sector_io(7, 512, 512, 8), Ok(()));
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
