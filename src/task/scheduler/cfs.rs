//! CFS（完全公平调度器）——基于虚拟运行时间的公平调度。

#[cfg(not(test))]
use alloc::vec::Vec;
#[cfg(test)]
use std::vec::Vec;

use crate::task::scheduler::Scheduler;
use crate::task::tcb::{TaskControlBlock, TaskRef};

/// CFS 队列中的条目
struct CfsEntry {
    /// 虚拟运行时间（越小越优先）
    vruntime: i64,
    /// 任务引用
    task: TaskRef,
}

/// CFS 调度器
///
/// 维护一个按 `vruntime` 排序的就绪队列。每次 `pick_next` 取最小 vruntime 的任务，
/// `task_tick` 递增当前任务的 vruntime 并判断是否需要抢占。
///
/// 简化实现：不区分权重/nice 值，所有任务权重相同。
pub struct CfsScheduler {
    /// 就绪队列（按 vruntime 升序排列）
    queue: Vec<CfsEntry>,
    /// 全局最小 vruntime（防止新任务饥饿）
    min_vruntime: i64,
    /// 当前运行任务的 vruntime（用于 task_tick 判断）
    current_vruntime: i64,
}

impl CfsScheduler {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            min_vruntime: 0,
            current_vruntime: 0,
        }
    }
}

impl Default for CfsScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for CfsScheduler {
    fn enqueue(&mut self, task: TaskRef) {
        // 新任务的 vruntime 设为当前 min_vruntime，避免饥饿也避免不公平抢占
        let vruntime = self.min_vruntime;
        // partition_point 返回第一个 > vruntime 的位置，保证同 vruntime 内 FIFO 顺序
        let pos = self.queue.partition_point(|e| e.vruntime <= vruntime);
        self.queue.insert(pos, CfsEntry { vruntime, task });
    }

    fn put_prev(&mut self, task: TaskRef) {
        // 保留运行期间累积的 vruntime（至少为 min_vruntime，防止倒退）
        let vruntime = self.current_vruntime.max(self.min_vruntime);
        let pos = self.queue.partition_point(|e| e.vruntime <= vruntime);
        self.queue.insert(pos, CfsEntry { vruntime, task });
    }

    fn pick_next(&mut self) -> Option<TaskRef> {
        if self.queue.is_empty() {
            return None;
        }
        let entry = self.queue.remove(0);
        self.current_vruntime = entry.vruntime;
        Some(entry.task)
    }

    fn task_tick(&mut self, _current: &TaskControlBlock) -> bool {
        // 递增当前任务的 vruntime（简化：每 tick +1）
        self.current_vruntime += 1;

        // 更新全局 min_vruntime
        if let Some(front) = self.queue.first() {
            self.min_vruntime = front.vruntime;
            // 如果当前任务的 vruntime 超过队首，需要抢占
            self.current_vruntime > front.vruntime
        } else {
            self.min_vruntime = self.current_vruntime;
            false
        }
    }

    fn queue_size(&self) -> usize {
        self.queue.len()
    }

    fn steal_one(&mut self) -> Option<TaskRef> {
        self.queue.pop().map(|e| e.task)
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_task(pid: usize) -> TaskRef {
        Arc::new(TaskControlBlock::new_for_test(pid, "cfs_test"))
    }

    #[test]
    fn basic_fairness() {
        let mut sched = CfsScheduler::new();
        sched.enqueue(make_task(1));
        sched.enqueue(make_task(2));
        sched.enqueue(make_task(3));

        // 首次取出——都是 vruntime=0，按入队顺序
        let t = sched.pick_next().expect("应有任务");
        assert_eq!(t.pid(), 1);
    }

    #[test]
    fn preemption_when_vruntime_exceeds() {
        let mut sched = CfsScheduler::new();
        let t1 = make_task(1);
        let t2 = make_task(2);
        sched.enqueue(t1.clone());
        sched.enqueue(t2);

        // pick t1 (vruntime=0)
        let picked = sched.pick_next().expect("取到 t1");
        assert_eq!(picked.pid(), 1);

        // 此时队列中 t2 的 vruntime=0
        // tick t1: vruntime 变为 1 > t2 的 0 → 应抢占
        assert!(sched.task_tick(&picked));
    }

    #[test]
    fn no_preemption_when_alone() {
        let mut sched = CfsScheduler::new();
        sched.enqueue(make_task(1));

        let picked = sched.pick_next().expect("取到任务");
        // 队列空，不需要抢占
        assert!(!sched.task_tick(&picked));
        assert!(!sched.task_tick(&picked));
    }

    #[test]
    fn put_prev_preserves_vruntime() {
        let mut sched = CfsScheduler::new();
        let t1 = make_task(1);
        let t2 = make_task(2);
        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());

        // 运行 t1 两个 tick
        let picked = sched.pick_next().expect("t1");
        sched.task_tick(&picked); // vruntime=1
        sched.task_tick(&picked); // vruntime=2

        // 通过 put_prev 放回 t1（保留 vruntime=2）
        sched.put_prev(t1);

        // t2(vruntime=0) 应先被选中（因为 t1 的 vruntime=2 更大）
        let next = sched.pick_next().expect("t2");
        assert_eq!(next.pid(), 2);

        // 再选应是 t1
        let next = sched.pick_next().expect("t1");
        assert_eq!(next.pid(), 1);
    }

    #[test]
    fn empty_queue() {
        let mut sched = CfsScheduler::new();
        assert!(sched.is_empty());
        assert!(sched.pick_next().is_none());
    }
}
