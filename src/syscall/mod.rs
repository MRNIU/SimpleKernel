// 系统调用接口（SAS 模式——类型安全的集中式 API 网关）
//
// 单地址空间架构下，syscall 层是跨模块操作的唯一公开入口。
// 不经过 trap（ecall/svc），调用者直接以 Rust 函数调用方式进入。
// SyscallNumber 枚举保留用于日志、审计和 POSIX 合规追踪。

#[cfg(target_os = "none")]
pub mod io;
#[cfg(target_os = "none")]
pub mod process;

/// 系统调用号（对齐 Linux ABI）
///
/// 保留用于日志、审计、统计及 POSIX 合规追踪。
/// SAS 模式下不再作为运行时分发键——分发在编译期由函数调用直接完成。
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallNumber {
    /// write(fd, buf, len) — 写文件描述符
    Write = 64,
    /// exit(code) — 终止当前任务
    Exit = 93,
    /// nanosleep(ms) — 睡眠指定毫秒数
    Nanosleep = 101,
    /// sched_yield() — 主动让出 CPU
    Yield = 124,
    /// kill(pid, sig) — 发送信号
    Kill = 129,
    /// clone(entry, arg) — 创建子任务
    Clone = 220,
    /// waitpid(pid) — 等待子进程退出
    Waitpid = 260,
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
