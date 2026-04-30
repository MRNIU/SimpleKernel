mod boot;
pub mod console;
pub mod context;
pub mod init;
pub mod interrupt;
pub mod ipi;
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

// kernel_thread_bootstrap 已迁移到 main.rs（打破 arch→task 循环依赖）
