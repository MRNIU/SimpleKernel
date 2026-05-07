//! 全局内存状态——`MemoryInfo`。

use memory_types::PhysAddr;

/// 内核启动时从 FDT 解析出的内存布局信息。
///
/// 在 `early_init()` 中通过 `MEMORY_INFO.call_once()` 填充，
/// 之后由内存子系统只读访问。
pub struct MemoryInfo {
    pub physical_memory_addr: PhysAddr,
    pub physical_memory_size: usize,
    pub kernel_addr: PhysAddr,
    pub kernel_size: usize,
    pub firmware_reserved_addr: PhysAddr,
    pub firmware_reserved_size: usize,
}

/// 全局内存布局信息（一次性初始化，之后只读）。
pub static MEMORY_INFO: spin::Once<MemoryInfo> = spin::Once::new();
