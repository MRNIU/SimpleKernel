// Copyright The SimpleKernel Contributors

/// RISC-V 64 陷阱上下文与被调用者保存寄存器上下文
///
/// 布局与 macro.S 中的宏严格对应，修改时需同步更新汇编代码。
// 编译时偏移验证
use core::mem::offset_of;

/// 陷阱上下文 — 保存发生中断/异常时所有通用寄存器、浮点寄存器及必要 CSR
///
/// 大小：544 字节（68 个字段 × 8 字节），与 `interrupt.S` 中 `kTrapContextSize` 一致。
///
/// 字段索引与汇编偏移的关系：`offset = index * 8`
/// - ra[0]=0, sp[1]=8, gp[2]=16, tp[3]=24
/// - t0[4]..t6[30]  各 +8
/// - ft0[31]..ft11[62] 以 RISC-V ABI 浮点寄存器顺序保存
/// - fcsr[63]=504
/// - sstatus[64]=512, sepc[65]=520, stval[66]=528, scause[67]=536
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct TrapContext {
    /// x1  返回地址
    pub ra: u64,
    /// x2  栈指针
    pub sp: u64,
    /// x3  全局指针
    pub gp: u64,
    /// x4  线程指针
    pub tp: u64,
    /// x5  临时寄存器
    pub t0: u64,
    /// x6
    pub t1: u64,
    /// x7
    pub t2: u64,
    /// x8  帧指针 / 保存寄存器 0
    pub s0: u64,
    /// x9  保存寄存器 1
    pub s1: u64,
    /// x10 函数参数 / 返回值 0
    pub a0: u64,
    /// x11 函数参数 / 返回值 1
    pub a1: u64,
    /// x12 函数参数 2
    pub a2: u64,
    /// x13 函数参数 3
    pub a3: u64,
    /// x14 函数参数 4
    pub a4: u64,
    /// x15 函数参数 5
    pub a5: u64,
    /// x16 函数参数 6
    pub a6: u64,
    /// x17 函数参数 7 / syscall 号
    pub a7: u64,
    /// x18 保存寄存器 2
    pub s2: u64,
    /// x19
    pub s3: u64,
    /// x20
    pub s4: u64,
    /// x21
    pub s5: u64,
    /// x22
    pub s6: u64,
    /// x23
    pub s7: u64,
    /// x24
    pub s8: u64,
    /// x25
    pub s9: u64,
    /// x26
    pub s10: u64,
    /// x27
    pub s11: u64,
    /// x28 临时寄存器
    pub t3: u64,
    /// x29
    pub t4: u64,
    /// x30
    pub t5: u64,
    /// x31
    pub t6: u64,
    /// f0  临时浮点寄存器
    pub ft0: u64,
    /// f1
    pub ft1: u64,
    /// f2
    pub ft2: u64,
    /// f3
    pub ft3: u64,
    /// f4
    pub ft4: u64,
    /// f5
    pub ft5: u64,
    /// f6
    pub ft6: u64,
    /// f7
    pub ft7: u64,
    /// f8  被调用者保存浮点寄存器 0
    pub fs0: u64,
    /// f9  被调用者保存浮点寄存器 1
    pub fs1: u64,
    /// f10 函数参数 / 返回值浮点寄存器 0
    pub fa0: u64,
    /// f11 函数参数 / 返回值浮点寄存器 1
    pub fa1: u64,
    /// f12 函数参数浮点寄存器 2
    pub fa2: u64,
    /// f13 函数参数浮点寄存器 3
    pub fa3: u64,
    /// f14 函数参数浮点寄存器 4
    pub fa4: u64,
    /// f15 函数参数浮点寄存器 5
    pub fa5: u64,
    /// f16 函数参数浮点寄存器 6
    pub fa6: u64,
    /// f17 函数参数浮点寄存器 7
    pub fa7: u64,
    /// f18 被调用者保存浮点寄存器 2
    pub fs2: u64,
    /// f19
    pub fs3: u64,
    /// f20
    pub fs4: u64,
    /// f21
    pub fs5: u64,
    /// f22
    pub fs6: u64,
    /// f23
    pub fs7: u64,
    /// f24
    pub fs8: u64,
    /// f25
    pub fs9: u64,
    /// f26
    pub fs10: u64,
    /// f27
    pub fs11: u64,
    /// f28 临时浮点寄存器
    pub ft8: u64,
    /// f29
    pub ft9: u64,
    /// f30
    pub ft10: u64,
    /// f31
    pub ft11: u64,
    /// fcsr 浮点控制状态寄存器 (偏移 504)
    pub fcsr: u64,
    /// sstatus CSR (偏移 512)
    pub sstatus: u64,
    /// sepc CSR — 陷阱返回地址 (偏移 520)
    pub sepc: u64,
    /// stval CSR — 陷阱附加信息 (偏移 528)
    pub stval: u64,
    /// scause CSR — 陷阱原因 (偏移 536)
    pub scause: u64,
}

// 编译时大小和偏移断言
const _: () = {
    assert!(
        core::mem::size_of::<TrapContext>() == 544,
        "TrapContext 大小必须为 544 字节"
    );
    assert!(offset_of!(TrapContext, ra) == 0);
    assert!(offset_of!(TrapContext, sp) == 8);
    assert!(offset_of!(TrapContext, a7) == 16 * 8, "a7 应在偏移 128");
    assert!(offset_of!(TrapContext, ft0) == 31 * 8, "ft0 应在偏移 248");
    assert!(offset_of!(TrapContext, fs0) == 39 * 8, "fs0 应在偏移 312");
    assert!(offset_of!(TrapContext, fa0) == 41 * 8, "fa0 应在偏移 328");
    assert!(offset_of!(TrapContext, fs11) == 58 * 8, "fs11 应在偏移 464");
    assert!(offset_of!(TrapContext, ft11) == 62 * 8, "ft11 应在偏移 496");
    assert!(offset_of!(TrapContext, fcsr) == 63 * 8, "fcsr 应在偏移 504");
    assert!(
        offset_of!(TrapContext, sstatus) == 64 * 8,
        "sstatus 应在偏移 512"
    );
    assert!(offset_of!(TrapContext, sepc) == 65 * 8, "sepc 应在偏移 520");
    assert!(
        offset_of!(TrapContext, stval) == 66 * 8,
        "stval 应在偏移 528"
    );
    assert!(
        offset_of!(TrapContext, scause) == 67 * 8,
        "scause 应在偏移 536"
    );
};

/// 被调用者保存上下文 — 任务切换时保存/恢复整数与浮点 callee-saved 寄存器
///
/// 大小：208 字节（26 个字段 × 8 字节），与 `switch.rs` 中 `switch_to` 一致。
///
/// 布局：ra[0], sp[1], s0[2]..s11[13], fs0[14]..fs11[25]
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct CalleeSavedContext {
    /// 返回地址 (切换后跳转目标)
    pub ra: u64,
    /// 栈指针
    pub sp: u64,
    /// s0-s11 被调用者保存寄存器
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
    /// fs0-fs11 被调用者保存浮点寄存器。
    pub fs0: u64,
    pub fs1: u64,
    pub fs2: u64,
    pub fs3: u64,
    pub fs4: u64,
    pub fs5: u64,
    pub fs6: u64,
    pub fs7: u64,
    pub fs8: u64,
    pub fs9: u64,
    pub fs10: u64,
    pub fs11: u64,
}

const _: () = {
    assert!(
        core::mem::size_of::<CalleeSavedContext>() == 208,
        "CalleeSavedContext 大小必须为 208 字节"
    );
    assert!(offset_of!(CalleeSavedContext, ra) == 0);
    assert!(offset_of!(CalleeSavedContext, sp) == 8);
    assert!(offset_of!(CalleeSavedContext, s0) == 16);
    assert!(offset_of!(CalleeSavedContext, s11) == 13 * 8);
    assert!(offset_of!(CalleeSavedContext, fs0) == 14 * 8);
    assert!(offset_of!(CalleeSavedContext, fs11) == 25 * 8);
};

impl CalleeSavedContext {
    /// 初始化内核线程上下文，使 `switch_to` 后跳转到 `kernel_thread_entry`。
    ///
    /// - `ra` → `kernel_thread_entry`（汇编入口）
    /// - `sp` → 内核栈顶
    /// - `s0` → 入口函数指针
    /// - `s1` → 入口函数参数
    pub fn init_for_kernel_thread(&mut self, kstack_top: usize, entry: fn(usize), arg: usize) {
        self.ra = super::switch::kernel_thread_entry as unsafe extern "C" fn() as usize as u64;
        self.sp = kstack_top as u64;
        self.s0 = entry as usize as u64;
        self.s1 = arg as u64;
    }
}
