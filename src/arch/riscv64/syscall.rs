/// RISC-V 64 系统调用处理
///
/// ecall 异常（exception code 8 = U-mode ecall，9 = S-mode ecall）的处理入口。
/// 仅负责寄存器提取、指令指针推进和返回值写回，分发逻辑由 `crate::syscall::dispatch` 统一处理。
use super::context::TrapContext;

/// 处理系统调用
///
/// 从 TrapContext 中提取系统调用号（a7）和参数（a0–a5），
/// 委托 `syscall::dispatch` 进行分发，并将返回值写回 a0。
///
/// sepc 前进 4 字节以跳过 ecall 指令。
///
/// # 参数
/// - `ctx`：指向陷阱上下文的可变引用，ecall 参数从此读取，返回值写回此处
pub fn handle_syscall(ctx: &mut TrapContext) {
    // 跳过 ecall 指令（固定 4 字节）
    ctx.sepc = ctx.sepc.wrapping_add(4);

    let ret = crate::syscall::dispatch(ctx.a7, [ctx.a0, ctx.a1, ctx.a2, ctx.a3, ctx.a4, ctx.a5]);

    ctx.a0 = ret as u64;
}
