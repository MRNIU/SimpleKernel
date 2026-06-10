// Copyright The SimpleKernel Contributors

//! PlatformBus probe 结果日志输出。

use device_core::{DriverDescriptor, DriverRegistry, ProbeContext, ProbeOutcome, ProbeRequirement};

pub(super) fn log_probe_outcome(
    descriptor: DriverDescriptor,
    context: ProbeContext,
    outcome: ProbeOutcome,
) {
    match (context, outcome) {
        (ProbeContext::Fdt(context), ProbeOutcome::Bound { device_id }) => {
            log::info!(
                "PlatformBus: driver={} bound FDT node_id={} name={}@{} compatible={} reg_addr={:#x} reg_size={:#x} device_id={}",
                descriptor.name,
                context.node_id.ordinal(),
                context.node_name.name,
                context.node_name.unit_address.unwrap_or("<none>"),
                context.matched_compatible,
                context.reg.address,
                context.reg.size,
                device_id.raw()
            );
        }
        (ProbeContext::Fdt(context), ProbeOutcome::Skipped { reason }) => {
            log::debug!(
                "PlatformBus: driver={} skipped FDT node_id={} name={}@{} compatible={} reg_addr={:#x} reg_size={:#x} reason={:?}",
                descriptor.name,
                context.node_id.ordinal(),
                context.node_name.name,
                context.node_name.unit_address.unwrap_or("<none>"),
                context.matched_compatible,
                context.reg.address,
                context.reg.size,
                reason
            );
        }
        (ProbeContext::Static, ProbeOutcome::Bound { device_id }) => {
            log::info!(
                "PlatformBus: static driver={} bound device_id={}",
                descriptor.name,
                device_id.raw()
            );
        }
        (ProbeContext::Static, ProbeOutcome::Skipped { reason }) => {
            log::debug!(
                "PlatformBus: static driver={} skipped reason={:?}",
                descriptor.name,
                reason
            );
        }
    }
}

pub(super) fn handle_probe_failure(
    descriptor: DriverDescriptor,
    context: ProbeContext,
    failure: device_core::ProbeFailure,
) {
    match descriptor.requirement {
        ProbeRequirement::Required => panic_required_probe_failure(descriptor, context, failure),
        ProbeRequirement::Optional => log_optional_probe_failure(descriptor, context, failure),
    }
}

fn panic_required_probe_failure(
    descriptor: DriverDescriptor,
    context: ProbeContext,
    failure: device_core::ProbeFailure,
) -> ! {
    match context {
        ProbeContext::Fdt(context) => {
            panic!(
                "PlatformBus: required driver={} probe 失败: node_id={}, name={}@{}, compatible={}, reg_addr={:#x}, reg_size={:#x}, failure={:?}",
                descriptor.name,
                context.node_id.ordinal(),
                context.node_name.name,
                context.node_name.unit_address.unwrap_or("<none>"),
                context.matched_compatible,
                context.reg.address,
                context.reg.size,
                failure
            )
        }
        ProbeContext::Static => {
            panic!(
                "PlatformBus: required static driver={} probe 失败: failure={:?}",
                descriptor.name, failure
            )
        }
    }
}

fn log_optional_probe_failure(
    descriptor: DriverDescriptor,
    context: ProbeContext,
    failure: device_core::ProbeFailure,
) {
    match context {
        ProbeContext::Fdt(context) => {
            log::warn!(
                "PlatformBus: optional driver={} probe 失败: node_id={}, name={}@{}, compatible={}, reg_addr={:#x}, reg_size={:#x}, failure={:?}",
                descriptor.name,
                context.node_id.ordinal(),
                context.node_name.name,
                context.node_name.unit_address.unwrap_or("<none>"),
                context.matched_compatible,
                context.reg.address,
                context.reg.size,
                failure
            );
        }
        ProbeContext::Static => {
            log::warn!(
                "PlatformBus: optional static driver={} probe 失败: failure={:?}",
                descriptor.name,
                failure
            );
        }
    }
}

pub(super) fn log_probe_summary(registry: &DriverRegistry<'_>) {
    for stats in registry.stats() {
        log::info!(
            "PlatformBus: driver={} matched={} bound={} skipped={} failed={}",
            stats.driver_name,
            stats.matched,
            stats.bound,
            stats.skipped,
            stats.failed
        );
    }
}
