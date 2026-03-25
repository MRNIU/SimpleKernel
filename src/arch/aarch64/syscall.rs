/// AArch64 系统调用分发
///
/// SVC 异常（ESR_EL1.EC = 0x15）的处理入口。
use super::context::TrapContext;
use crate::syscall::SyscallNumber;

/// 处理系统调用
///
/// 从 TrapContext 中提取系统调用号（x8）和参数（x0–x5），
/// 分发到对应的系统调用处理函数，并将返回值写回 x0。
///
/// ELR_EL1 前进 4 字节以跳过 svc 指令。
///
/// # 参数
/// - `ctx`：指向陷阱上下文的可变引用，svc 参数从此读取，返回值写回此处
pub fn handle_syscall(ctx: &mut TrapContext) {
    // 跳过 svc 指令（固定 4 字节）
    ctx.elr_el1 = ctx.elr_el1.wrapping_add(4);

    // AArch64 Linux ABI: 系统调用号在 x8，参数在 x0–x5
    let syscall_num = ctx.x[8];
    let a0 = ctx.x[0];
    let a1 = ctx.x[1];
    let a2 = ctx.x[2];

    let ret = match SyscallNumber::from_u64(syscall_num) {
        Some(SyscallNumber::Write) => crate::syscall::sys_write(a0, a1, a2),
        Some(SyscallNumber::Exit) => crate::syscall::sys_exit(a0),
        Some(SyscallNumber::Yield) => crate::syscall::sys_yield(),
        None => {
            log::warn!("handle_syscall: 未知系统调用号 {}", syscall_num);
            -1i64
        }
    };

    ctx.x[0] = ret as u64;
}
