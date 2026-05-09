// Copyright The SimpleKernel Contributors

//! CPU topology 平台契约校验。
//!
//! 当前 SimpleKernel 明确只支持 CPU id 为 `0..core_count` 的 dense 单调编号。
//! FDT CPU 表用于启动期校验和异常诊断，不在运行期建立 logical/hardware remap。

/// 当前支持的 CPU topology 摘要。
#[derive(Clone, Copy)]
pub struct CpuTopology {
    discovered_core_count: usize,
    primary_core_id: usize,
}

impl CpuTopology {
    /// FDT 中发现的 CPU 数量。
    pub fn discovered_core_count(&self) -> usize {
        self.discovered_core_count
    }

    /// 当前启动 CPU 的 core id。
    pub fn primary_core_id(&self) -> usize {
        self.primary_core_id
    }
}

static CPU_TOPOLOGY: spin::Once<CpuTopology> = spin::Once::new();

/// 打印 FDT CPU id 表，供 topology fail-fast 诊断使用。
fn log_cpu_id_table(ids: &[usize]) {
    for (entry, core_id) in ids.iter().copied().enumerate() {
        log::error!("CPU topology: FDT cpu[{}] core_id={}", entry, core_id);
    }
}

/// 校验 CPU id 表满足当前平台契约：`0..core_count` dense、无重复、无空洞。
///
/// # Panics
/// 当 CPU 列表为空、CPU 数超过 [`config::MAX_CORE_COUNT`]、id 重复、
/// id 超出 `0..core_count`，或表中存在空洞时 panic。
pub fn validate_dense_core_ids(ids: &[usize]) {
    assert!(!ids.is_empty(), "CPU topology: CPU 列表不能为空");
    assert!(
        ids.len() <= config::MAX_CORE_COUNT,
        "CPU topology: CPU 数 {} 超出 MAX_CORE_COUNT {}",
        ids.len(),
        config::MAX_CORE_COUNT
    );

    let mut seen = [false; config::MAX_CORE_COUNT];
    for (entry, core_id) in ids.iter().copied().enumerate() {
        if core_id >= ids.len() {
            log_cpu_id_table(ids);
            panic!(
                "CPU topology: 当前只支持 dense CPU id 0..{}，FDT cpu[{}] 给出 core_id={}",
                ids.len(),
                entry,
                core_id
            );
        }
        if seen[core_id] {
            log_cpu_id_table(ids);
            panic!("CPU topology: FDT CPU id {} 重复", core_id);
        }
        seen[core_id] = true;
    }

    for (expected, present) in seen.iter().copied().take(ids.len()).enumerate() {
        if !present {
            log_cpu_id_table(ids);
            panic!("CPU topology: FDT CPU 表缺少 dense core_id={}", expected);
        }
    }
}

/// 从 FDT 初始化并校验当前支持的 CPU topology。
///
/// 返回 FDT 中发现并通过校验的 CPU 数量。
///
/// # Panics
/// 当 FDT CPU 表无法解析、CPU id 不满足 dense 平台契约、primary core id
/// 不在 FDT CPU 表中，或 topology 被重复初始化时 panic。
pub fn init_from_fdt(fdt: &crate::fdt::KernelFdt<'_>) -> usize {
    let ids = fdt
        .cpu_hardware_ids()
        .expect("CPU topology: FDT CPU id 解析失败");
    validate_dense_core_ids(&ids);

    let primary_core_id = per_cpu::current_core_id();
    if primary_core_id >= ids.len() {
        log_cpu_id_table(&ids);
        panic!(
            "CPU topology: primary core_id={} 不在 FDT dense CPU 表 0..{} 中",
            primary_core_id,
            ids.len()
        );
    }

    crate::timer::init_timekeeper(primary_core_id);
    CPU_TOPOLOGY.call_once(|| CpuTopology {
        discovered_core_count: ids.len(),
        primary_core_id,
    });

    for &core_id in &ids {
        log::info!("CPU topology: supported dense core {}", core_id);
    }

    ids.len()
}

/// 返回已校验的 CPU topology 摘要。
///
/// # Panics
/// 当 CPU topology 尚未初始化时 panic。
pub fn topology() -> &'static CpuTopology {
    CPU_TOPOLOGY.get().expect("CPU topology 尚未初始化")
}
