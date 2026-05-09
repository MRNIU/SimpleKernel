mod boot;
pub mod console;
pub mod context;
pub mod init;
pub mod interrupt;
pub mod ipi;
mod mmu;
pub mod pte;
pub mod switch;
pub mod timer;

use super::ArchOps;

/// QEMU virt 平台 PL011 UART MMIO 基地址
///
/// 用于 early console，此时 FDT 尚未解析完成。
/// 更换平台时需修改此常量。
const PL011_BASE: usize = 0x0900_0000;

/// PL011 MMIO 区域大小（1 页）
const PL011_SIZE: usize = 0x1000;

/// AArch64 架构实现
pub struct Aarch64;

impl ArchOps for Aarch64 {
    unsafe fn dtb_addr(argc: i32, argv: *const *const u8) -> usize {
        // U-Boot 将 DTB 地址作为 argv[2] 的十六进制字符串传入
        // SAFETY: 调用方保证 argc/argv 来自当前 AArch64 boot 入口。
        unsafe { init::dtb_addr_from_argv(argc, argv) }
    }

    #[inline]
    fn init_interrupt() {
        interrupt::init();
    }

    #[inline]
    fn init_interrupt_smp() {
        interrupt::init_smp();
    }

    #[inline]
    fn init_timer() {
        timer::init();
    }

    #[inline]
    fn init_timer_smp(core_id: usize) {
        timer::init_smp(core_id);
    }

    #[inline]
    fn wake_secondary_cores() {
        ipi::wake_secondary_cores();
    }

    #[inline]
    fn send_ipi(core_id: usize) {
        ipi::send_ipi(core_id);
    }

    fn map_early_mmio() {
        // PL011 UART —— 通过 memory 门面建立 MMIO identity map
        // （VA == PA，Device-nGnRnE 属性，含 RAM 重叠校验）。
        memory::MmioRegion::map(memory_types::PhysAddr::new(PL011_BASE), PL011_SIZE);
        log::info!("MemoryInit: mapped PL011 UART @ {:#010X}", PL011_BASE);
    }

    unsafe fn activate_page_table(pt: &paging::PageTable) {
        // SAFETY: trait 调用方保证页表已经建立了启用 MMU 所需映射。
        unsafe { mmu::activate_page_table(pt) };
    }

    fn console_write(s: &str) {
        console::puts(s);
    }
}

// kernel_thread_bootstrap 已迁移到 main.rs（打破 arch→task 循环依赖）
