//! 架构相关系统测试。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_aarch64_fp_arithmetic();
    log::info!("test aarch64_fp_arithmetic ... ok");

    log::info!("arch-test: all tests passed");
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
