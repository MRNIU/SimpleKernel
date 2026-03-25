/// AArch64 陷阱上下文与被调用者保存寄存器上下文
///
/// 布局与 macro.S 中的宏严格对应，修改时需同步更新汇编代码。
use core::mem::offset_of;

/// 陷阱上下文 — 保存发生异常/中断时所有通用寄存器及必要系统寄存器
///
/// 大小：320 字节，与 macro.S 中 `kTrapContextSize` 一致。
///
/// 内存布局：
/// - 偏移   0–240: x0–x30 (31 个通用寄存器，每个 8 字节)
/// - 偏移 248: _padding0（对齐填充）
/// - 偏移 256: elr_el1（异常链接寄存器）
/// - 偏移 264: spsr_el1（保存的程序状态寄存器）
/// - 偏移 272: esr_el1（异常综合寄存器）
/// - 偏移 280: sp_el0（用户栈指针）
/// - 偏移 288: tpidr_el0（用户线程指针）
/// - 偏移 296: ttbr0_el1（用户页表基地址）
/// - 偏移 304: sp_el1（内核栈指针，陷入前）
/// - 偏移 312: tpidr_el1（内核线程指针）
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct TrapContext {
    /// x0–x30 通用寄存器
    pub x: [u64; 31],
    /// 填充，使 elr_el1 对齐到偏移 256
    pub _padding0: u64,
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
        core::mem::size_of::<TrapContext>() == 320,
        "TrapContext 大小必须为 320 字节"
    );
    assert!(offset_of!(TrapContext, x) == 0, "x 数组应从偏移 0 开始");
    assert!(
        offset_of!(TrapContext, _padding0) == 248,
        "_padding0 应在偏移 248"
    );
    assert!(
        offset_of!(TrapContext, elr_el1) == 256,
        "elr_el1 应在偏移 256"
    );
    assert!(
        offset_of!(TrapContext, spsr_el1) == 264,
        "spsr_el1 应在偏移 264"
    );
    assert!(
        offset_of!(TrapContext, esr_el1) == 272,
        "esr_el1 应在偏移 272"
    );
    assert!(
        offset_of!(TrapContext, sp_el0) == 280,
        "sp_el0 应在偏移 280"
    );
    assert!(
        offset_of!(TrapContext, tpidr_el0) == 288,
        "tpidr_el0 应在偏移 288"
    );
    assert!(
        offset_of!(TrapContext, ttbr0_el1) == 296,
        "ttbr0_el1 应在偏移 296"
    );
    assert!(
        offset_of!(TrapContext, sp_el1) == 304,
        "sp_el1 应在偏移 304"
    );
    assert!(
        offset_of!(TrapContext, tpidr_el1) == 312,
        "tpidr_el1 应在偏移 312"
    );
};

/// 被调用者保存上下文 — 任务切换时保存/恢复
///
/// 大小：112 字节，与 macro.S 中 `kCalleeSavedContextSize` 一致。
///
/// 布局：
/// - 偏移   0–95: x19–x30（12 个寄存器）
/// - 偏移  96: sp（当前栈指针）
/// - 偏移 104: pc（恢复后跳转地址，对应 x30/lr）
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CalleeSavedContext {
    /// x19–x30 被调用者保存寄存器（12 个）
    pub regs: [u64; 12],
    /// 栈指针
    pub sp: u64,
    /// 恢复后跳转地址（link register）
    pub pc: u64,
}

const _: () = {
    assert!(
        core::mem::size_of::<CalleeSavedContext>() == 112,
        "CalleeSavedContext 大小必须为 112 字节"
    );
    assert!(offset_of!(CalleeSavedContext, regs) == 0);
    assert!(offset_of!(CalleeSavedContext, sp) == 96);
    assert!(offset_of!(CalleeSavedContext, pc) == 104);
};
