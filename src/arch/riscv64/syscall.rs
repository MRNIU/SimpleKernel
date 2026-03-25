/// RISC-V 64 系统调用分发
///
/// ecall 异常（exception code 8 = U-mode ecall，9 = S-mode ecall）的处理入口。
use super::context::TrapContext;
use crate::syscall::SyscallNumber;

/// 处理系统调用
///
/// 从 TrapContext 中提取系统调用号（a7）和参数（a0–a5），
/// 分发到对应的系统调用处理函数，并将返回值写回 a0。
///
/// sepc 前进 4 字节以跳过 ecall 指令。
///
/// # 参数
/// - `ctx`：指向陷阱上下文的可变引用，ecall 参数从此读取，返回值写回此处
pub fn handle_syscall(ctx: &mut TrapContext) {
    // 跳过 ecall 指令（固定 4 字节）
    ctx.sepc = ctx.sepc.wrapping_add(4);

    let syscall_num = ctx.a7;
    let a0 = ctx.a0;
    let a1 = ctx.a1;
    let a2 = ctx.a2;

    let ret = match SyscallNumber::from_u64(syscall_num) {
        Some(SyscallNumber::Write) => crate::syscall::sys_write(a0, a1, a2),
        Some(SyscallNumber::Exit) => crate::syscall::sys_exit(a0),
        Some(SyscallNumber::Yield) => crate::syscall::sys_yield(),
        None => {
            log::warn!("handle_syscall: 未知系统调用号 {}", syscall_num);
            -1i64
        }
    };

    ctx.a0 = ret as u64;
}
