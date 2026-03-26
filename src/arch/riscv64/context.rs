/// RISC-V 64 陷阱上下文与被调用者保存寄存器上下文
///
/// 布局与 macro.S 中的宏严格对应，修改时需同步更新汇编代码。
// 编译时偏移验证
use core::mem::offset_of;

/// 陷阱上下文 — 保存发生中断/异常时所有通用寄存器及必要 CSR
///
/// 大小：288 字节（36 个字段 × 8 字节），与 macro.S 中 `kTrapContextSize` 一致。
///
/// 字段索引与汇编偏移的关系：`offset = index * 8`
/// - ra[0]=0, sp[1]=8, gp[2]=16, tp[3]=24
/// - t0[4]..t6[30]  各 +8
/// - sstatus[31]=248, sepc[32]=256, stval[33]=264, scause[34]=272
/// - _pad[35]=280（保持结构体 16 字节对齐）
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
    /// sstatus CSR (偏移 248)
    pub sstatus: u64,
    /// sepc CSR — 陷阱返回地址 (偏移 256)
    pub sepc: u64,
    /// stval CSR — 陷阱附加信息 (偏移 264)
    pub stval: u64,
    /// scause CSR — 陷阱原因 (偏移 272)
    pub scause: u64,
    /// 填充，保持 16 字节对齐 (偏移 280)
    pub _pad: u64,
}

// 编译时大小和偏移断言
const _: () = {
    assert!(
        core::mem::size_of::<TrapContext>() == 288,
        "TrapContext 大小必须为 288 字节"
    );
    assert!(offset_of!(TrapContext, ra) == 0);
    assert!(offset_of!(TrapContext, sp) == 8);
    assert!(offset_of!(TrapContext, a7) == 16 * 8, "a7 应在偏移 128");
    assert!(
        offset_of!(TrapContext, sstatus) == 31 * 8,
        "sstatus 应在偏移 248"
    );
    assert!(offset_of!(TrapContext, sepc) == 32 * 8, "sepc 应在偏移 256");
    assert!(
        offset_of!(TrapContext, stval) == 33 * 8,
        "stval 应在偏移 264"
    );
    assert!(
        offset_of!(TrapContext, scause) == 34 * 8,
        "scause 应在偏移 272"
    );
};

/// 被调用者保存上下文 — 任务切换时保存/恢复
///
/// 大小：112 字节（14 个字段 × 8 字节），与 macro.S 中 `kCalleeSavedContextSize` 一致。
///
/// 布局：ra[0], sp[1], s0[2]..s11[13]
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
}

const _: () = {
    assert!(
        core::mem::size_of::<CalleeSavedContext>() == 112,
        "CalleeSavedContext 大小必须为 112 字节"
    );
    assert!(offset_of!(CalleeSavedContext, ra) == 0);
    assert!(offset_of!(CalleeSavedContext, sp) == 8);
    assert!(offset_of!(CalleeSavedContext, s0) == 16);
    assert!(offset_of!(CalleeSavedContext, s11) == 13 * 8);
};

impl CalleeSavedContext {
    /// 初始化内核线程上下文，使 `switch_to` 后跳转到 `kernel_thread_entry`。
    ///
    /// - `ra` → `kernel_thread_entry`（汇编入口）
    /// - `sp` → 内核栈顶
    /// - `s0` → 入口函数指针
    /// - `s1` → 入口函数参数
    pub fn init_for_kernel_thread(&mut self, kstack_top: usize, entry: fn(usize), arg: usize) {
        unsafe extern "C" {
            fn kernel_thread_entry();
        }
        self.ra = kernel_thread_entry as unsafe extern "C" fn() as u64;
        self.sp = kstack_top as u64;
        self.s0 = entry as u64;
        self.s1 = arg as u64;
    }
}
