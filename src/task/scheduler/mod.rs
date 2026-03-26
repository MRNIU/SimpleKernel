//! 调度器子系统——定义调度器接口与各种调度算法实现。

pub mod fifo;

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

    fn queue_size(&self) -> usize;
    fn is_empty(&self) -> bool;
}
