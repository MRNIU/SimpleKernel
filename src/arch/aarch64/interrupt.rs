/// AArch64 中断子系统
///
/// 使用 `arm-gic` crate 初始化 GICv3，通过 `GicCpuInterface` 系统寄存器
/// 完成 IAR/EOIR 操作。VBAR_EL1 设置及 16 个异常向量处理函数。
use arm_gic::gicv3::registers::{Gicd, GicrSgi};
use arm_gic::gicv3::{GicCpuInterface, GicV3};
use arm_gic::{IntId, InterruptGroup, UniqueMmioPointer};
use core::ptr::NonNull;

use crate::memory::address::PhysAddr;
use crate::memory::map_mmio;

use super::context::TrapContext;

// vector_table 由 interrupt.S 提供（.balign 0x800 对齐）
// SAFETY: 链接器保证该符号存在
unsafe extern "C" {
    static vector_table: u8;
}

// ─────────────────────────────────────────────────────────────────────────────
// GICv3 地址常量（QEMU virt 平台）
// ─────────────────────────────────────────────────────────────────────────────

/// GIC Distributor（GICD）基地址
const GICD_BASE: usize = 0x0800_0000;
/// GICD 映射大小
const GICD_SIZE: usize = 0x1_0000;

/// GIC Redistributor（GICR）基地址
const GICR_BASE: usize = 0x080A_0000;
/// 每核 GICR 帧大小（RD_base 64KB + SGI_base 64KB = 0x20000）
const GICR_STRIDE: usize = 0x2_0000;
/// GICR 映射总大小（MAX_CORE_COUNT 个核）
const GICR_SIZE: usize = GICR_STRIDE * crate::config::MAX_CORE_COUNT;

/// 虚拟定时器 PPI 编号（PPI 11 = GIC IRQ 27）
const VTIMER_PPI: u32 = 11;

/// 定时器中断优先级
const VTIMER_PRIORITY: u8 = 0xA0;

// ─────────────────────────────────────────────────────────────────────────────
// GICv3 初始化辅助
// ─────────────────────────────────────────────────────────────────────────────

/// 创建临时 GicV3 实例
///
/// GIC 硬件状态在 setup/init_cpu 后持续有效；调用方使用后可安全 drop。
/// IAR/EOIR 通过 `GicCpuInterface` 静态方法访问，不依赖此实例。
///
/// # Safety
/// GICD 和 GICR 区域必须已通过 `map_mmio` 映射。
unsafe fn create_gic<'a>() -> GicV3<'a> {
    let cpu_count = crate::per_cpu::BASIC_INFO
        .get()
        .map(|info| info.core_count)
        .unwrap_or(1);

    // SAFETY: GICD/GICR 已映射，identity mapping 保证地址有效
    unsafe {
        let gicd_ptr: *mut Gicd = GICD_BASE as *mut Gicd;
        let gicd = UniqueMmioPointer::new(NonNull::new(gicd_ptr).expect("GICD_BASE is null"));
        let gicr = NonNull::new(GICR_BASE as *mut GicrSgi).expect("GICR_BASE is null");
        GicV3::new(gicd, gicr, cpu_count, false)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 中断初始化
// ─────────────────────────────────────────────────────────────────────────────

/// 初始化主核中断系统
///
/// 1. 设置 VBAR_EL1 为向量表地址
/// 2. 映射 GICD/GICR 并通过 arm-gic 初始化 GICv3
/// 3. 使能虚拟定时器 PPI（IRQ 27）
/// 4. 通过 DAIFCLR 使能 IRQ
pub fn interrupt_init() {
    // SAFETY: vector_table 由链接器定义，.balign 0x800 对齐
    let vbar = unsafe { &vector_table as *const u8 as u64 };
    // SAFETY: msr vbar_el1 在 EL1 下合法
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {vbar}",
            "isb",
            vbar = in(reg) vbar,
        );
    }

    // 映射 GICD 和 GICR MMIO 区域
    map_mmio(PhysAddr::new(GICD_BASE), GICD_SIZE).expect("interrupt_init: 映射 GICD 失败");
    map_mmio(PhysAddr::new(GICR_BASE), GICR_SIZE).expect("interrupt_init: 映射 GICR 失败");

    let cpu_id = crate::per_cpu::current_core_id();

    // SAFETY: GICD/GICR 已映射
    unsafe {
        let mut gic = create_gic();
        // setup() 完成：ICC_SRE 使能、Redistributor 唤醒、GICD 配置、Group 1 使能
        gic.setup(cpu_id);

        // 使能虚拟定时器 PPI
        let vtimer_intid = IntId::ppi(VTIMER_PPI);
        gic.set_interrupt_priority(vtimer_intid, Some(cpu_id), VTIMER_PRIORITY)
            .expect("set_interrupt_priority 失败");
        gic.enable_interrupt(vtimer_intid, Some(cpu_id), true)
            .expect("enable_interrupt 失败");
    }

    // 设置优先级掩码（接受所有优先级）
    GicCpuInterface::set_priority_mask(0xFF);

    // 使能 IRQ（仅清除 DAIF.I 位）
    // SAFETY: msr daifclr 是特权指令，在 EL1 下合法
    unsafe {
        core::arch::asm!("msr daifclr, #2");
    }

    log::info!("InterruptInit: GICv3 (arm-gic) done");
}

/// 初始化从核中断系统
///
/// GICD 和 GICR 区域已由主核映射。从核仅需：
/// 1. 设置 VBAR_EL1
/// 2. 初始化本核 GIC CPU 接口和 Redistributor
/// 3. 使能虚拟定时器 PPI
/// 4. 使能 IRQ
pub fn interrupt_init_smp() {
    // SAFETY: vector_table 由链接器定义
    let vbar = unsafe { &vector_table as *const u8 as u64 };
    // SAFETY: msr vbar_el1 在 EL1 下合法
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {vbar}",
            "isb",
            vbar = in(reg) vbar,
        );
    }

    let cpu_id = crate::per_cpu::current_core_id();

    // SAFETY: GICD/GICR 已由主核映射
    unsafe {
        let mut gic = create_gic();
        // init_cpu：ICC_SRE 使能 + Redistributor 唤醒
        gic.init_cpu(cpu_id);

        // 使能本核虚拟定时器 PPI
        let vtimer_intid = IntId::ppi(VTIMER_PPI);
        gic.set_interrupt_priority(vtimer_intid, Some(cpu_id), VTIMER_PRIORITY)
            .expect("set_interrupt_priority 失败");
        gic.enable_interrupt(vtimer_intid, Some(cpu_id), true)
            .expect("enable_interrupt 失败");
    }

    GicCpuInterface::set_priority_mask(0xFF);
    GicCpuInterface::enable_group1(true);

    // SAFETY: msr daifclr 在 EL1 下合法
    unsafe {
        core::arch::asm!("msr daifclr, #2");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 中央分发函数
// ─────────────────────────────────────────────────────────────────────────────

/// 分发 IRQ（通过 GicCpuInterface 系统寄存器完成 IAR/EOIR）
fn dispatch_irq(ctx: &mut TrapContext) {
    let intid = GicCpuInterface::get_and_acknowledge_interrupt(InterruptGroup::Group1);

    let Some(intid) = intid else {
        // Spurious interrupt（IntId::SPECIAL_NONE = 1023），忽略
        return;
    };

    match intid {
        id if id == IntId::ppi(VTIMER_PPI) => {
            super::timer::handle_timer(ctx);
        }
        id => {
            log::warn!("dispatch_irq: 未知 IRQ {:?}", id);
        }
    }

    GicCpuInterface::end_interrupt(intid, InterruptGroup::Group1);
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

// ── Current EL with SP0 ──────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn sync_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::warn!("sync_current_el_sp0: ESR=0x{:x}", ctx.esr_el1);
    dispatch_sync(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn irq_current_el_sp0_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn fiq_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_current_el_sp0: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

#[unsafe(no_mangle)]
pub extern "C" fn error_current_el_sp0_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_current_el_sp0: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

// ── Current EL with SPx ──────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn sync_current_el_spx_handler(ctx: &mut TrapContext) {
    dispatch_sync(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn irq_current_el_spx_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn fiq_current_el_spx_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_current_el_spx: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

#[unsafe(no_mangle)]
pub extern "C" fn error_current_el_spx_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_current_el_spx: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

// ── Lower EL AArch64 ────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn sync_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    dispatch_sync(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn irq_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn fiq_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_lower_el_aarch64: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

#[unsafe(no_mangle)]
pub extern "C" fn error_lower_el_aarch64_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_lower_el_aarch64: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

// ── Lower EL AArch32 ────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn sync_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "sync_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

#[unsafe(no_mangle)]
pub extern "C" fn irq_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    dispatch_irq(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn fiq_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "fiq_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}

#[unsafe(no_mangle)]
pub extern "C" fn error_lower_el_aarch32_handler(ctx: &mut TrapContext) {
    log::error!(
        "error_lower_el_aarch32: ESR=0x{:x}, ELR=0x{:x}",
        ctx.esr_el1,
        ctx.elr_el1
    );
    crate::halt::halt("致命异常，内核停止");
}
