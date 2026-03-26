use crate::memory::address::PhysAddr;
use spin::Once;

/// 内核启动时从 FDT 解析出的基本信息。
///
/// 在 `early_init()` 中通过 `BASIC_INFO.call_once()` 填充，
/// 之后由各子系统只读访问。
pub struct BasicInfo {
    pub physical_memory_addr: PhysAddr,
    pub physical_memory_size: usize,
    pub kernel_addr: PhysAddr,
    pub kernel_size: usize,
    pub elf_addr: PhysAddr,
    pub fdt_addr: PhysAddr,
    pub core_count: usize,
    /// 定时器硬件频率（Hz）——RISC-V 从 FDT `timebase-frequency` 读取，
    /// AArch64 从 `CNTFRQ_EL0` 读取。0 表示未知。
    pub timer_freq: u64,
}

impl BasicInfo {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            physical_memory_addr: PhysAddr::new(0),
            physical_memory_size: 0,
            kernel_addr: PhysAddr::new(0),
            kernel_size: 0,
            elf_addr: PhysAddr::new(0),
            fdt_addr: PhysAddr::new(0),
            core_count: 0,
            timer_freq: 0,
        }
    }
}

pub static BASIC_INFO: Once<BasicInfo> = Once::new();
