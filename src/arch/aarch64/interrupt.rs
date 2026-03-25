/// AArch64 中断子系统
///
/// 负责 GICv2 初始化、VBAR_EL1 设置，以及 16 个异常向量处理函数的实现。
use crate::memory::address::PhysAddr;
use crate::memory::map_mmio;

use super::context::TrapContext;

// vector_table 由 interrupt.S 提供（.balign 0x800 对齐）
// SAFETY: 链接器保证该符号存在
unsafe extern "C" {
    static vector_table: u8;
}

// ─────────────────────────────────────────────────────────────────────────────
// GICv2 地址常量与辅助函数
// ─────────────────────────────────────────────────────────────────────────────

/// GIC 分发器（GICD）基地址（QEMU virt 平台）
const GICD_BASE: usize = 0x0800_0000;
/// GIC 分发器映射大小
const GICD_SIZE: usize = 0x1_0000;

/// GIC CPU 接口（GICC）基地址（QEMU virt 平台）
const GICC_BASE: usize = 0x0801_0000;
/// GIC CPU 接口映射大小
const GICC_SIZE: usize = 0x1_0000;

// GICD 寄存器偏移
const GICD_CTLR: usize = 0x000;
const GICD_ISENABLER: usize = 0x100;
const GICD_IPRIORITYR: usize = 0x400;

// GICC 寄存器偏移
const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;

/// 虚拟定时器 PPI 中断号（GIC IRQ 27）
const VTIMER_IRQ: u32 = 27;

/// 写 GICD 32 位寄存器
///
/// # Safety
/// 调用前必须已通过 `map_mmio` 映射 GICD 区域。
#[inline]
unsafe fn gicd_write32(offset: usize, val: u32) {
    let addr = (GICD_BASE + offset) as *mut u32;
    // SAFETY: 调用方保证地址已映射且对齐
    unsafe { core::ptr::write_volatile(addr, val) };
}

/// 写 GICC 32 位寄存器
///
/// # Safety
/// 调用前必须已通过 `map_mmio` 映射 GICC 区域。
#[inline]
unsafe fn gicc_write32(offset: usize, val: u32) {
    let addr = (GICC_BASE + offset) as *mut u32;
    // SAFETY: 调用方保证地址已映射且对齐
    unsafe { core::ptr::write_volatile(addr, val) };
}

/// 读 GICC 32 位寄存器
///
/// # Safety
/// 调用前必须已通过 `map_mmio` 映射 GICC 区域。
#[inline]
unsafe fn gicc_read32(offset: usize) -> u32 {
    let addr = (GICC_BASE + offset) as *const u32;
    // SAFETY: 调用方保证地址已映射且对齐
    unsafe { core::ptr::read_volatile(addr) }
}

// ─────────────────────────────────────────────────────────────────────────────
// GICv2 初始化
// ─────────────────────────────────────────────────────────────────────────────

/// 初始化 GICv2
///
/// 1. Identity-map GICD 和 GICC 寄存器区域
/// 2. 使能 GICD（GICD_CTLR = 1）
/// 3. 设置虚拟定时器（IRQ 27）优先级为 0xA0
/// 4. 使能 IRQ 27（GICD_ISENABLER 对应位）
/// 5. 设置 GICC_PMR = 0xFF（接受所有优先级）
/// 6. 使能 GICC（GICC_CTLR = 1）
fn gic_init() {
    // Step 1: 映射 GICD 和 GICC
    map_mmio(PhysAddr::new(GICD_BASE), GICD_SIZE).expect("gic_init: 映射 GICD 失败");
    map_mmio(PhysAddr::new(GICC_BASE), GICC_SIZE).expect("gic_init: 映射 GICC 失败");

    // SAFETY: GICD/GICC 已通过 map_mmio 映射
    unsafe {
        // Step 2: 使能 GICD
        gicd_write32(GICD_CTLR, 1);

        // Step 3: 设置 IRQ 27 优先级
        // GICD_IPRIORITYR 每个寄存器包含 4 个 IRQ 的优先级（每 8 位一个）
        // IRQ 27 → 寄存器 GICD_IPRIORITYR[27/4] = GICD_IPRIORITYR[6]，偏移量 = 0x400 + 6*4 = 0x418
        // byte offset in the register: 27 % 4 = 3 → 第 3 个字节（bits 31:24）
        let ipriorityr_reg_offset = GICD_IPRIORITYR + (VTIMER_IRQ as usize / 4) * 4;
        let byte_offset = (VTIMER_IRQ % 4) * 8;
        // 读-修改-写：仅设置 IRQ 27 对应字节，保持其他 IRQ 优先级不变
        let current = {
            let addr = (GICD_BASE + ipriorityr_reg_offset) as *const u32;
            core::ptr::read_volatile(addr)
        };
        let mask = !(0xFFu32 << byte_offset);
        let new_val = (current & mask) | (0xA0u32 << byte_offset);
        {
            let addr = (GICD_BASE + ipriorityr_reg_offset) as *mut u32;
            core::ptr::write_volatile(addr, new_val);
        }

        // Step 4: 使能 IRQ 27
        // GICD_ISENABLER[27/32] = GICD_ISENABLER[0]，位 27%32 = 27
        let isenabler_offset = GICD_ISENABLER + (VTIMER_IRQ as usize / 32) * 4;
        gicd_write32(isenabler_offset, 1u32 << (VTIMER_IRQ % 32));

        // Step 5: 设置 GICC_PMR = 0xFF（接受所有优先级）
        gicc_write32(GICC_PMR, 0xFF);

        // Step 6: 使能 GICC
        gicc_write32(GICC_CTLR, 1);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 中断初始化
// ─────────────────────────────────────────────────────────────────────────────

/// 初始化主核中断系统
///
/// 1. 设置 VBAR_EL1 为向量表地址
/// 2. 初始化 GICv2
/// 3. 通过 DAIFCLR 使能 IRQ（bit 1）
pub fn interrupt_init() {
    // SAFETY: vector_table 由链接器定义，.balign 0x800 对齐，在内核生命周期内有效
    let vbar = unsafe { &vector_table as *const u8 as u64 };
    // SAFETY: msr vbar_el1 在 EL1 下合法；daifclr 使能 IRQ
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {vbar}",
            "isb",
            vbar = in(reg) vbar,
        );
    }

    gic_init();

    // 使能 IRQ（清除 DAIF.I 位，bit 1 of daifclr）
    // SAFETY: msr daifclr 是特权指令，在 EL1 下合法
    unsafe {
        core::arch::asm!("msr daifclr, #2");
    }

    log::info!("InterruptInit done");
}

/// 初始化从核中断系统
///
/// 设置 VBAR_EL1，使能 IRQ。GICv2 GICD 由主核统一初始化。
pub fn interrupt_init_smp() {
    // SAFETY: vector_table 由链接器定义，在内核生命周期内有效
    let vbar = unsafe { &vector_table as *const u8 as u64 };
    // SAFETY: msr vbar_el1 在 EL1 下合法
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {vbar}",
            "isb",
            vbar = in(reg) vbar,
        );
        core::arch::asm!("msr daifclr, #2");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 中央分发函数
// ─────────────────────────────────────────────────────────────────────────────

/// 分发 IRQ（读取 GICC_IAR，分发，写 GICC_EOIR）
fn dispatch_irq(ctx: &mut TrapContext) {
    // SAFETY: GICC 已在 gic_init 中映射
    let iar = unsafe { gicc_read32(GICC_IAR) };
    let irq_id = iar & 0x3FF; // 低 10 位为中断 ID

    match irq_id {
        n if n == VTIMER_IRQ => {
            super::timer::handle_timer(ctx);
        }
        1023 => {
            // spurious interrupt（GICv2 1023 = no interrupt pending），忽略
        }
        n => {
            log::warn!("dispatch_irq: 未知 IRQ {}", n);
        }
    }

    // 写 EOIR 完成中断处理
    if irq_id != 1023 {
        // SAFETY: GICC 已映射
        unsafe { gicc_write32(GICC_EOIR, iar) };
    }
}

/// 分发同步异常（读取 ctx.esr_el1，根据 EC 字段路由）
fn dispatch_sync(ctx: &mut TrapContext) {
    let esr = ctx.esr_el1;
    let ec = (esr >> 26) & 0x3F; // ESR_EL1.EC 字段

    match ec {
        // SVC 指令（EC = 0x15 = 21）
        0x15 => super::syscall::handle_syscall(ctx),
        // 数据中止（EC = 0x24/0x25）或指令中止（EC = 0x20/0x21）
        0x20 | 0x21 | 0x24 | 0x25 => {
            log::error!(
                "dispatch_sync: 地址中止 EC=0x{:02x}, ESR=0x{:x}, ELR=0x{:x}",
                ec,
                esr,
                ctx.elr_el1
            );
            crate::halt::halt("致命异常，内核停止");
        }
        _ => {
            log::error!(
                "dispatch_sync: 未知同步异常 EC=0x{:02x}, ESR=0x{:x}, ELR=0x{:x}",
                ec,
                esr,
                ctx.elr_el1
            );
            crate::halt::halt("致命异常，内核停止");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 16 个异常向量处理函数（由 interrupt.S 调用）
// ─────────────────────────────────────────────────────────────────────────────

/// Current EL with SP0 — 同步异常
#[unsafe(no_mangle)]
pub extern "C" fn sync_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::warn!("sync_current_el_sp0: ESR=0x{:x}", ctx.esr_el1);
    dispatch_sync(ctx);
}

/// Current EL with SP0 — IRQ
#[unsafe(no_mangle)]
pub extern "C" fn irq_current_el_sp0_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

/// Current EL with SP0 — FIQ
#[unsafe(no_mangle)]
pub extern "C" fn fiq_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_current_el_sp0: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Current EL with SP0 — 系统错误
#[unsafe(no_mangle)]
pub extern "C" fn error_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_current_el_sp0: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Current EL with SPx — 同步异常
#[unsafe(no_mangle)]
pub extern "C" fn sync_current_el_spx_handler(ctx: &mut TrapContext) {
    dispatch_sync(ctx);
}

/// Current EL with SPx — IRQ
#[unsafe(no_mangle)]
pub extern "C" fn irq_current_el_spx_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

/// Current EL with SPx — FIQ
#[unsafe(no_mangle)]
pub extern "C" fn fiq_current_el_spx_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_current_el_spx: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Current EL with SPx — 系统错误
#[unsafe(no_mangle)]
pub extern "C" fn error_current_el_spx_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_current_el_spx: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Lower EL AArch64 — 同步异常
#[unsafe(no_mangle)]
pub extern "C" fn sync_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    dispatch_sync(ctx);
}

/// Lower EL AArch64 — IRQ
#[unsafe(no_mangle)]
pub extern "C" fn irq_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

/// Lower EL AArch64 — FIQ
#[unsafe(no_mangle)]
pub extern "C" fn fiq_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_lower_el_aarch64: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Lower EL AArch64 — 系统错误
#[unsafe(no_mangle)]
pub extern "C" fn error_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_lower_el_aarch64: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Lower EL AArch32 — 同步异常
#[unsafe(no_mangle)]
pub extern "C" fn sync_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "sync_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Lower EL AArch32 — IRQ
#[unsafe(no_mangle)]
pub extern "C" fn irq_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

/// Lower EL AArch32 — FIQ
#[unsafe(no_mangle)]
pub extern "C" fn fiq_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

/// Lower EL AArch32 — 系统错误
#[unsafe(no_mangle)]
pub extern "C" fn error_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}
