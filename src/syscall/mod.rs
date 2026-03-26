// 系统调用接口
//
// 定义系统调用号枚举和中央分发函数。
// 各系统调用的具体实现按类别放在子模块中。

#[cfg(not(test))]
mod io;
#[cfg(not(test))]
mod process;

/// 系统调用号
///
/// 与 Linux ABI 对齐，方便后续用户态程序移植。
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallNumber {
    /// write(fd, buf, len) — 写文件描述符
    Write = 64,
    /// exit(code) — 终止当前任务
    Exit = 93,
    /// sched_yield() — 主动让出 CPU
    Yield = 124,
    /// clone(entry, arg) — 创建子任务
    Clone = 220,
    /// waitpid(pid) — 等待子进程退出
    Waitpid = 260,
    /// nanosleep(ms) — 睡眠指定毫秒数
    Nanosleep = 101,
    /// kill(pid, sig) — 发送信号
    Kill = 129,
}

impl SyscallNumber {
    /// 从 u64 转换为 SyscallNumber，未知号返回 None
    pub fn from_u64(n: u64) -> Option<Self> {
        match n {
            64 => Some(Self::Write),
            93 => Some(Self::Exit),
            101 => Some(Self::Nanosleep),
            124 => Some(Self::Yield),
            129 => Some(Self::Kill),
            220 => Some(Self::Clone),
            260 => Some(Self::Waitpid),
            _ => None,
        }
    }
}

/// 系统调用中央分发入口
///
/// 架构侧从 TrapContext 提取系统调用号和参数后，统一调用此函数。
///
/// # 参数
/// - `nr`：系统调用号（原始 u64）
/// - `args`：最多 6 个参数
///
/// # 返回值
/// 系统调用返回值，-1 表示未知调用号
#[cfg(not(test))]
pub fn dispatch(nr: u64, args: [u64; 6]) -> i64 {
    match SyscallNumber::from_u64(nr) {
        Some(SyscallNumber::Write) => io::sys_write(args[0], args[1], args[2]),
        Some(SyscallNumber::Exit) => process::sys_exit(args[0]),
        Some(SyscallNumber::Yield) => process::sys_yield(),
        Some(SyscallNumber::Clone) => process::sys_clone(args[0], args[1]),
        Some(SyscallNumber::Waitpid) => process::sys_waitpid(args[0]),
        Some(SyscallNumber::Nanosleep) => process::sys_nanosleep(args[0]),
        Some(SyscallNumber::Kill) => process::sys_kill(args[0], args[1]),
        None => {
            log::warn!("syscall::dispatch: 未知系统调用号 {}", nr);
            -1i64
        }
    }
}
