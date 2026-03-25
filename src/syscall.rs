/// 系统调用接口
///
/// 定义系统调用号枚举及各系统调用的存根实现（P4 阶段）。
/// P5 任务管理实现后，各存根将替换为真实逻辑。

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
}

impl SyscallNumber {
    /// 从 u64 转换为 SyscallNumber，未知号返回 None
    pub fn from_u64(n: u64) -> Option<Self> {
        match n {
            64 => Some(Self::Write),
            93 => Some(Self::Exit),
            124 => Some(Self::Yield),
            _ => None,
        }
    }
}

/// sys_write 存根
///
/// # 参数
/// - `_fd`：文件描述符
/// - `_buf`：用户缓冲区地址
/// - `_len`：写入字节数
///
/// # 返回值
/// 实际写入字节数（当前为 0）
pub fn sys_write(_fd: u64, _buf: u64, _len: u64) -> i64 {
    0
}

/// sys_exit 存根
///
/// # 参数
/// - `_code`：退出码
///
/// # 返回值
/// 不应返回（当前为 0 存根）
pub fn sys_exit(_code: u64) -> i64 {
    0
}

/// sys_yield 存根
///
/// 主动让出 CPU，触发调度器重新调度。
///
/// # 返回值
/// 0 表示成功
pub fn sys_yield() -> i64 {
    0
}
