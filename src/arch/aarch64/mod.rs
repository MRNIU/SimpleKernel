pub mod console;
pub mod context;
pub mod init;
pub mod interrupt;
pub mod ipi;
pub mod pte;
pub mod syscall;
pub mod timer;

use super::ArchOps;

/// AArch64 架构实现
pub struct Aarch64;

impl ArchOps for Aarch64 {
    fn dtb_addr(_argc: i32, argv: *const *const u8) -> usize {
        // U-Boot 将 DTB 地址作为 argv[2] 的十六进制字符串传入
        init::dtb_addr_from_argv(argv)
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
    fn secondary_core_id(_argc: i32, _argv: *const *const u8) -> usize {
        // PSCI cpu_on 后从 MPIDR_EL1 读取
        Self::core_id()
    }

    #[inline]
    fn core_id() -> usize {
        let mpidr: u64;
        // SAFETY: MPIDR_EL1 在 EL1 下始终可读
        unsafe { core::arch::asm!("mrs {mpidr}, mpidr_el1", mpidr = out(reg) mpidr) };
        (mpidr & 0xFF) as usize
    }

    #[inline]
    fn irq_enabled() -> bool {
        let daif: u64;
        // SAFETY: DAIF 在 EL1 下可读
        unsafe { core::arch::asm!("mrs {daif}, daif", daif = out(reg) daif) };
        (daif & (1 << 7)) == 0
    }

    #[inline]
    fn irq_disable() {
        // SAFETY: msr daifset 是 EL1 特权指令
        unsafe { core::arch::asm!("msr daifset, #2") };
    }

    #[inline]
    unsafe fn irq_enable() {
        // SAFETY: msr daifclr 是 EL1 特权指令
        unsafe { core::arch::asm!("msr daifclr, #2") };
    }

    #[inline]
    fn flush_tlb() {
        // SAFETY: tlbi/dsb/isb 是 EL1 特权指令
        unsafe { core::arch::asm!("tlbi vmalle1", "dsb sy", "isb") };
    }

    fn map_early_mmio(pt: &mut crate::memory::page_table::PageTable) -> crate::error::KResult<()> {
        use crate::memory::address::PhysAddr;
        use crate::memory::page_table::PageFlags;
        // PL011 UART @ 0x0900_0000, 1 页 —— console 直接 MMIO 访问
        let start = PhysAddr::new(0x0900_0000);
        let end = PhysAddr::new(0x0900_1000);
        crate::memory::identity_map_range(pt, start, end, PageFlags::kernel_rw())?;
        log::info!("MemoryInit: mapped PL011 UART @ 0x09000000");
        Ok(())
    }

    unsafe fn activate_page_table(pt: &crate::memory::page_table::PageTable) {
        let ttbr = pt.root_paddr().as_usize() as u64;

        // MAIR_EL1: 定义内存属性索引
        //   Attr0 = 0xFF: Normal, Write-Back, Read/Write-Allocate（内核代码/数据）
        //   Attr1 = 0x00: Device-nGnRnE（MMIO 设备寄存器）
        let mair: u64 = 0xFF | (0x00 << 8);

        // TCR_EL1: 翻译控制
        //   T0SZ  = 16  → 48 位虚拟地址空间（bits [5:0]）
        //   TG0   = 0b00 → 4KB granule（bits [15:14]）
        //   SH0   = 0b11 → Inner Shareable（bits [13:12]）
        //   ORGN0 = 0b01 → Outer Write-Back, Write-Allocate（bits [11:10]）
        //   IRGN0 = 0b01 → Inner Write-Back, Write-Allocate（bits [9:8]）
        let tcr: u64 = 16 // T0SZ = 16
            | (0b01 << 8)  // IRGN0
            | (0b01 << 10) // ORGN0
            | (0b11 << 12) // SH0
            | (0b00 << 14); // TG0 = 4KB

        // SAFETY: 调用方保证页表映射正确；MAIR/TCR 必须在写入 TTBR 并使能 MMU 前配置
        unsafe {
            core::arch::asm!(
                "msr mair_el1, {mair}",
                "msr tcr_el1, {tcr}",
                "isb",
                "msr ttbr0_el1, {ttbr}",
                "isb",
                "tlbi vmalle1",
                "dsb sy",
                "isb",
                mair = in(reg) mair,
                tcr = in(reg) tcr,
                ttbr = in(reg) ttbr,
            );
            // 使能 MMU（SCTLR_EL1.M，bit 0）
            let mut sctlr: u64;
            core::arch::asm!("mrs {sctlr}, sctlr_el1", sctlr = out(reg) sctlr);
            sctlr |= 1;
            core::arch::asm!(
                "msr sctlr_el1, {sctlr}",
                "isb",
                sctlr = in(reg) sctlr,
            );
        }
    }

    fn console_write(s: &str) {
        console::puts(s);
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
