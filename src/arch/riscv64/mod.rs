pub mod console;
pub mod context;
pub mod interrupt;
pub mod ipi;
pub mod pte;
pub mod syscall;
pub mod timer;

use super::ArchOps;

/// RISC-V 64 架构实现
pub struct Riscv64;

impl ArchOps for Riscv64 {
    #[inline]
    fn dtb_addr(_argc: i32, argv: *const *const u8) -> usize {
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
    fn secondary_core_id(argc: i32, _argv: *const *const u8) -> usize {
        // SBI hart_start 将 hart_id 放入 a0（编码为 argc）
        argc as usize
    }

    #[inline]
    fn core_id() -> usize {
        let id: usize;
        // SAFETY: tp 寄存器在 boot.S 中由 mv tp, a0 设置为 hart ID
        unsafe { core::arch::asm!("mv {id}, tp", id = out(reg) id) };
        id
    }

    #[inline]
    fn irq_enabled() -> bool {
        riscv::register::sstatus::read().sie()
    }

    #[inline]
    fn irq_disable() {
        riscv::interrupt::supervisor::disable();
    }

    #[inline]
    unsafe fn irq_enable() {
        unsafe { riscv::interrupt::supervisor::enable() };
    }

    #[inline]
    fn flush_tlb() {
        // SAFETY: sfence.vma 是 S-mode 特权指令
        unsafe { core::arch::asm!("sfence.vma") };
    }

    fn map_early_mmio(_pt: &mut crate::memory::page_table::PageTable) -> crate::error::KResult<()> {
        // RISC-V console 通过 SBI ecall（M-mode），无需 MMIO 映射
        Ok(())
    }

    unsafe fn activate_page_table(pt: &crate::memory::page_table::PageTable) {
        let ppn = pt.root_paddr().as_usize() >> 12;
        let satp = (8usize << 60) | ppn; // MODE = 8 → Sv39
        // SAFETY: 调用方保证页表映射正确
        unsafe {
            core::arch::asm!(
                "csrw satp, {satp}",
                "sfence.vma",
                satp = in(reg) satp,
            );
        }
    }

    fn console_write(s: &str) {
        for byte in s.bytes() {
            sbi_rt::console_write_byte(byte);
        }
    }

    #[inline]
    fn get_current_tick() -> u64 {
        timer::get_current_tick()
    }

    #[inline]
    fn ticks_per_second() -> u64 {
        timer::ticks_per_second()
    }
}

/// 内核线程引导函数（供 switch.S 中 `kernel_thread_entry` 调用）
///
/// 新任务首次被 `switch_to` 调度运行时，从此函数开始执行。
/// 负责释放调度锁、调用入口函数、最终退出任务。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    // 释放 schedule() 中通过 lock_raw 获取的 sched_lock
    // SAFETY: schedule() 在 switch_to 前获取了 sched_lock，
    // 新任务首次运行时负责释放
    unsafe { crate::task::release_sched_lock() };

    // 调用入口函数
    // SAFETY: entry 是由 new_kernel_thread 编码的合法 fn(usize) 指针
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);

    // 入口函数返回 → 退出任务
    crate::task::exit(0);
}
