// Copyright The SimpleKernel Contributors

//! 架构相关系统测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_kernel_stack_alignment();
    log::info!("test kernel_stack_alignment ... ok");

    test_tick_interval_contract();
    log::info!("test tick_interval_contract ... ok");

    test_cpu_topology_contract();
    log::info!("test cpu_topology_contract ... ok");

    test_irq_exit_preemption_contract();
    log::info!("test irq_exit_preemption_contract ... ok");

    test_aarch64_fp_arithmetic();
    log::info!("test aarch64_fp_arithmetic ... ok");

    log::info!("arch-test: all tests passed");
}

/// 内核线程栈顶必须满足 RISC-V psABI 和 AAPCS64 的 16 字节对齐要求。
fn test_kernel_stack_alignment() {
    let stack = simplekernel::task::tcb::KernelStack::new();

    assert_eq!(simplekernel::task::tcb::KernelStack::ALIGN, 16);
    assert_eq!(
        stack.top() % simplekernel::task::tcb::KernelStack::ALIGN,
        0,
        "KernelStack 栈顶未按 16 字节对齐: top={:#x}",
        stack.top()
    );
}

/// timer tick interval 必须非零，避免启动后进入中断风暴或静默停摆。
fn test_tick_interval_contract() {
    let interval = simplekernel::timer::checked_tick_interval(config::TIMER_FREQ_HZ * 10);
    assert_eq!(interval, 10);
}

/// CPU topology 必须显式校验当前 dense core id 平台契约。
fn test_cpu_topology_contract() {
    let discovered = *simplekernel::CORE_COUNT
        .get()
        .expect("arch-test: CORE_COUNT 未初始化");
    let topology = simplekernel::cpu_topology::topology();

    assert_eq!(
        topology.discovered_core_count(),
        discovered,
        "CPU topology 发现核心数必须与 CORE_COUNT 一致"
    );
    assert!(
        topology.primary_core_id() < discovered,
        "primary core id 必须落在 dense CPU 表内: primary={}, discovered={}",
        topology.primary_core_id(),
        discovered
    );
    assert_eq!(
        simplekernel::timer::timekeeper_core_id(),
        topology.primary_core_id(),
        "timer timekeeper 必须跟随 primary core"
    );

    assert!(
        per_cpu::current_core_id() < discovered,
        "当前 core id 必须落在 dense CPU 表内: current={}, discovered={}",
        per_cpu::current_core_id(),
        discovered
    );
}

/// IRQ-exit 抢占判定必须只消费一次 pending reschedule 标志。
fn test_irq_exit_preemption_contract() {
    simplekernel::preempt::request_current_core_reschedule();
    assert!(
        simplekernel::preempt::take_irq_exit_preemption_request(),
        "设置 need_resched 后，IRQ-exit 抢占判定应返回 true"
    );
    assert!(
        !simplekernel::preempt::take_irq_exit_preemption_request(),
        "IRQ-exit 抢占判定必须原子消费 need_resched"
    );
}

/// AArch64 hardfloat 目标应能执行硬件浮点运算。
#[cfg(target_arch = "aarch64")]
fn test_aarch64_fp_arithmetic() {
    let lhs = core::hint::black_box(1.25_f64);
    let rhs = core::hint::black_box(2.5_f64);
    let value = fp_expression(lhs, rhs);

    assert!(
        (value - 7.5).abs() < f64::EPSILON,
        "AArch64 浮点计算结果错误: value={}",
        value
    );
}

/// 非 AArch64 架构不执行本测试。
#[cfg(not(target_arch = "aarch64"))]
fn test_aarch64_fp_arithmetic() {}

/// 产生一段不可在编译期完全折叠的浮点表达式。
#[cfg(target_arch = "aarch64")]
#[inline(never)]
fn fp_expression(lhs: f64, rhs: f64) -> f64 {
    let sum = core::hint::black_box(lhs + rhs);
    sum * core::hint::black_box(2.0_f64)
}
