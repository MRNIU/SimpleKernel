//! 信号子系统——信号编号、掩码及默认动作。

use bitflags::bitflags;

/// 信号编号（1–31，与 POSIX 对齐）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// 挂起
    SIGHUP = 1,
    /// 中断（Ctrl+C）
    SIGINT = 2,
    /// 退出
    SIGQUIT = 3,
    /// 非法指令
    SIGILL = 4,
    /// 中止
    SIGABRT = 6,
    /// 浮点异常
    SIGFPE = 8,
    /// 强制终止（不可屏蔽）
    SIGKILL = 9,
    /// 段错误
    SIGSEGV = 11,
    /// 管道断开
    SIGPIPE = 13,
    /// 定时器
    SIGALRM = 14,
    /// 终止
    SIGTERM = 15,
    /// 用户定义 1
    SIGUSR1 = 10,
    /// 用户定义 2
    SIGUSR2 = 12,
    /// 子进程状态改变
    SIGCHLD = 17,
    /// 继续执行
    SIGCONT = 18,
    /// 停止（不可屏蔽）
    SIGSTOP = 19,
    /// 终端停止（Ctrl+Z）
    SIGTSTP = 20,
}

impl Signal {
    /// 从 u8 转换
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::SIGHUP),
            2 => Some(Self::SIGINT),
            3 => Some(Self::SIGQUIT),
            4 => Some(Self::SIGILL),
            6 => Some(Self::SIGABRT),
            8 => Some(Self::SIGFPE),
            9 => Some(Self::SIGKILL),
            10 => Some(Self::SIGUSR1),
            11 => Some(Self::SIGSEGV),
            12 => Some(Self::SIGUSR2),
            13 => Some(Self::SIGPIPE),
            14 => Some(Self::SIGALRM),
            15 => Some(Self::SIGTERM),
            17 => Some(Self::SIGCHLD),
            18 => Some(Self::SIGCONT),
            19 => Some(Self::SIGSTOP),
            20 => Some(Self::SIGTSTP),
            _ => None,
        }
    }

    /// 是否不可屏蔽
    pub fn is_uncatchable(self) -> bool {
        matches!(self, Signal::SIGKILL | Signal::SIGSTOP)
    }

    /// 默认动作
    pub fn default_action(self) -> SignalAction {
        match self {
            Signal::SIGCHLD | Signal::SIGCONT => SignalAction::Ignore,
            Signal::SIGSTOP | Signal::SIGTSTP => SignalAction::Stop,
            _ => SignalAction::Terminate,
        }
    }
}

/// 信号的默认处理动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    /// 终止任务
    Terminate,
    /// 停止（挂起）任务
    Stop,
    /// 忽略
    Ignore,
}

bitflags! {
    /// 信号掩码——位图表示哪些信号被屏蔽。
    ///
    /// 位 N 对应信号 N（1–31），位 0 未使用。
    /// SIGKILL(9) 和 SIGSTOP(19) 不可屏蔽：即使 mask 中设置了也会被强制投递。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SignalMask: u32 {
        const SIGHUP  = 1 << 1;
        const SIGINT  = 1 << 2;
        const SIGQUIT = 1 << 3;
        const SIGILL  = 1 << 4;
        const SIGABRT = 1 << 6;
        const SIGFPE  = 1 << 8;
        const SIGUSR1 = 1 << 10;
        const SIGSEGV = 1 << 11;
        const SIGUSR2 = 1 << 12;
        const SIGPIPE = 1 << 13;
        const SIGALRM = 1 << 14;
        const SIGTERM = 1 << 15;
        const SIGCHLD = 1 << 17;
        const SIGCONT = 1 << 18;
        const SIGTSTP = 1 << 20;
    }
}

impl SignalMask {
    /// 将单个信号转为掩码位
    pub fn from_signal(sig: Signal) -> Self {
        Self::from_bits_truncate(1 << (sig as u32))
    }
}

/// 不可屏蔽信号掩码（SIGKILL | SIGSTOP）
pub const UNCATCHABLE_MASK: u32 = (1 << 9) | (1 << 19);

/// 获取待处理信号中第一个未被屏蔽的信号
///
/// # 参数
/// - `pending`：待处理信号位图
/// - `mask`：被屏蔽的信号
///
/// # 返回值
/// 第一个可投递的信号，或 None
pub fn first_deliverable(pending: u32, mask: SignalMask) -> Option<Signal> {
    // 不可屏蔽信号总是可投递
    let effective_mask = mask.bits() & !UNCATCHABLE_MASK;
    let deliverable = pending & !effective_mask;
    if deliverable == 0 {
        return None;
    }
    let bit = deliverable.trailing_zeros() as u8;
    Signal::from_u8(bit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_uncatchable() {
        assert!(Signal::SIGKILL.is_uncatchable());
        assert!(Signal::SIGSTOP.is_uncatchable());
        assert!(!Signal::SIGTERM.is_uncatchable());
    }

    #[test]
    fn signal_default_actions() {
        assert_eq!(Signal::SIGKILL.default_action(), SignalAction::Terminate);
        assert_eq!(Signal::SIGSTOP.default_action(), SignalAction::Stop);
        assert_eq!(Signal::SIGCHLD.default_action(), SignalAction::Ignore);
    }

    #[test]
    fn signal_mask_from_signal() {
        let mask = SignalMask::from_signal(Signal::SIGTERM);
        assert_eq!(mask.bits(), 1 << 15);
    }

    #[test]
    fn first_deliverable_basic() {
        // pending: SIGTERM(15)，无屏蔽
        let sig = first_deliverable(1 << 15, SignalMask::empty());
        assert_eq!(sig, Some(Signal::SIGTERM));
    }

    #[test]
    fn first_deliverable_masked() {
        // pending: SIGTERM(15)，SIGTERM 被屏蔽
        let sig = first_deliverable(1 << 15, SignalMask::SIGTERM);
        assert_eq!(sig, None);
    }

    #[test]
    fn sigkill_cannot_be_masked() {
        // pending: SIGKILL(9)，尝试屏蔽
        let mask = SignalMask::from_bits_truncate(1 << 9);
        let sig = first_deliverable(1 << 9, mask);
        assert_eq!(sig, Some(Signal::SIGKILL));
    }

    #[test]
    fn from_u8_roundtrip() {
        assert_eq!(Signal::from_u8(9), Some(Signal::SIGKILL));
        assert_eq!(Signal::from_u8(0), None);
        assert_eq!(Signal::from_u8(255), None);
    }
}
