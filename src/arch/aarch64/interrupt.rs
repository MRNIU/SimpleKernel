// Copyright The SimpleKernel Contributors

/// AArch64 中断子系统
///
/// 使用 `arm-gic` crate 初始化 GICv3，通过 `GicCpuInterface` 系统寄存器
/// 完成 IAR/EOIR 操作。VBAR_EL1 设置及 16 个异常向量处理函数。
use arm_gic::gicv3::registers::{Gicd, GicrSgi};
use arm_gic::gicv3::{GicCpuInterface, GicV3, Group};
use arm_gic::{IntId, InterruptGroup, UniqueMmioPointer};
use core::arch::global_asm;
use core::ptr::NonNull;

use arch_primitives::FDT_INTERRUPT_CONTROLLER_COMPATIBLES;
use memory::MmioRegion;
use memory_types::PhysAddr;

use super::context::TrapContext;

// 异常向量表 + Trap 入口/返回汇编（含宏定义），由 LLVM 内置汇编器处理
global_asm!(include_str!("interrupt.S"));

// vector_table 由上述 global_asm! 定义（.balign 0x800 对齐）
// SAFETY: global_asm! 保证该符号存在
unsafe extern "C" {
    static vector_table: u8;
}

#[cfg(feature = "test-support")]
mod expected_fault {
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::TrapContext;

    const NO_CORE: usize = usize::MAX;
    const EC_DATA_ABORT_CURRENT_EL: u64 = 0x25;
    const ESR_EC_SHIFT: u64 = 26;
    const ESR_EC_MASK: u64 = 0x3F;
    const ESR_ISS_WNR: u64 = 1 << 6;

    static ARMED: AtomicBool = AtomicBool::new(false);
    static OBSERVED: AtomicBool = AtomicBool::new(false);
    static TARGET_CORE_ID: AtomicUsize = AtomicUsize::new(NO_CORE);
    static FAULT_ADDR: AtomicUsize = AtomicUsize::new(0);
    static FAULT_PC: AtomicUsize = AtomicUsize::new(0);
    static RESUME_PC: AtomicUsize = AtomicUsize::new(0);

    pub fn expect_write_data_abort(
        target_core_id: usize,
        fault_addr: usize,
        fault_pc: usize,
        resume_pc: usize,
    ) {
        TARGET_CORE_ID.store(target_core_id, Ordering::Relaxed);
        FAULT_ADDR.store(fault_addr, Ordering::Relaxed);
        FAULT_PC.store(fault_pc, Ordering::Relaxed);
        RESUME_PC.store(resume_pc, Ordering::Relaxed);
        OBSERVED.store(false, Ordering::Relaxed);
        ARMED.store(true, Ordering::Release);
    }

    pub fn observed() -> bool {
        OBSERVED.load(Ordering::Acquire)
    }

    pub fn try_handle(ctx: &mut TrapContext) -> bool {
        let ec = (ctx.esr_el1 >> ESR_EC_SHIFT) & ESR_EC_MASK;
        if ec != EC_DATA_ABORT_CURRENT_EL
            || ctx.esr_el1 & ESR_ISS_WNR == 0
            || !ARMED.load(Ordering::Acquire)
        {
            return false;
        }

        let core_id = per_cpu::current_core_id();
        if core_id != TARGET_CORE_ID.load(Ordering::Relaxed)
            || ctx.far_el1 as usize != FAULT_ADDR.load(Ordering::Relaxed)
            || ctx.elr_el1 as usize != FAULT_PC.load(Ordering::Relaxed)
        {
            return false;
        }

        ctx.elr_el1 = RESUME_PC.load(Ordering::Relaxed) as u64;
        OBSERVED.store(true, Ordering::Release);
        ARMED.store(false, Ordering::Release);
        true
    }
}

/// 注册一个测试专用的 AArch64 写 data abort 恢复点。
///
/// 仅用于独立系统测试验证 TLB shootdown 后远端核心访问语义。
#[cfg(feature = "test-support")]
pub fn expect_write_data_abort_for_test(
    target_core_id: usize,
    fault_addr: usize,
    fault_pc: usize,
    resume_pc: usize,
) {
    expected_fault::expect_write_data_abort(target_core_id, fault_addr, fault_pc, resume_pc);
}

/// 返回测试专用 AArch64 写 data abort 是否已经命中。
#[cfg(feature = "test-support")]
pub fn write_data_abort_observed_for_test() -> bool {
    expected_fault::observed()
}

/// GIC 地址信息（运行时初始化）
struct GicAddrs {
    gicd_base: usize,
    gicd_size: usize,
    gicr_base: usize,
    gicr_size: usize,
}

static GIC_ADDRS: spin::Once<GicAddrs> = spin::Once::new();

/// 虚拟定时器 PPI 编号（PPI 11 = GIC IRQ 27）
const VTIMER_PPI: u32 = 11;

/// 定时器中断优先级
const VTIMER_PRIORITY: u8 = 0xA0;

/// TLB shootdown 使用的 SGI 编号。
const TLB_SHOOTDOWN_SGI: u32 = 0;

/// TLB shootdown SGI 优先级。
const TLB_SHOOTDOWN_PRIORITY: u8 = 0x90;

/// 从 FDT 中读取 GICv3 的指定 `reg` 区域。
fn gic_reg_from_fdt(
    fdt: &crate::platform_fdt::PlatformFdt,
    reg_index: usize,
) -> Result<crate::platform_fdt::FdtReg, crate::platform_fdt::FdtError> {
    for compatible in FDT_INTERRUPT_CONTROLLER_COMPATIBLES {
        match fdt.compatible_reg(compatible, reg_index) {
            Err(crate::platform_fdt::FdtError::NodeNotFound) => {}
            result => return result,
        }
    }

    Err(crate::platform_fdt::FdtError::NodeNotFound)
}

/// 从 FDT 解析 GICv3 的 GICD 和 GICR 基地址。
///
/// GICv3 `reg` 属性包含两组区域：GICD (addr, size) + GICR (addr, size)。
fn init_gic_addrs() {
    GIC_ADDRS.call_once(|| {
        let fdt = crate::platform_fdt::get().expect("init_gic_addrs: FDT 未初始化");

        let gicd = gic_reg_from_fdt(fdt, 0).expect("init_gic_addrs: FDT 中未找到 GICv3 GICD reg");
        let gicr = gic_reg_from_fdt(fdt, 1).expect("init_gic_addrs: FDT 中未找到 GICv3 GICR reg");

        log::info!(
            "GIC: GICD={:#x}({}), GICR={:#x}({})",
            gicd.address,
            gicd.size,
            gicr.address,
            gicr.size
        );
        GicAddrs {
            gicd_base: usize::try_from(gicd.address).expect("init_gic_addrs: GICD 地址超出 usize"),
            gicd_size: gicd.size,
            gicr_base: usize::try_from(gicr.address).expect("init_gic_addrs: GICR 地址超出 usize"),
            gicr_size: gicr.size,
        }
    });
}

/// 创建临时 GicV3 实例
///
/// GIC 硬件状态在 setup/init_cpu 后持续有效；调用方使用后可安全 drop。
/// IAR/EOIR 通过 `GicCpuInterface` 静态方法访问，不依赖此实例。
///
/// # Safety
/// GICD 和 GICR 区域必须已通过 `MmioRegion::map` 映射。
unsafe fn create_gic<'a>() -> GicV3<'a> {
    let cpu_count = crate::CORE_COUNT.get().copied().unwrap_or(1);

    let addrs = GIC_ADDRS.get().expect("GIC_ADDRS 未初始化");

    // SAFETY: GICD/GICR 已映射，identity mapping 保证地址有效
    unsafe {
        let gicd_ptr: *mut Gicd = addrs.gicd_base as *mut Gicd;
        let gicd = UniqueMmioPointer::new(NonNull::new(gicd_ptr).expect("GICD 基地址为 null"));
        let gicr = NonNull::new(addrs.gicr_base as *mut GicrSgi).expect("GICR 基地址为 null");
        GicV3::new(gicd, gicr, cpu_count, false)
    }
}

/// 使能当前 CPU 接收 TLB shootdown SGI。
fn enable_tlb_shootdown_sgi(gic: &mut GicV3<'_>, cpu_id: usize) {
    let intid = IntId::sgi(TLB_SHOOTDOWN_SGI);
    gic.set_group(intid, Some(cpu_id), Group::Group1NS)
        .expect("GIC: 设置 TLB shootdown SGI 分组失败");
    gic.set_interrupt_priority(intid, Some(cpu_id), TLB_SHOOTDOWN_PRIORITY)
        .expect("GIC: 设置 TLB shootdown SGI 优先级失败");
    gic.enable_interrupt(intid, Some(cpu_id), true)
        .expect("GIC: 使能 TLB shootdown SGI 失败");
}

/// 初始化主核中断系统
///
/// 1. 设置 VBAR_EL1 为向量表地址
/// 2. 映射 GICD/GICR 并通过 arm-gic 初始化 GICv3
/// 3. 使能虚拟定时器 PPI（IRQ 27）
/// 4. 通过 DAIFCLR 使能 IRQ
pub fn init() {
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

    // 从 FDT 初始化 GIC 地址
    init_gic_addrs();
    let addrs = GIC_ADDRS.get().expect("GIC_ADDRS 未初始化");

    MmioRegion::map(PhysAddr::new(addrs.gicd_base), addrs.gicd_size);
    MmioRegion::map(PhysAddr::new(addrs.gicr_base), addrs.gicr_size);

    let cpu_id = per_cpu::current_core_id();

    // SAFETY: GICD/GICR 已映射
    unsafe {
        let mut gic = create_gic();
        // setup() 完成：ICC_SRE 使能、Redistributor 唤醒、GICD 配置、Group 1 使能
        gic.setup(cpu_id);

        // 使能虚拟定时器 PPI
        let vtimer_intid = IntId::ppi(VTIMER_PPI);
        gic.set_interrupt_priority(vtimer_intid, Some(cpu_id), VTIMER_PRIORITY)
            .expect("GIC: 设置定时器中断优先级失败");
        gic.enable_interrupt(vtimer_intid, Some(cpu_id), true)
            .expect("GIC: 使能定时器中断失败");
        enable_tlb_shootdown_sgi(&mut gic, cpu_id);
    }

    // 设置优先级掩码（接受所有优先级）
    GicCpuInterface::set_priority_mask(0xFF);

    // 使能 IRQ（仅清除 DAIF.I 位）
    // SAFETY: 向量表已设置，GIC 已初始化，此处为 bootstrap 路径
    unsafe { interrupt_state::bootstrap_enable() };

    log::info!("InterruptInit: GICv3 (arm-gic) done");
}

/// 初始化从核中断系统
///
/// GICD 和 GICR 区域已由主核映射。从核仅需：
/// 1. 设置 VBAR_EL1
/// 2. 初始化本核 GIC CPU 接口和 Redistributor
/// 3. 使能虚拟定时器 PPI
/// 4. 使能 IRQ
pub fn init_smp() {
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

    let cpu_id = per_cpu::current_core_id();

    // SAFETY: GICD/GICR 已由主核映射
    unsafe {
        let mut gic = create_gic();
        // init_cpu：ICC_SRE 使能 + Redistributor 唤醒
        gic.init_cpu(cpu_id);

        // 使能本核虚拟定时器 PPI
        let vtimer_intid = IntId::ppi(VTIMER_PPI);
        gic.set_interrupt_priority(vtimer_intid, Some(cpu_id), VTIMER_PRIORITY)
            .expect("GIC SMP: 设置定时器中断优先级失败");
        gic.enable_interrupt(vtimer_intid, Some(cpu_id), true)
            .expect("GIC SMP: 使能定时器中断失败");
        enable_tlb_shootdown_sgi(&mut gic, cpu_id);
    }

    GicCpuInterface::set_priority_mask(0xFF);
    GicCpuInterface::enable_group1(true);

    // SAFETY: 向量表已设置，GIC 已初始化，此处为 SMP bootstrap 路径
    unsafe { interrupt_state::bootstrap_enable() };
}

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
        id if id == IntId::sgi(TLB_SHOOTDOWN_SGI) => {
            crate::tlb_shootdown::handle_ipi();
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

    #[cfg(feature = "test-support")]
    if expected_fault::try_handle(ctx) {
        return;
    }

    match ec {
        // SAS 模式下 SVC 不应触发——所有 syscall 通过直接函数调用
        0x15 => {
            panic!(
                "SVC 在 SAS 模式下不应触发: ESR=0x{:x}, ELR=0x{:x}",
                esr, ctx.elr_el1
            );
        }
        // FP/SIMD access trap（EC = 0x07）：通常说明 CPACR_EL1.FPEN 未正确开启。
        0x07 => {
            log::error!(
                "dispatch_sync: FP/SIMD 访问异常，检查 CPACR_EL1.FPEN: ESR=0x{:x}, ELR=0x{:x}",
                esr,
                ctx.elr_el1
            );
            crate::util::halt::halt("FP/SIMD 未启用，内核停止");
        }
        // 数据中止（EC = 0x24/0x25）或指令中止（EC = 0x20/0x21）
        0x20 | 0x21 | 0x24 | 0x25 => {
            log::error!(
                "dispatch_sync: 地址中止 EC=0x{:02x}, ESR=0x{:x}, ELR=0x{:x}, FAR=0x{:x}",
                ec,
                esr,
                ctx.elr_el1,
                ctx.far_el1
            );
            crate::util::halt::halt("致命异常，内核停止");
        }
        _ => {
            log::error!(
                "dispatch_sync: 未知同步异常 EC=0x{:02x}, ESR=0x{:x}, ELR=0x{:x}, FAR=0x{:x}",
                ec,
                esr,
                ctx.elr_el1,
                ctx.far_el1
            );
            crate::util::halt::halt("致命异常，内核停止");
        }
    }
}

/// 生成 `#[unsafe(no_mangle)] pub extern "C" fn` 异常处理函数。
macro_rules! exception_handler {
    ($name:ident => sync) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(ctx: &mut TrapContext) {
            dispatch_sync(ctx);
        }
    };
    ($name:ident => irq) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(ctx: &mut TrapContext) {
            {
                let _irq = interrupt_state::HardIrqGuard::enter();
                dispatch_irq(ctx);
            }
            crate::task::preempt_after_irq();
        }
    };
    ($name:ident => fatal) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(ctx: &mut TrapContext) {
            log::error!(
                "{}: ESR=0x{:x}, ELR=0x{:x}, FAR=0x{:x}",
                stringify!($name),
                ctx.esr_el1,
                ctx.elr_el1,
                ctx.far_el1
            );
            crate::util::halt::halt("致命异常，内核停止");
        }
    };
}

exception_handler!(sync_current_el_sp0_handler      => sync);
exception_handler!(irq_current_el_sp0_handler       => irq);
exception_handler!(fiq_current_el_sp0_handler       => fatal);
exception_handler!(error_current_el_sp0_handler     => fatal);
exception_handler!(sync_current_el_spx_handler      => sync);
exception_handler!(irq_current_el_spx_handler       => irq);
exception_handler!(fiq_current_el_spx_handler       => fatal);
exception_handler!(error_current_el_spx_handler     => fatal);
exception_handler!(sync_lower_el_aarch64_handler    => sync);
exception_handler!(irq_lower_el_aarch64_handler     => irq);
exception_handler!(fiq_lower_el_aarch64_handler     => fatal);
exception_handler!(error_lower_el_aarch64_handler   => fatal);
exception_handler!(sync_lower_el_aarch32_handler    => fatal);
exception_handler!(irq_lower_el_aarch32_handler     => irq);
exception_handler!(fiq_lower_el_aarch32_handler     => fatal);
exception_handler!(error_lower_el_aarch32_handler   => fatal);
