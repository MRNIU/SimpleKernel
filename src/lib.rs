#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(alloc_error_handler))]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]
// 测试模式下部分模块不编译（arch, fdt, lang_items），导致它们的消费者
// 产生 dead_code 警告。这些代码在目标架构上被正常使用。
#![cfg_attr(test, allow(dead_code))]

extern crate alloc;

/// 实际在线核心数（从 FDT 解析，`early_init` 中初始化）。
///
/// 与 `config::MAX_CORE_COUNT`（编译期上限）不同，此值为运行时实际核心数。
pub static CORE_COUNT: spin::Once<usize> = spin::Once::new();

#[cfg(target_os = "none")]
pub mod arch;
pub mod elf;
#[cfg(target_os = "none")]
pub mod fdt;
#[cfg(target_os = "none")]
pub mod init;
pub mod irq_context;
#[cfg(target_os = "none")]
pub mod lang_items;
pub mod logging;
pub mod panic;
pub mod preempt;
pub mod syscall;
pub mod task;
#[cfg(target_os = "none")]
pub mod timer;
pub mod util;
