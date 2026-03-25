/// RISC-V 64 中断子系统
///
/// 负责 PLIC 初始化、stvec 设置，以及陷阱分发（定时器、外部中断、IPI、系统调用、异常）。
use crate::memory::address::PhysAddr;
use crate::memory::map_mmio;

use super::context::TrapContext;

// trap_entry 由 interrupt.S 提供
// SAFETY: 链接器保证该符号存在
unsafe extern "C" {
    fn trap_entry();
}

// ─────────────────────────────────────────────────────────────────────────────
// PLIC 地址常量与辅助函数
// ─────────────────────────────────────────────────────────────────────────────

/// PLIC 基地址（QEMU virt 平台）
const PLIC_BASE: usize = 0x0C00_0000;

/// PLIC 映射大小（4 MB，覆盖优先级、使能、阈值、claim 寄存器）
const PLIC_SIZE: usize = 0x0040_0000;

/// UART 中断号（QEMU virt 平台）
const UART_IRQ: u32 = 10;

/// hart 0 S-mode 上下文编号（S-mode context = 2*hart + 1）
const PLIC_S_CONTEXT_HART0: usize = 1;

/// 写 PLIC 32 位寄存器
///
/// # Safety
/// 调用前必须已通过 `map_mmio` 映射 PLIC 区域。
#[inline]
unsafe fn plic_write32(offset: usize, val: u32) {
    let addr = (PLIC_BASE + offset) as *mut u32;
    // SAFETY: 调用方保证地址已映射且对齐
    unsafe { core::ptr::write_volatile(addr, val) };
}

/// 读 PLIC 32 位寄存器
///
/// # Safety
/// 调用前必须已通过 `map_mmio` 映射 PLIC 区域。
#[inline]
unsafe fn plic_read32(offset: usize) -> u32 {
    let addr = (PLIC_BASE + offset) as *const u32;
    // SAFETY: 调用方保证地址已映射且对齐
    unsafe { core::ptr::read_volatile(addr) }
}

/// 初始化 PLIC
///
/// 1. 将 PLIC 寄存器区域 identity-map 进内核页表
/// 2. 设置 UART IRQ（10）优先级为 1
/// 3. 使能 hart 0 S-mode 上下文中的 UART IRQ
/// 4. 设置 hart 0 S-mode 上下文阈值为 0（接受所有优先级 ≥1 的中断）
fn plic_init() {
    // Step 1: 映射 PLIC MMIO 区域
    map_mmio(PhysAddr::new(PLIC_BASE), PLIC_SIZE).expect("plic_init: 映射 PLIC MMIO 失败");

    // SAFETY: PLIC 已通过 map_mmio 映射
    unsafe {
        // Step 2: 设置 UART IRQ 优先级（偏移 = IRQ * 4）
        // 优先级寄存器: base + 0x000000 + irq*4
        plic_write32(UART_IRQ as usize * 4, 1);

        // Step 3: 使能 IRQ 10 — hart 0 S-mode 上下文
        // 使能寄存器: base + 0x002000 + context*0x80 + (irq/32)*4
        // context=1, irq=10: offset = 0x2000 + 1*0x80 + 0 = 0x2080
        let enable_offset =
            0x0002_0000 + PLIC_S_CONTEXT_HART0 * 0x80 + (UART_IRQ as usize / 32) * 4;
        let current = plic_read32(enable_offset);
        plic_write32(enable_offset, current | (1 << (UART_IRQ % 32)));

        // Step 4: 设置阈值为 0
        // 阈值寄存器: base + 0x200000 + context*0x1000
        let threshold_offset = 0x0020_0000 + PLIC_S_CONTEXT_HART0 * 0x1000;
        plic_write32(threshold_offset, 0);
    }
}

/// 从 PLIC claim 寄存器读取待处理中断号
///
/// # Safety
/// 调用前必须已映射 PLIC。
#[inline]
unsafe fn plic_claim(context: usize) -> u32 {
    let claim_offset = 0x0020_0000 + context * 0x1000 + 4;
    // SAFETY: 调用方保证已映射
    unsafe { plic_read32(claim_offset) }
}

/// 向 PLIC complete 寄存器写入中断号，完成处理
///
/// # Safety
/// 调用前必须已映射 PLIC。
#[inline]
unsafe fn plic_complete(context: usize, irq: u32) {
    let complete_offset = 0x0020_0000 + context * 0x1000 + 4;
    // SAFETY: 调用方保证已映射
    unsafe { plic_write32(complete_offset, irq) };
}

// ─────────────────────────────────────────────────────────────────────────────
// 外部中断处理
// ─────────────────────────────────────────────────────────────────────────────

/// 处理外部中断（PLIC IRQ）
fn handle_external() {
    // SAFETY: plic_init 已在 interrupt_init 中调用，PLIC 已映射
    let irq = unsafe { plic_claim(PLIC_S_CONTEXT_HART0) };
    match irq {
        0 => {
            // spurious interrupt, 忽略
        }
        n if n == UART_IRQ => {
            log::info!("UART IRQ");
        }
        n => {
            log::warn!("handle_external: 未知 IRQ {}", n);
        }
    }
    if irq != 0 {
        // SAFETY: PLIC 已映射
        unsafe { plic_complete(PLIC_S_CONTEXT_HART0, irq) };
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 中断初始化
// ─────────────────────────────────────────────────────────────────────────────

/// 初始化主核中断系统
///
/// 1. 设置 stvec 为 trap_entry（Direct 模式，mode=0）
/// 2. 使能 sie 中的 STIE（bit5）、SEIE（bit9）、SSIE（bit1）
/// 3. 设置 sstatus.SIE（bit1）使能全局中断
/// 4. 初始化 PLIC
pub fn interrupt_init() {
    // SAFETY: stvec/sscratch/sie/sstatus 是 S 模式 CSR，在 S 模式下可安全写入
    unsafe {
        // sscratch = 0 表示当前处于内核态。
        // trap_entry 通过 csrrw sp, sscratch, sp 交换后检查 sp 是否为 0
        // 来区分内核态/用户态。若 sscratch 非 0，内核态中断会被误判为用户态，
        // 导致 sp 被替换为垃圾值、上下文全部写坏。
        core::arch::asm!("csrw sscratch, zero");

        // 设置 stvec（Direct 模式：低 2 位 = 00）
        let trap_entry_addr = trap_entry as unsafe extern "C" fn() as usize;
        core::arch::asm!(
            "csrw stvec, {addr}",
            addr = in(reg) trap_entry_addr,
        );

        // 使能 sie: SSIE(1) | STIE(5) | SEIE(9) → mask = 0x222
        core::arch::asm!(
            "csrs sie, {mask}",
            mask = in(reg) 0x222usize,
        );

        // 使能全局中断 sstatus.SIE（bit1）
        core::arch::asm!("csrs sstatus, {mask}", mask = in(reg) 0x2usize);
    }

    plic_init();
    log::info!("InterruptInit done");
}

/// 初始化从核中断系统
///
/// 设置 stvec，使能 sie，设置 sstatus.SIE。
/// PLIC 由主核统一初始化，从核只需配置本核的 CSR。
pub fn interrupt_init_smp() {
    // SAFETY: CSR 写入在 S 模式下安全
    unsafe {
        // sscratch = 0 标记内核态（与主核相同）
        core::arch::asm!("csrw sscratch, zero");

        let trap_entry_addr = trap_entry as unsafe extern "C" fn() as usize;
        core::arch::asm!(
            "csrw stvec, {addr}",
            addr = in(reg) trap_entry_addr,
        );
        core::arch::asm!(
            "csrs sie, {mask}",
            mask = in(reg) 0x222usize,
        );
        core::arch::asm!("csrs sstatus, {mask}", mask = in(reg) 0x2usize);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 陷阱分发入口（由 interrupt.S 调用）
// ─────────────────────────────────────────────────────────────────────────────

/// 陷阱处理入口 — 由汇编 interrupt.S 中的 trap_entry 调用
///
/// **必须返回 `*mut TrapContext`**：汇编 trap_return 使用返回值（a0）
/// 恢复 sp，若返回 void 则 a0 为垃圾值导致 RestoreTrapContext 从错误地址加载。
///
/// # Safety
/// 调用方（汇编）保证：
/// - `ctx` 指向栈上已正确保存的 TrapContext
/// - 当前处于 S 模式，中断已被 CPU 自动关闭（sstatus.SIE=0）
#[unsafe(no_mangle)]
pub extern "C" fn HandleTrap(ctx: &mut TrapContext) -> *mut TrapContext {
    let scause = ctx.scause;
    // scause 最高位为 1 表示中断，为 0 表示异常
    let is_interrupt = (scause >> 63) != 0;
    let code = scause & !(1u64 << 63);

    if is_interrupt {
        match code {
            // 定时器中断（STIP，code 5）
            5 => super::timer::handle_timer(),
            // 外部中断（SEIP，code 9）
            9 => handle_external(),
            // 软件中断 / IPI（SSIP，code 1）
            1 => super::ipi::handle_ipi(ctx),
            _ => {
                log::warn!(
                    "HandleTrap: 未知中断 code={}, sepc=0x{:x}, scause=0x{:x}",
                    code,
                    ctx.sepc,
                    ctx.scause
                );
            }
        }
    } else {
        match code {
            // U-mode ecall (code 8) 或 S-mode ecall (code 9)
            8 | 9 => super::syscall::handle_syscall(ctx),
            _ => {
                log::error!(
                    "HandleTrap: 异常 code={}, sepc=0x{:x}, stval=0x{:x}, scause=0x{:x}",
                    code,
                    ctx.sepc,
                    ctx.stval,
                    ctx.scause,
                );
                crate::halt::halt("致命异常，内核停止");
            }
        }
    }

    ctx as *mut TrapContext
}
