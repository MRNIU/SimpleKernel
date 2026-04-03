//! 全局任务表——持有所有任务、睡眠队列和等待队列。

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::task::resource_id::ResourceId;
use crate::task::sched::PerCpuSched;
use crate::task::scheduler::Scheduler;
use crate::task::signal::{SignalAction, SignalMask, first_deliverable};
use crate::task::state::TaskState;
use crate::task::tcb::{Pid, TaskRef};
use sync::SpinLockIrq;
use sync::lock_level;

/// 全局任务表——由 `TASK_TABLE` 的 `SpinLockIrq` 保护（级别 1，高于调度锁级别 0）。
pub(super) struct TaskTable {
    pub(super) tasks: Vec<TaskRef>,
    next_pid: Pid,
    pub(super) sleep_queue: Vec<TaskRef>,
    wait_queues: BTreeMap<ResourceId, VecDeque<TaskRef>>,
}

impl TaskTable {
    pub(super) fn new() -> Self {
        Self {
            tasks: Vec::new(),
            next_pid: 1,
            sleep_queue: Vec::new(),
            wait_queues: BTreeMap::new(),
        }
    }

    /// 唤醒等待指定资源的一个任务——加入指定核心的就绪队列。
    ///
    /// 使用 `VecDeque::pop_front()` 实现 O(1) 出队（原 `Vec::remove(0)` 为 O(n)）。
    pub(super) fn wake_one(&mut self, sched: &mut PerCpuSched, resource: ResourceId) {
        if let Some(waiters) = self.wait_queues.get_mut(&resource) {
            if let Some(task) = waiters.pop_front() {
                task.set_state(TaskState::Ready);
                sched.scheduler.enqueue(task);
            }
            if waiters.is_empty() {
                self.wait_queues.remove(&resource);
            }
        }
    }

    /// 唤醒等待指定资源的所有任务。
    pub(super) fn wake_all(&mut self, sched: &mut PerCpuSched, resource: ResourceId) {
        if let Some(waiters) = self.wait_queues.remove(&resource) {
            for task in waiters {
                task.set_state(TaskState::Ready);
                sched.scheduler.enqueue(task);
            }
        }
    }

    /// 唤醒到期的睡眠任务。
    pub(super) fn wake_expired_sleepers(&mut self, sched: &mut PerCpuSched) {
        let now = global_tick::current();
        let mut i = 0;
        while i < self.sleep_queue.len() {
            let task = &self.sleep_queue[i];
            if task.wake_tick() != 0 && task.wake_tick() <= now {
                let task = self.sleep_queue.remove(i);
                task.set_wake_tick(0);
                task.set_state(TaskState::Ready);
                sched.scheduler.enqueue(task);
            } else {
                i += 1;
            }
        }
    }

    /// 检查当前任务的待处理信号并执行默认动作。
    pub(super) fn deliver_pending_signals(&mut self, sched: &mut PerCpuSched) {
        let task = match sched.current.as_ref() {
            Some(t) => t.clone(),
            None => return,
        };
        if task.is_idle() {
            return;
        }

        let pending = task.pending_signals();
        if pending == 0 {
            return;
        }
        let mask = SignalMask::from_bits_truncate(task.signal_mask());
        if let Some(sig) = first_deliverable(pending, mask) {
            task.clear_signal(1 << (sig as u32));
            match sig.default_action() {
                SignalAction::Terminate => {
                    let pid = task.pid();
                    let code = -(sig as i32);
                    task.set_exit_code(code);
                    task.set_state(TaskState::Exited);
                    log::info!(
                        "Task [pid={}] \"{}\" killed by {:?}",
                        task.pid(),
                        task.name(),
                        sig
                    );
                    self.wake_one(sched, ResourceId::ChildExit(pid));
                    self.wake_one(sched, ResourceId::ChildExit(0));
                }
                SignalAction::Stop => {
                    task.set_state(TaskState::Stopped);
                    log::info!(
                        "Task [pid={}] \"{}\" stopped by {:?}",
                        task.pid(),
                        task.name(),
                        sig
                    );
                }
                SignalAction::Ignore => {}
            }
        }
    }

    /// 将任务加入等待队列。
    pub(super) fn add_waiter(&mut self, resource: ResourceId, task: TaskRef) {
        self.wait_queues
            .entry(resource)
            .or_default()
            .push_back(task);
    }

    /// 注册任务到全局任务列表。
    pub(super) fn register_task(&mut self, task: TaskRef) {
        self.tasks.push(task);
    }

    /// 分配下一个 PID。
    pub(super) fn alloc_pid(&mut self) -> Pid {
        let pid = self.next_pid;
        self.next_pid += 1;
        pid
    }
}

pub(super) static TASK_TABLE: SpinLockIrq<TaskTable> =
    SpinLockIrq::new(TaskTable::EMPTY, "task_table", lock_level::TASK_TABLE);

impl TaskTable {
    /// 编译期空值——用于 SpinLockIrq 静态初始化。
    ///
    /// `init()` 中会通过 `*guard = TaskTable::new()` 替换为真正的实例。
    const EMPTY: Self = Self {
        tasks: Vec::new(),
        next_pid: 0,
        sleep_queue: Vec::new(),
        wait_queues: BTreeMap::new(),
    };
}
