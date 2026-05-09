// Copyright The SimpleKernel Contributors

//! 架构相关系统测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[cfg(target_arch = "riscv64")]
use core::sync::atomic::{AtomicBool, Ordering};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

#[cfg(target_arch = "riscv64")]
core::arch::global_asm!(
    r#"
    .section .text
    .option push
    .option arch, +d
    .global arch_test_write_fs0_bits
    .type arch_test_write_fs0_bits, @function
arch_test_write_fs0_bits:
    fmv.d.x fs0, a0
    ret

    .global arch_test_read_fs0_bits
    .type arch_test_read_fs0_bits, @function
arch_test_read_fs0_bits:
    fmv.x.d a0, fs0
    ret
    .option pop
"#
);

#[cfg(target_arch = "riscv64")]
unsafe extern "C" {
    fn arch_test_write_fs0_bits(bits: u64);
    fn arch_test_read_fs0_bits() -> u64;
}

#[cfg(target_arch = "riscv64")]
static RISCV64_FP_CHILD_DONE: AtomicBool = AtomicBool::new(false);

fn run_tests() {
    test_kernel_stack_alignment();
    log::info!("test kernel_stack_alignment ... ok");

    test_tick_interval_contract();
    log::info!("test tick_interval_contract ... ok");

    test_absolute_deadline_contract();
    log::info!("test_absolute_deadline_contract ... ok");

    test_cpu_topology_contract();
    log::info!("test cpu_topology_contract ... ok");

    test_smp_online_barrier_contract();
    log::info!("test smp_online_barrier_contract ... ok");

    test_irq_exit_preemption_contract();
    log::info!("test irq_exit_preemption_contract ... ok");

    test_aarch64_fp_arithmetic();
    log::info!("test aarch64_fp_arithmetic ... ok");

    test_riscv64_fp_arithmetic();
    log::info!("test riscv64_fp_arithmetic ... ok");

    test_riscv64_fp_context_switch();
    log::info!("test riscv64_fp_context_switch ... ok");

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

/// 方案 B 要求硬件 deadline 跳到未来，但每次 timer interrupt 仍只对应一个逻辑 tick。
fn test_absolute_deadline_contract() {
    assert_eq!(
        simplekernel::timer::next_absolute_deadline(100, 99, 10),
        100
    );
    assert_eq!(
        simplekernel::timer::next_absolute_deadline(100, 100, 10),
        110
    );
    assert_eq!(
        simplekernel::timer::next_absolute_deadline(100, 129, 10),
        130
    );
    assert_eq!(
        simplekernel::timer::next_absolute_deadline(100, 130, 10),
        140
    );
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

/// Full 初始化返回后，所有 FDT 发现的核心都必须已经完成 SMP online。
fn test_smp_online_barrier_contract() {
    let discovered = *simplekernel::CORE_COUNT
        .get()
        .expect("arch-test: CORE_COUNT 未初始化");

    assert!(
        simplekernel::tlb_shootdown::all_discovered_cores_online(),
        "Full 初始化返回时所有 discovered CPU 都必须 online: discovered={}, online={}",
        discovered,
        simplekernel::tlb_shootdown::online_core_count()
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

/// RISC-V `gc` 目标应能在 S-mode 执行硬件浮点运算。
#[cfg(target_arch = "riscv64")]
fn test_riscv64_fp_arithmetic() {
    let lhs = core::hint::black_box(3.5_f64);
    let rhs = core::hint::black_box(1.25_f64);
    let value = riscv64_fp_expression(lhs, rhs);

    assert!(
        (value - 5.625).abs() < f64::EPSILON,
        "RISC-V 浮点计算结果错误: value={}",
        value
    );
}

/// 非 RISC-V 架构不执行本测试。
#[cfg(not(target_arch = "riscv64"))]
fn test_riscv64_fp_arithmetic() {}

/// 产生一段不可在编译期完全折叠的 RISC-V 浮点表达式。
#[cfg(target_arch = "riscv64")]
#[inline(never)]
fn riscv64_fp_expression(lhs: f64, rhs: f64) -> f64 {
    let sum = core::hint::black_box(lhs + rhs);
    sum + core::hint::black_box(0.875_f64)
}

/// RISC-V 任务切换必须保存/恢复 callee-saved 浮点寄存器 `fs0`。
#[cfg(target_arch = "riscv64")]
fn test_riscv64_fp_context_switch() {
    const PARENT_VALUE: f64 = 11.5;

    RISCV64_FP_CHILD_DONE.store(false, Ordering::Release);

    // SAFETY: 测试在 RISC-V `gc` 目标下运行，`fs0` 用作被调用者保存浮点寄存器。
    unsafe { arch_test_write_fs0_bits(PARENT_VALUE.to_bits()) };

    simplekernel::task::spawn_kernel_thread("riscv64-fp-child", riscv64_fp_child, 0);
    simplekernel::syscall::process::yield_now();

    assert!(
        RISCV64_FP_CHILD_DONE.load(Ordering::Acquire),
        "RISC-V FP 子任务未运行完成"
    );

    // SAFETY: 与写入侧相同，读取当前任务的 `fs0` 原始位模式用于验证上下文恢复。
    let actual = unsafe { arch_test_read_fs0_bits() };
    assert_eq!(
        actual,
        PARENT_VALUE.to_bits(),
        "RISC-V 任务切换未恢复父任务 fs0: actual={:#x}, expected={:#x}",
        actual,
        PARENT_VALUE.to_bits()
    );
}

/// 子任务覆盖 `fs0`，用于验证父任务切回后是否恢复自己的 FP 状态。
#[cfg(target_arch = "riscv64")]
fn riscv64_fp_child(_: usize) {
    const CHILD_VALUE: f64 = 42.25;

    // SAFETY: 测试在 RISC-V `gc` 目标下运行，故意覆盖 `fs0` 以暴露上下文切换缺口。
    unsafe { arch_test_write_fs0_bits(CHILD_VALUE.to_bits()) };

    let _ = riscv64_fp_expression(
        core::hint::black_box(7.0_f64),
        core::hint::black_box(8.0_f64),
    );
    RISCV64_FP_CHILD_DONE.store(true, Ordering::Release);
}

/// 非 RISC-V 架构不执行本测试。
#[cfg(not(target_arch = "riscv64"))]
fn test_riscv64_fp_context_switch() {}
