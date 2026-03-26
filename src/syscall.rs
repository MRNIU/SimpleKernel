// 系统调用接口
//
// 定义系统调用号枚举、中央分发函数及各系统调用实现。
// 架构侧仅负责寄存器提取和返回值写回，分发逻辑统一在此处。

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
        Some(SyscallNumber::Write) => sys_write(args[0], args[1], args[2]),
        Some(SyscallNumber::Exit) => sys_exit(args[0]),
        Some(SyscallNumber::Yield) => sys_yield(),
        Some(SyscallNumber::Clone) => sys_clone(args[0], args[1]),
        Some(SyscallNumber::Waitpid) => sys_waitpid(args[0]),
        Some(SyscallNumber::Nanosleep) => sys_nanosleep(args[0]),
        Some(SyscallNumber::Kill) => sys_kill(args[0], args[1]),
        None => {
            log::warn!("syscall::dispatch: 未知系统调用号 {}", nr);
            -1i64
        }
    }
}

/// sys_write — 写文件描述符（当前为存根）
#[cfg(not(test))]
fn sys_write(_fd: u64, _buf: u64, _len: u64) -> i64 {
    0
}

/// sys_exit — 终止当前任务
#[cfg(not(test))]
fn sys_exit(code: u64) -> i64 {
    crate::task::exit(code as i32);
}

/// sys_yield — 主动让出 CPU
#[cfg(not(test))]
fn sys_yield() -> i64 {
    crate::task::yield_now();
    0
}

/// sys_clone — 创建子内核线程（简化版，entry 和 arg 需为合法函数指针）
#[cfg(not(test))]
fn sys_clone(_entry: u64, _arg: u64) -> i64 {
    // P5b 简化版：仅在内核态创建线程，用户态 clone 在 P6 实现
    // 此处返回 -1 表示暂不支持通过 syscall 创建线程
    -1
}

/// sys_waitpid — 等待子进程退出
#[cfg(not(test))]
fn sys_waitpid(pid: u64) -> i64 {
    match crate::task::wait_child(pid as usize) {
        Ok((child_pid, code)) => {
            // 将 pid 和 code 编码在返回值中：高 32 位 = pid，低 32 位 = code
            ((child_pid as i64) << 32) | ((code as i64) & 0xFFFF_FFFF)
        }
        Err(_) => -1,
    }
}

/// sys_nanosleep — 睡眠指定毫秒数
#[cfg(not(test))]
fn sys_nanosleep(ms: u64) -> i64 {
    crate::task::sleep_ms(ms);
    0
}

/// sys_kill — 发送信号
#[cfg(not(test))]
fn sys_kill(pid: u64, sig: u64) -> i64 {
    use crate::task::signal::Signal;
    let signal = match Signal::from_u8(sig as u8) {
        Some(s) => s,
        None => return -1,
    };
    match crate::task::send_signal(pid as usize, signal) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
