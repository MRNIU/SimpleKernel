// Copyright The SimpleKernel Contributors

/// AArch64 陷阱上下文与被调用者保存寄存器上下文
///
/// 布局与 interrupt.S / switch.rs 中的汇编严格对应，修改时需同步更新汇编代码。
use core::mem::offset_of;

/// 陷阱上下文 — 保存发生异常/中断时所有通用寄存器及必要系统寄存器
///
/// 大小：896 字节，与 interrupt.S 中 `kTrapContextSize` 一致。
///
/// 内存布局：
/// - 偏移   0–240: x0–x30 (31 个通用寄存器，每个 8 字节)
/// - 偏移 248: _padding0（对齐填充）
/// - 偏移 256–767: q0–q31（FP/SIMD 128 位寄存器）
/// - 偏移 768: fpsr（浮点状态寄存器）
/// - 偏移 776: fpcr（浮点控制寄存器）
/// - 偏移 784–831: _padding1（对齐填充）
/// - 偏移 832: elr_el1（异常链接寄存器）
/// - 偏移 840: spsr_el1（保存的程序状态寄存器）
/// - 偏移 848: esr_el1（异常综合寄存器）
/// - 偏移 856: sp_el0（用户栈指针）
/// - 偏移 864: tpidr_el0（用户线程指针）
/// - 偏移 872: ttbr0_el1（用户页表基地址）
/// - 偏移 880: sp_el1（内核栈指针，陷入前）
/// - 偏移 888: tpidr_el1（内核线程指针）
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct TrapContext {
    /// x0–x30 通用寄存器
    pub x: [u64; 31],
    /// 填充，使 elr_el1 对齐到偏移 256
    pub _padding0: u64,
    /// q0–q31 FP/SIMD 128 位寄存器
    pub q: [u128; 32],
    /// FPSR — 浮点状态寄存器
    pub fpsr: u64,
    /// FPCR — 浮点控制寄存器
    pub fpcr: u64,
    /// 填充，使系统寄存器区域对齐到偏移 832
    pub _padding1: [u64; 6],
    /// ELR_EL1 — 异常返回地址
    pub elr_el1: u64,
    /// SPSR_EL1 — 保存的处理器状态
    pub spsr_el1: u64,
    /// ESR_EL1 — 异常综合寄存器
    pub esr_el1: u64,
    /// SP_EL0 — 用户栈指针
    pub sp_el0: u64,
    /// TPIDR_EL0 — 用户线程本地存储指针
    pub tpidr_el0: u64,
    /// TTBR0_EL1 — 用户页表基地址寄存器
    pub ttbr0_el1: u64,
    /// SP_EL1 — 陷入前的内核栈指针
    pub sp_el1: u64,
    /// TPIDR_EL1 — 内核线程本地存储指针
    pub tpidr_el1: u64,
}

// 编译时大小和偏移断言
const _: () = {
    assert!(
        core::mem::size_of::<TrapContext>() == 896,
        "TrapContext 大小必须为 896 字节"
    );
    assert!(offset_of!(TrapContext, x) == 0, "x 数组应从偏移 0 开始");
    assert!(
        offset_of!(TrapContext, _padding0) == 248,
        "_padding0 应在偏移 248"
    );
    assert!(offset_of!(TrapContext, q) == 256, "q 数组应在偏移 256");
    assert!(offset_of!(TrapContext, fpsr) == 768, "fpsr 应在偏移 768");
    assert!(offset_of!(TrapContext, fpcr) == 776, "fpcr 应在偏移 776");
    assert!(
        offset_of!(TrapContext, _padding1) == 784,
        "_padding1 应在偏移 784"
    );
    assert!(
        offset_of!(TrapContext, elr_el1) == 832,
        "elr_el1 应在偏移 832"
    );
    assert!(
        offset_of!(TrapContext, spsr_el1) == 840,
        "spsr_el1 应在偏移 840"
    );
    assert!(
        offset_of!(TrapContext, esr_el1) == 848,
        "esr_el1 应在偏移 848"
    );
    assert!(
        offset_of!(TrapContext, sp_el0) == 856,
        "sp_el0 应在偏移 856"
    );
    assert!(
        offset_of!(TrapContext, tpidr_el0) == 864,
        "tpidr_el0 应在偏移 864"
    );
    assert!(
        offset_of!(TrapContext, ttbr0_el1) == 872,
        "ttbr0_el1 应在偏移 872"
    );
    assert!(
        offset_of!(TrapContext, sp_el1) == 880,
        "sp_el1 应在偏移 880"
    );
    assert!(
        offset_of!(TrapContext, tpidr_el1) == 888,
        "tpidr_el1 应在偏移 888"
    );
};

/// 被调用者保存上下文 — 任务切换时保存/恢复
///
/// 大小：176 字节，与 switch.rs 中保存/恢复偏移一致。
///
/// 布局：
/// - 偏移   0–95: x19–x30（12 个寄存器）
/// - 偏移  96–159: d8–d15（AArch64 ABI 的 callee-saved FP 寄存器低 64 位）
/// - 偏移 160: sp（当前栈指针）
/// - 偏移 168: pc（恢复后跳转地址，对应 x30/lr）
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CalleeSavedContext {
    /// x19–x30 被调用者保存寄存器（12 个）
    pub regs: [u64; 12],
    /// d8–d15 被调用者保存 FP 寄存器
    pub fp_regs: [u64; 8],
    /// 栈指针
    pub sp: u64,
    /// 恢复后跳转地址（link register）
    pub pc: u64,
}

const _: () = {
    assert!(
        core::mem::size_of::<CalleeSavedContext>() == 176,
        "CalleeSavedContext 大小必须为 176 字节"
    );
    assert!(offset_of!(CalleeSavedContext, regs) == 0);
    assert!(offset_of!(CalleeSavedContext, fp_regs) == 96);
    assert!(offset_of!(CalleeSavedContext, sp) == 160);
    assert!(offset_of!(CalleeSavedContext, pc) == 168);
};

impl CalleeSavedContext {
    /// 初始化内核线程上下文，使 `switch_to` 后跳转到 `kernel_thread_entry`。
    ///
    /// - `pc` → `kernel_thread_entry`（汇编入口）
    /// - `sp` → 内核栈顶
    /// - `x19`（`regs[0]`）→ 入口函数指针
    /// - `x20`（`regs[1]`）→ 入口函数参数
    pub fn init_for_kernel_thread(&mut self, kstack_top: usize, entry: fn(usize), arg: usize) {
        self.pc = super::switch::kernel_thread_entry as unsafe extern "C" fn() as usize as u64;
        self.sp = kstack_top as u64;
        self.regs[0] = entry as usize as u64;
        self.regs[1] = arg as u64;
    }
}
