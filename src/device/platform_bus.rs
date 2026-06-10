// Copyright The SimpleKernel Contributors

//! 平台总线——通过 FDT 遍历发现并探测设备。
//!
//! 遍历设备树中的所有节点，按 driver descriptor 匹配 `compatible` 属性并调用 probe。
//! 参考 Linux `drivers/of/platform.c` 中 `of_platform_bus_create()` 的设计模式。

mod logging;
mod node_set;

use device_core::{
    DriverDescriptor, DriverRegistry, FdtProbeContext, ProbeContext, ProbeKind, ProbeLevel,
    ProbeOutcome, ProbePriority, ProbeRequirement,
};
use platform_fdt::{FdtNodeView, FdtSelector, PlatformFdt};

use super::virtio::{FDT_COMPATIBLES_MMIO, probe_mmio_descriptor};
use logging::{handle_probe_failure, log_probe_outcome, log_probe_summary};
use node_set::FdtNodeSet;

static VIRTIO_MMIO_DRIVER: DriverDescriptor = DriverDescriptor {
    name: "virtio-mmio",
    probe_kind: ProbeKind::Fdt {
        compatibles: FDT_COMPATIBLES_MMIO,
    },
    requirement: ProbeRequirement::Required,
    level: ProbeLevel::Device,
    priority: ProbePriority::DEFAULT,
    probe: probe_mmio_descriptor,
};

static BUILTIN_DRIVERS: &[DriverDescriptor] = &[VIRTIO_MMIO_DRIVER];

/// 扫描 FDT 并探测所有已知设备。
///
/// 当前通过内建 driver descriptor 编排 Static / FDT probe。
///
/// # Panics
/// Full 初始化路径要求 FDT 已由 `early_init()` 记录且可解析；若缺失或解析失败，
/// 表示启动平台契约被破坏，必须 fail-fast，不能静默跳过设备扫描。
pub fn probe_all() {
    let fdt =
        crate::platform_fdt::get().expect("PlatformBus: FDT 未初始化，Full 初始化不能跳过设备扫描");

    let mut registry = DriverRegistry::new(BUILTIN_DRIVERS).unwrap_or_else(|error| {
        panic!("PlatformBus: 内建 driver descriptor 注册失败: {:?}", error)
    });

    probe_static_drivers(&mut registry);
    probe_fdt_drivers(fdt, &mut registry);
    log_probe_summary(&registry);
}

fn probe_static_drivers(registry: &mut DriverRegistry<'_>) {
    for index in 0..registry.drivers().len() {
        let descriptor = *registry.drivers()[index];
        if !matches!(descriptor.probe_kind, ProbeKind::Static) {
            continue;
        }

        let outcome = run_probe(descriptor, ProbeContext::Static);
        record_probe_result(registry, index, descriptor, ProbeContext::Static, outcome);
    }
}

fn probe_fdt_drivers(fdt: &PlatformFdt, registry: &mut DriverRegistry<'_>) {
    let mut bound_nodes = FdtNodeSet::<{ device_core::MAX_REGISTERED_DEVICES }>::new();

    for index in 0..registry.drivers().len() {
        let descriptor = *registry.drivers()[index];
        let compatibles = descriptor.probe_kind.compatibles();
        if compatibles.is_empty() {
            continue;
        }

        for compatible in compatibles {
            fdt.visit_nodes(FdtSelector::Compatible(compatible), |node| {
                if bound_nodes.contains(node.id())
                    || !is_first_descriptor_match(&node, compatibles, compatible)
                {
                    return Ok(());
                }

                let context = build_fdt_probe_context(&node, descriptor.name, compatible);
                let probe_context = ProbeContext::Fdt(context);
                let outcome = run_probe(descriptor, probe_context);
                let bound =
                    record_probe_result(registry, index, descriptor, probe_context, outcome);
                if bound {
                    bound_nodes.insert(node.id(), descriptor.name);
                }
                Ok(())
            })
            .unwrap_or_else(|error| {
                panic!(
                    "PlatformBus: driver={} compatible={} FDT 查询失败: {}",
                    descriptor.name, compatible, error
                )
            });
        }
    }
}

fn is_first_descriptor_match(
    node: &FdtNodeView<'static>,
    descriptor_compatibles: &[&'static str],
    queried_compatible: &'static str,
) -> bool {
    descriptor_compatibles
        .iter()
        .find(|compatible| node.compatibles().contains(compatible))
        .is_some_and(|compatible| *compatible == queried_compatible)
}

fn run_probe(
    descriptor: DriverDescriptor,
    context: ProbeContext,
) -> Result<ProbeOutcome, device_core::ProbeFailure> {
    (descriptor.probe)(context)
}

fn record_probe_result(
    registry: &mut DriverRegistry<'_>,
    index: usize,
    descriptor: DriverDescriptor,
    context: ProbeContext,
    result: Result<ProbeOutcome, device_core::ProbeFailure>,
) -> bool {
    registry.stats_mut()[index].record_match();
    match result {
        Ok(outcome) => {
            registry.stats_mut()[index].record_outcome(outcome);
            log_probe_outcome(descriptor, context, outcome);
            matches!(outcome, ProbeOutcome::Bound { .. })
        }
        Err(failure) => {
            registry.stats_mut()[index].record_failure();
            handle_probe_failure(descriptor, context, failure);
            false
        }
    }
}

fn build_fdt_probe_context(
    node: &FdtNodeView<'static>,
    driver_name: &'static str,
    queried_compatible: &'static str,
) -> FdtProbeContext {
    let reg = node.reg_required().unwrap_or_else(|error| {
        panic!(
            "PlatformBus: driver={} FDT node_id={} name={}@{} compatible={} reg 缺失或非法: {}",
            driver_name,
            node.id().ordinal(),
            node.name().name,
            node.name().unit_address.unwrap_or("<none>"),
            queried_compatible,
            error
        )
    });
    let matched_compatible = node.matched_compatible().unwrap_or_else(|| {
        panic!(
            "PlatformBus: driver={} FDT node_id={} name={}@{} 查询 compatible={} 后缺少 matched compatible",
            driver_name,
            node.id().ordinal(),
            node.name().name,
            node.name().unit_address.unwrap_or("<none>"),
            queried_compatible
        )
    });

    FdtProbeContext {
        node_id: node.id(),
        node_name: node.name(),
        matched_compatible,
        reg,
    }
}
