// Copyright The SimpleKernel Contributors

mod boot;
pub mod console;
pub mod context;
pub mod interrupt;
pub mod ipi;
mod mmu;
pub mod pte;
pub mod switch;
pub mod timer;

use super::ArchOps;

/// RISC-V 64 架构实现
pub struct Riscv64;

impl ArchOps for Riscv64 {
    #[inline]
    unsafe fn dtb_addr(_argc: i32, argv: *const *const u8) -> usize {
        // OpenSBI 传递 a0=hart_id, a1=DTB 地址
        // boot.S 将 a1 作为第二个 C 参数（argv）转发
        argv as usize
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
        // RISC-V console 通过 SBI ecall（M-mode），无需 MMIO 映射
    }

    unsafe fn activate_page_table(pt: &paging::PageTable) {
        // SAFETY: trait 调用方保证页表已经建立了启用 MMU 所需映射。
        unsafe { mmu::activate_page_table(pt) };
    }

    fn console_write(s: &str) {
        for byte in s.bytes() {
            sbi_rt::console_write_byte(byte);
        }
    }
}

// kernel_thread_bootstrap 已迁移到 main.rs（打破 arch→task 循环依赖）
