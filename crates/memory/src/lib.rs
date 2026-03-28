//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]
#![cfg_attr(any(test, target_os = "none"), allow(incomplete_features))]
#![cfg_attr(any(test, target_os = "none"), feature(adt_const_params))]

#[cfg(any(test, target_os = "none"))]
extern crate alloc;

/// 地址转换与映射工具。
#[cfg(any(test, target_os = "none"))]
pub mod addr_conv;
/// 错误类型。
pub mod error;
/// 物理帧分配器与帧生命周期状态机。
#[cfg(any(test, target_os = "none"))]
pub mod frame;
/// 全局内存状态。
pub mod globals;
/// 堆分配器。
///
/// `#[global_allocator]` 在宿主机上会与系统分配器冲突，因此门控为裸机专用。
#[cfg(target_os = "none")]
pub mod heap;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
#[cfg(target_os = "none")]
pub mod init;
/// 仿射类型映射（Theseus 风格 `MappedPages`）。
#[cfg(any(test, target_os = "none"))]
pub mod mapped_pages;
/// 类型化 MMIO 区域。
#[cfg(any(test, target_os = "none"))]
pub mod mmio;
/// 虚拟页分配器。
#[cfg(any(test, target_os = "none"))]
pub mod page_allocator;
/// 多级页表与架构原生 PTE 标志位。
pub mod page_table;
/// TLB 刷新。
pub mod tlb;
/// 虚拟内存区域（VMA）与地址空间管理。
#[cfg(any(test, target_os = "none"))]
pub mod vma;

// 公共 API re-export——保持外部调用方的 `memory::Xxx` 路径不变。
pub use globals::{MEMORY_INFO, MemoryInfo};
#[cfg(any(test, target_os = "none"))]
pub use globals::{kernel_page_table, map_mmio, store_kernel_page_table};

#[cfg(any(test, target_os = "none"))]
pub use addr_conv::{phys_to_virt, virt_to_phys};

#[cfg(target_os = "none")]
pub use init::{init, init_smp};
