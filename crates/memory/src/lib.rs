//! 内核内存管理门面——统一编排子系统初始化、提供 MMIO 类型化访问。

#![no_std]

pub mod globals;
pub mod init;
pub mod mmio;

pub use globals::{MEMORY_INFO, MemoryInfo};
pub use init::{init, init_smp};
pub use mmio::MmioRegion;
