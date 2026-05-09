// Copyright The SimpleKernel Contributors

//! 调度器子系统——定义调度器接口与各种调度算法实现。

pub mod cfs;
pub mod fifo;
pub mod round_robin;

use crate::task::tcb::TaskRef;

/// 调度器接口
///
/// 实现此 trait 以添加新的调度算法。
/// P5a 仅实现 FIFO；P5b 添加 RoundRobin 和 CFS。
pub trait Scheduler: Send {
    /// 将就绪任务加入队列
    fn enqueue(&mut self, task: TaskRef);

    /// 从队列中选取下一个运行的任务（移出队列）
    fn pick_next(&mut self) -> Option<TaskRef>;

    /// 定时器 tick 回调 — 返回 true 表示当前任务时间片耗尽需调度
    fn task_tick(&mut self, _current: &crate::task::tcb::TaskControlBlock) -> bool {
        false
    }

    /// 将刚让出 CPU 的任务放回队列——允许调度器保留运行时统计。
    ///
    /// 默认实现等同于 `enqueue`。CFS 会保留累积的 vruntime。
    fn put_prev(&mut self, task: TaskRef) {
        self.enqueue(task);
    }

    /// 获取当前运行任务的调度优先级快照（用于延迟入队）。
    ///
    /// CFS 返回 `current_vruntime`；其他调度器返回 0（不使用此值）。
    fn snapshot_current_priority(&self) -> i64 {
        0
    }

    /// 使用保存的优先级快照将延迟的 prev 任务入队。
    ///
    /// CFS 使用 `priority` 作为 vruntime 插入排序；
    /// 默认实现忽略 `priority`，等同于 `enqueue`。
    fn enqueue_prev_deferred(&mut self, task: TaskRef, _priority: i64) {
        self.enqueue(task);
    }

    /// 从队列尾部窃取一个任务（用于跨核负载均衡）。
    ///
    /// 默认返回 None。支持窃取的调度器应覆盖此方法。
    fn steal_one(&mut self) -> Option<TaskRef> {
        None
    }

    fn queue_size(&self) -> usize;
    fn is_empty(&self) -> bool;
}

/// 调度策略枚举——静态分发，避免 `Box<dyn Scheduler>` 的堆分配开销。
pub enum SchedPolicy {
    Fifo(fifo::FifoScheduler),
    RoundRobin(round_robin::RoundRobinScheduler),
    Cfs(cfs::CfsScheduler),
}

impl SchedPolicy {
    /// 根据默认配置创建调度器。
    pub fn default_policy() -> Self {
        SchedPolicy::Fifo(fifo::FifoScheduler::new())
    }
}

/// 为 `SchedPolicy` 自动转发 `Scheduler` trait 的所有方法。
macro_rules! dispatch_scheduler {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            SchedPolicy::Fifo(s) => s.$method($($arg),*),
            SchedPolicy::RoundRobin(s) => s.$method($($arg),*),
            SchedPolicy::Cfs(s) => s.$method($($arg),*),
        }
    };
}

impl Scheduler for SchedPolicy {
    fn enqueue(&mut self, task: TaskRef) {
        dispatch_scheduler!(self, enqueue, task);
    }

    fn pick_next(&mut self) -> Option<TaskRef> {
        dispatch_scheduler!(self, pick_next)
    }

    fn task_tick(&mut self, current: &crate::task::tcb::TaskControlBlock) -> bool {
        dispatch_scheduler!(self, task_tick, current)
    }

    fn put_prev(&mut self, task: TaskRef) {
        dispatch_scheduler!(self, put_prev, task);
    }

    fn steal_one(&mut self) -> Option<TaskRef> {
        dispatch_scheduler!(self, steal_one)
    }

    fn snapshot_current_priority(&self) -> i64 {
        dispatch_scheduler!(self, snapshot_current_priority)
    }

    fn enqueue_prev_deferred(&mut self, task: TaskRef, priority: i64) {
        dispatch_scheduler!(self, enqueue_prev_deferred, task, priority);
    }

    fn queue_size(&self) -> usize {
        dispatch_scheduler!(self, queue_size)
    }

    fn is_empty(&self) -> bool {
        dispatch_scheduler!(self, is_empty)
    }
}
