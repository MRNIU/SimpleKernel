//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]

#[cfg(any(test, target_os = "none"))]
extern crate alloc;

/// 错误类型。
pub mod error;
/// 物理帧分配器与帧生命周期状态机（re-export `frame_allocator` crate）。
#[cfg(any(test, target_os = "none"))]
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
#[cfg(target_os = "none")]
pub use heap_crate as heap;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
#[cfg(target_os = "none")]
pub mod init;
/// 仿射类型映射。
#[cfg(any(test, target_os = "none"))]
pub mod mapped_pages;
/// 类型化 MMIO 区域。
#[cfg(any(test, target_os = "none"))]
pub mod mmio;
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
pub use globals::{
    kernel_address_space, kernel_page_table, store_kernel_address_space, store_kernel_page_table,
};
#[cfg(any(test, target_os = "none"))]
pub use mmio::map_mmio;

#[cfg(any(test, target_os = "none"))]
pub use address::{phys_to_virt, virt_to_phys};

#[cfg(target_os = "none")]
pub use init::{init, init_smp};
