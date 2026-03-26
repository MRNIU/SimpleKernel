/// AArch64 系统调用处理
///
/// SVC 异常（ESR_EL1.EC = 0x15）的处理入口。
/// 仅负责寄存器提取、指令指针推进和返回值写回，分发逻辑由 `crate::syscall::dispatch` 统一处理。
use super::context::TrapContext;

/// 处理系统调用
///
/// 从 TrapContext 中提取系统调用号（x8）和参数（x0–x5），
/// 委托 `syscall::dispatch` 进行分发，并将返回值写回 x0。
///
/// ELR_EL1 前进 4 字节以跳过 svc 指令。
///
/// # 参数
/// - `ctx`：指向陷阱上下文的可变引用，svc 参数从此读取，返回值写回此处
pub fn handle_syscall(ctx: &mut TrapContext) {
    // 跳过 svc 指令（固定 4 字节）
    ctx.elr_el1 = ctx.elr_el1.wrapping_add(4);

    // AArch64 Linux ABI: 系统调用号在 x8，参数在 x0–x5
    let ret = crate::syscall::dispatch(
        ctx.x[8],
        [ctx.x[0], ctx.x[1], ctx.x[2], ctx.x[3], ctx.x[4], ctx.x[5]],
    );

    ctx.x[0] = ret as u64;
}
