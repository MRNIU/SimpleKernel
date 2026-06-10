// Copyright The SimpleKernel Contributors

//! driver descriptor 集合校验、排序和 probe 统计。

use core::cmp::Ordering;

use heapless::Vec;

use crate::{DriverDescriptor, ProbeOutcome};

use super::{MAX_DRIVER_DESCRIPTORS, RegistryError};

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
        for descriptor in &drivers {
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

fn compare_descriptors(left: &DriverDescriptor, right: &DriverDescriptor) -> Ordering {
    left.level
        .cmp(&right.level)
        .then(left.priority.cmp(&right.priority))
        .then(left.name.cmp(right.name))
}
