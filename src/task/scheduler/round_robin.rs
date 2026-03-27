//! Round-Robin 调度器——时间片轮转。

use alloc::collections::VecDeque;

use crate::task::scheduler::Scheduler;
use crate::task::tcb::{TaskControlBlock, TaskRef};

/// 默认时间片（tick 数）
const DEFAULT_TIME_QUANTUM: u64 = 5;

/// Round-Robin 调度器
///
/// 在 FIFO 基础上增加时间片：当前任务运行满 `time_quantum` 个 tick 后
/// `task_tick()` 返回 true，触发抢占调度。
pub struct RoundRobinScheduler {
    /// 就绪队列
    queue: VecDeque<TaskRef>,
    /// 时间片大小（tick 数）
    time_quantum: u64,
    /// 当前任务已运行的 tick 数
    elapsed_ticks: u64,
}

impl RoundRobinScheduler {
    /// 创建 RR 调度器，使用默认时间片。
    pub fn new() -> Self {
        Self::with_quantum(DEFAULT_TIME_QUANTUM)
    }

    /// 创建 RR 调度器，指定时间片大小。
    pub fn with_quantum(time_quantum: u64) -> Self {
        Self {
            queue: VecDeque::new(),
            time_quantum,
            elapsed_ticks: 0,
        }
    }
}

impl Default for RoundRobinScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for RoundRobinScheduler {
    fn enqueue(&mut self, task: TaskRef) {
        self.queue.push_back(task);
    }

    fn pick_next(&mut self) -> Option<TaskRef> {
        self.elapsed_ticks = 0;
        self.queue.pop_front()
    }

    fn task_tick(&mut self, _current: &TaskControlBlock) -> bool {
        self.elapsed_ticks += 1;
        self.elapsed_ticks >= self.time_quantum
    }

    fn queue_size(&self) -> usize {
        self.queue.len()
    }

    fn steal_one(&mut self) -> Option<TaskRef> {
        self.queue.pop_back()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    fn make_task(pid: usize) -> TaskRef {
        Arc::new(TaskControlBlock::new_for_test(pid, "rr_test"))
    }

    #[test]
    fn basic_enqueue_pick() {
        let mut sched = RoundRobinScheduler::new();
        sched.enqueue(make_task(1));
        sched.enqueue(make_task(2));

        let t = sched.pick_next().expect("应能取到任务");
        assert_eq!(t.pid(), 1);
        let t = sched.pick_next().expect("应能取到任务");
        assert_eq!(t.pid(), 2);
        assert!(sched.pick_next().is_none());
    }

    #[test]
    fn time_slice_expiry() {
        let mut sched = RoundRobinScheduler::with_quantum(3);
        let task = make_task(1);

        // pick_next 重置 elapsed
        sched.enqueue(task.clone());
        let picked = sched.pick_next().expect("应能取到任务");

        // 前两次 tick 不触发抢占
        assert!(!sched.task_tick(&picked));
        assert!(!sched.task_tick(&picked));
        // 第三次 tick 触发抢占
        assert!(sched.task_tick(&picked));
    }

    #[test]
    fn pick_resets_elapsed() {
        let mut sched = RoundRobinScheduler::with_quantum(2);
        let t1 = make_task(1);
        let t2 = make_task(2);
        sched.enqueue(t1);
        sched.enqueue(t2);

        let picked = sched.pick_next().expect("取到 t1");
        assert!(!sched.task_tick(&picked)); // tick 1
        assert!(sched.task_tick(&picked)); // tick 2 → 到期

        // 重新 pick 应重置计数
        let picked = sched.pick_next().expect("取到 t2");
        assert!(!sched.task_tick(&picked)); // tick 1（重置）
    }

    #[test]
    fn steal_one_takes_from_back() {
        let mut sched = RoundRobinScheduler::new();
        sched.enqueue(make_task(1));
        sched.enqueue(make_task(2));
        sched.enqueue(make_task(3));
        let stolen = sched.steal_one().expect("应能偷到任务");
        assert_eq!(stolen.pid(), 3);
        // 剩余 [1, 2]
        assert_eq!(sched.pick_next().expect("").pid(), 1);
    }
}
