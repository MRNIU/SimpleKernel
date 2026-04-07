//! Per-CPU 数据——通过 `#[cpu_local]` 分散声明，每核心独立副本。
//!
//! 本 crate 只提供 per-CPU 机制（声明、初始化、访问），
//! 不包含任何业务变量——各子系统在自己的模块中用 `#[cpu_local]` 声明。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `CORE_ID` | `usize` | 当前核心 ID（`percpu_init` 写入） |
//!
//! ## 原理
//!
//! 1. `#[cpu_local]` 将变量放入 `.percpu` ELF section（模板）
//! 2. `percpu_init()` 将模板复制 N 份（每 CPU 一份）到 BSS 预留区
//! 3. per-CPU 基地址寄存器指向当前 CPU 的副本
//! 4. 访问：`base + (模板地址 - __percpu_start)` = 当前 CPU 的变量地址

#![no_std]
#![cfg_attr(bare_metal, feature(sync_unsafe_cell))]

// 让 #[cpu_local] 宏展开的 `per_cpu::CpuLocal` 路径在本 crate 内部也能解析
extern crate self as per_cpu;

pub use macros::cpu_local;

/// Per-CPU 变量的包装器。
///
/// 数据在 `.percpu` section 的模板中，初始化后每 CPU 各有一份副本，
/// 通过基地址寄存器 + 偏移量访问。
pub struct CpuLocal<T: Sync> {
    /// 指向模板变量的指针。
    template_ptr: *const T,
}

// SAFETY: CpuLocal 只是偏移计算器，T: Sync 保证跨线程共享安全
unsafe impl<T: Sync> Send for CpuLocal<T> {}
unsafe impl<T: Sync> Sync for CpuLocal<T> {}

impl<T: Sync> CpuLocal<T> {
    /// 由 `#[cpu_local]` 宏调用，不应手动使用。
    ///
    /// # Safety
    /// `ptr` 必须指向 `.percpu` section 中由宏生成的 static 变量。
    #[doc(hidden)]
    pub const unsafe fn __new(ptr: *const T) -> Self {
        Self { template_ptr: ptr }
    }
}

mod bare_metal;
pub use bare_metal::{current_core_id, percpu_init, percpu_init_smp};

/// 当前核心 ID（由 `percpu_init()` 写入每个 CPU 的区域）。
#[cpu_local]
static CORE_ID: usize = 0;
