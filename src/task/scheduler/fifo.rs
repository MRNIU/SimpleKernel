//! FIFO 调度器——先进先出，最简单的调度算法。

use crate::compat::VecDeque;

use crate::task::scheduler::Scheduler;
use crate::task::tcb::{TaskControlBlock, TaskRef};

/// FIFO 调度器
///
/// 按入队顺序依次运行任务，不抢占，不考虑优先级。
pub struct FifoScheduler {
    /// 就绪队列
    queue: VecDeque<TaskRef>,
}

impl FifoScheduler {
    /// 创建一个空的 FIFO 调度器。
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Default for FifoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for FifoScheduler {
    /// 将就绪任务追加到队列尾部。
    fn enqueue(&mut self, task: TaskRef) {
        self.queue.push_back(task);
    }

    /// 从队列头部取出下一个任务。
    fn pick_next(&mut self) -> Option<TaskRef> {
        self.queue.pop_front()
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
    use crate::compat::Arc;

    fn make_task(pid: usize) -> TaskRef {
        Arc::new(TaskControlBlock::new_for_test(pid, "test"))
    }

    #[test]
    fn enqueue_and_pick() {
        let mut sched = FifoScheduler::new();
        let t1 = make_task(1);
        let t2 = make_task(2);
        sched.enqueue(t1);
        sched.enqueue(t2);
        let next = sched.pick_next().expect("应能取到第一个任务");
        assert_eq!(next.pid(), 1);
        let next = sched.pick_next().expect("应能取到第二个任务");
        assert_eq!(next.pid(), 2);
        assert!(sched.pick_next().is_none());
    }

    #[test]
    fn is_empty_and_size() {
        let mut sched = FifoScheduler::new();
        assert!(sched.is_empty());
        sched.enqueue(make_task(1));
        assert!(!sched.is_empty());
        assert_eq!(sched.queue_size(), 1);
    }
}
