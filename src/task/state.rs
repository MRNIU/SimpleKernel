// Copyright The SimpleKernel Contributors

//! 任务状态机——定义任务的生命周期状态、消息以及状态转移函数。

use core::sync::atomic::{AtomicU8, Ordering};

/// 任务的生命周期状态。
///
/// 使用 `#[repr(u8)]` 以便与 `AtomicU8` 配合使用。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 尚未初始化——任务控制块已分配，但未就绪。
    UnInit = 0,
    /// 就绪——可被调度器选中运行。
    Ready = 1,
    /// 运行中——当前正在某个 CPU 核上执行。
    Running = 2,
    /// 睡眠中——等待定时器唤醒。
    Sleeping = 3,
    /// 阻塞中——等待某个事件（锁、I/O 等）唤醒。
    Blocked = 4,
    /// 已退出——已调用 exit，等待父进程回收（僵尸态）。
    Exited = 5,
    /// 已停止——收到 SIGSTOP/SIGTSTP。
    Stopped = 6,
}

impl TaskState {
    /// 返回状态对应的原始 `u8` 值，用于 `AtomicU8` 存储。
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// 将原始 `u8` 值转换回 `TaskState`。
    ///
    /// # Errors
    ///
    /// 若 `v` 不对应任何已知变体，返回 `None`。
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::UnInit),
            1 => Some(Self::Ready),
            2 => Some(Self::Running),
            3 => Some(Self::Sleeping),
            4 => Some(Self::Blocked),
            5 => Some(Self::Exited),
            6 => Some(Self::Stopped),
            _ => None,
        }
    }
}

/// 驱动状态转移的消息（事件）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskMsg {
    /// 调度器将任务纳入就绪队列。
    Schedule,
    /// 调度器从就绪队列中选中该任务，开始执行。
    PickUp,
    /// 任务主动让出 CPU。
    Yield,
    /// 任务调用 exit 系统调用，携带退出码。
    Exit {
        /// 退出码，将传递给父进程。
        code: i32,
    },
    /// 外部事件（定时器/锁释放等）唤醒任务。
    Wakeup,
    /// 任务进入睡眠。
    Sleep,
    /// 任务在资源上阻塞。
    Block,
    /// 停止信号（SIGSTOP/SIGTSTP）。
    Stop,
    /// 继续信号（SIGCONT）。
    Cont,
    /// 父进程回收僵尸子进程。
    Reap,
}

/// 非法状态转移错误——记录转移发生时的原始状态与消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    /// 转移时任务所处的状态。
    pub from: TaskState,
    /// 触发转移的消息。
    pub msg: TaskMsg,
}

/// 根据当前状态与消息计算下一个状态。
///
/// 合法的转移路径：
/// - `(UnInit, Schedule)` → `Ready`
/// - `(Ready, PickUp)` → `Running`
/// - `(Running, Yield)` → `Ready`
/// - `(Running, Exit { .. })` → `Exited`
/// - `(Running, Sleep)` → `Sleeping`
/// - `(Running, Block)` → `Blocked`
/// - `(Running, Stop)` → `Stopped`
/// - `(Sleeping, Wakeup)` → `Ready`
/// - `(Blocked, Wakeup)` → `Ready`
/// - `(Stopped, Cont)` → `Ready`
/// - `(Exited, Reap)` → `Exited`（保持，父进程回收后可释放）
///
/// # Errors
///
/// 若 `(from, msg)` 组合不在合法路径中，返回 `Err(InvalidTransition)`。
pub fn transition(from: TaskState, msg: TaskMsg) -> Result<TaskState, InvalidTransition> {
    match (from, msg) {
        (TaskState::UnInit, TaskMsg::Schedule) => Ok(TaskState::Ready),
        (TaskState::Ready, TaskMsg::PickUp) => Ok(TaskState::Running),
        (TaskState::Running, TaskMsg::Yield) => Ok(TaskState::Ready),
        (TaskState::Running, TaskMsg::Exit { .. }) => Ok(TaskState::Exited),
        (TaskState::Running, TaskMsg::Sleep) => Ok(TaskState::Sleeping),
        (TaskState::Running, TaskMsg::Block) => Ok(TaskState::Blocked),
        (TaskState::Running, TaskMsg::Stop) => Ok(TaskState::Stopped),
        (TaskState::Sleeping, TaskMsg::Wakeup) => Ok(TaskState::Ready),
        (TaskState::Blocked, TaskMsg::Wakeup) => Ok(TaskState::Ready),
        (TaskState::Stopped, TaskMsg::Cont) => Ok(TaskState::Ready),
        (TaskState::Exited, TaskMsg::Reap) => Ok(TaskState::Exited),
        _ => Err(InvalidTransition { from, msg }),
    }
}

/// 基于 `AtomicU8` 的原子任务状态，供多核环境使用。
pub struct AtomicTaskState(AtomicU8);

impl AtomicTaskState {
    /// 以给定初始状态创建原子任务状态。
    pub const fn new(state: TaskState) -> Self {
        Self(AtomicU8::new(state as u8))
    }

    /// 以 `Acquire` 语序读取当前状态。
    ///
    /// # Panics
    ///
    /// 若底层存储值不对应任何已知 `TaskState` 变体则 panic（不应发生）。
    pub fn load(&self) -> TaskState {
        TaskState::from_u8(self.0.load(Ordering::Acquire))
            .expect("AtomicTaskState: 存储了无效的状态值")
    }

    /// 以 `Release` 语序写入新状态。
    pub fn store(&self, state: TaskState) {
        self.0.store(state.as_u8(), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 表驱动验证所有合法转移路径。
    #[test]
    fn valid_transitions() {
        use TaskMsg::*;
        use TaskState::*;
        let cases: &[(TaskState, TaskMsg, TaskState)] = &[
            (UnInit, Schedule, Ready),
            (Ready, PickUp, Running),
            (Running, Yield, Ready),
            (Running, Exit { code: 0 }, Exited),
            (Running, Exit { code: -1 }, Exited),
            (Running, Sleep, Sleeping),
            (Running, Block, Blocked),
            (Running, Stop, Stopped),
            (Sleeping, Wakeup, Ready),
            (Blocked, Wakeup, Ready),
            (Stopped, Cont, Ready),
            (Exited, Reap, Exited),
        ];
        for &(from, ref msg, expected) in cases {
            assert_eq!(transition(from, msg.clone()), Ok(expected));
        }
    }

    /// 验证非法转移路径均被拒绝。
    #[test]
    fn invalid_transitions_rejected() {
        use TaskMsg::*;
        use TaskState::*;
        let cases: &[(TaskState, TaskMsg)] = &[
            (UnInit, PickUp),
            (Ready, Wakeup),
            (Exited, Schedule),
            (Sleeping, Yield),
        ];
        for &(from, ref msg) in cases {
            assert!(transition(from, msg.clone()).is_err());
        }
    }

    /// 验证 `from_u8` / `as_u8` 往返 + AtomicTaskState 存取。
    #[test]
    fn roundtrip_and_atomic() {
        for v in 0u8..=6 {
            let state = TaskState::from_u8(v).expect("应能解析 0..=6");
            assert_eq!(state.as_u8(), v);
        }
        assert!(TaskState::from_u8(7).is_none());

        let atomic = AtomicTaskState::new(TaskState::UnInit);
        for &s in &[
            TaskState::Ready,
            TaskState::Running,
            TaskState::Sleeping,
            TaskState::Blocked,
            TaskState::Exited,
            TaskState::Stopped,
        ] {
            atomic.store(s);
            assert_eq!(atomic.load(), s);
        }
    }
}
