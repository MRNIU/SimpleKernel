//! 任务状态机——定义任务的生命周期状态、消息以及状态转移函数。

use core::sync::atomic::{AtomicU8, Ordering};

// ─── TaskState ────────────────────────────────────────────────────────────────

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
    /// 已退出——已调用 exit，等待父进程回收。
    Exited = 5,
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
            _ => None,
        }
    }
}

// ─── TaskMsg ──────────────────────────────────────────────────────────────────

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
}

// ─── InvalidTransition ────────────────────────────────────────────────────────

/// 非法状态转移错误——记录转移发生时的原始状态与消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    /// 转移时任务所处的状态。
    pub from: TaskState,
    /// 触发转移的消息。
    pub msg: TaskMsg,
}

// ─── transition ───────────────────────────────────────────────────────────────

/// 根据当前状态与消息计算下一个状态。
///
/// 合法的转移路径：
/// - `(UnInit, Schedule)` → `Ready`
/// - `(Ready, PickUp)` → `Running`
/// - `(Running, Yield)` → `Ready`
/// - `(Running, Exit { .. })` → `Exited`
/// - `(Sleeping, Wakeup)` → `Ready`
/// - `(Blocked, Wakeup)` → `Ready`
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
        (TaskState::Sleeping, TaskMsg::Wakeup) => Ok(TaskState::Ready),
        (TaskState::Blocked, TaskMsg::Wakeup) => Ok(TaskState::Ready),
        _ => Err(InvalidTransition { from, msg }),
    }
}

// ─── AtomicTaskState ──────────────────────────────────────────────────────────

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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证所有 6 条合法转移路径均返回预期的目标状态。
    #[test]
    fn valid_transitions() {
        assert_eq!(
            transition(TaskState::UnInit, TaskMsg::Schedule),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition(TaskState::Ready, TaskMsg::PickUp),
            Ok(TaskState::Running)
        );
        assert_eq!(
            transition(TaskState::Running, TaskMsg::Yield),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition(TaskState::Running, TaskMsg::Exit { code: 0 }),
            Ok(TaskState::Exited)
        );
        assert_eq!(
            transition(TaskState::Running, TaskMsg::Exit { code: -1 }),
            Ok(TaskState::Exited)
        );
        assert_eq!(
            transition(TaskState::Sleeping, TaskMsg::Wakeup),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition(TaskState::Blocked, TaskMsg::Wakeup),
            Ok(TaskState::Ready)
        );
    }

    /// 验证非法转移路径均被拒绝并返回正确的错误信息。
    #[test]
    fn invalid_transitions_rejected() {
        // UnInit 状态下不能 PickUp
        assert_eq!(
            transition(TaskState::UnInit, TaskMsg::PickUp),
            Err(InvalidTransition {
                from: TaskState::UnInit,
                msg: TaskMsg::PickUp,
            })
        );

        // Ready 状态下不能 Wakeup
        assert_eq!(
            transition(TaskState::Ready, TaskMsg::Wakeup),
            Err(InvalidTransition {
                from: TaskState::Ready,
                msg: TaskMsg::Wakeup,
            })
        );

        // Exited 状态下不能再 Schedule
        assert_eq!(
            transition(TaskState::Exited, TaskMsg::Schedule),
            Err(InvalidTransition {
                from: TaskState::Exited,
                msg: TaskMsg::Schedule,
            })
        );

        // Sleeping 状态下不能 Yield（必须先 Wakeup → Ready → PickUp → Running → Yield）
        assert_eq!(
            transition(TaskState::Sleeping, TaskMsg::Yield),
            Err(InvalidTransition {
                from: TaskState::Sleeping,
                msg: TaskMsg::Yield,
            })
        );
    }

    /// 验证通过 `AtomicU8` 存取 `TaskState` 的完整往返正确性。
    #[test]
    fn atomic_state_roundtrip() {
        let atomic = AtomicTaskState::new(TaskState::UnInit);
        assert_eq!(atomic.load(), TaskState::UnInit);

        atomic.store(TaskState::Ready);
        assert_eq!(atomic.load(), TaskState::Ready);

        atomic.store(TaskState::Running);
        assert_eq!(atomic.load(), TaskState::Running);

        atomic.store(TaskState::Sleeping);
        assert_eq!(atomic.load(), TaskState::Sleeping);

        atomic.store(TaskState::Blocked);
        assert_eq!(atomic.load(), TaskState::Blocked);

        atomic.store(TaskState::Exited);
        assert_eq!(atomic.load(), TaskState::Exited);
    }

    /// 验证 `from_u8` 对所有已知变体正确往返。
    #[test]
    fn from_u8_roundtrip() {
        for v in 0u8..=5 {
            let state = TaskState::from_u8(v).expect("应能解析 0..=5");
            assert_eq!(state.as_u8(), v);
        }
        assert!(TaskState::from_u8(6).is_none());
        assert!(TaskState::from_u8(255).is_none());
    }
}
