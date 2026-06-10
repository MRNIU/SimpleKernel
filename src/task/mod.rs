// Copyright The SimpleKernel Contributors

//! 任务管理子系统——任务控制块、状态机、调度器接口。

use core::fmt;

pub mod mutex;
pub mod resource_id;
pub mod scheduler;
pub mod signal;
pub mod state;
pub mod tcb;

/// 任务子系统错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskError {
    /// 找不到子进程
    NoChildFound,
    /// 找不到目标任务（信号发送）
    TaskNotFound,
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for TaskError {}

mod sched;
mod task_table;

// 引导 + 基础设施（保持 pub）
pub use sched::{bootstrap_enable_irq, current_task, preempt_after_irq, schedule, timer_tick};

// syscall 网关后的实现（pub(crate)——外部调用者通过 syscall:: 进入）
pub(crate) use sched::yield_now;
mod api {
    use alloc::sync::Arc;

    use super::TaskError;
    use crate::task::resource_id::ResourceId;
    use crate::task::sched::{
        PER_CPU_SCHED, PER_CPU_SCHED_LOCK, PerCpuSched, per_cpu_sched, schedule,
    };
    use crate::task::scheduler::SchedPolicy;
    use crate::task::scheduler::Scheduler;
    use crate::task::signal::Signal;
    use crate::task::state::TaskState;
    use crate::task::task_table::{TASK_TABLE, TaskTable};
    use crate::task::tcb::{Pid, TaskControlBlock, TaskRef};

    /// BSP 初始化。
    ///
    /// # Panics
    /// 当前核心 ID 超出 `config::MAX_CORE_COUNT` 时 panic。
    pub fn init() {
        let core_id = per_cpu::current_core_id();
        assert!(
            core_id < config::MAX_CORE_COUNT,
            "TaskInit: BSP core_id={} 超出 MAX_CORE_COUNT={}",
            core_id,
            config::MAX_CORE_COUNT
        );
        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        // SAFETY: 此时仅 BSP 核心运行，尚未启动从核和任务调度，不存在并发访问调度数组。
        unsafe {
            let mut sched = PerCpuSched::new(SchedPolicy::default_policy());
            sched.current = Some(idle.clone());
            sched.idle = Some(idle);
            (&mut *PER_CPU_SCHED.get())[core_id] = Some(sched);
        }

        // 初始化任务表
        {
            let mut table = TASK_TABLE.lock();
            *table = TaskTable::new();
        }

        log::info!("TaskInit: idle task created for core {}", core_id);
    }

    /// 从核初始化。
    ///
    /// # Panics
    /// 当前核心 ID 超出 `config::MAX_CORE_COUNT` 时 panic。
    pub fn init_smp() {
        let core_id = per_cpu::current_core_id();
        assert!(
            core_id < config::MAX_CORE_COUNT,
            "TaskInitSMP: core_id={} 超出 MAX_CORE_COUNT={}",
            core_id,
            config::MAX_CORE_COUNT
        );
        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        // SAFETY: 每个从核只写入以自身 `core_id` 索引的 slot；主核已完成全局数组初始化。
        unsafe {
            let mut sched = PerCpuSched::new(SchedPolicy::default_policy());
            sched.current = Some(idle.clone());
            sched.idle = Some(idle);
            (&mut *PER_CPU_SCHED.get())[core_id] = Some(sched);
        }

        log::info!("TaskInit: idle task created for core {}", core_id);
    }
    /// 内核线程构建器——借鉴 Theseus OS 的 `TaskBuilder` 模式。
    ///
    /// 通过链式调用设置任务参数，最后调用 `spawn()` 创建任务。
    /// 比直接函数调用更可扩展（新增参数无需改变现有调用点）。
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let task = TaskBuilder::new(my_entry, 0)
    ///     .name("worker")
    ///     .parent(parent_pid)
    ///     .spawn();
    /// ```
    pub struct TaskBuilder {
        entry: fn(usize),
        arg: usize,
        name: &'static str,
        parent_pid: Option<Pid>,
    }

    impl TaskBuilder {
        /// 创建构建器，指定入口函数和参数。
        #[must_use]
        pub fn new(entry: fn(usize), arg: usize) -> Self {
            Self {
                entry,
                arg,
                name: "<unnamed>",
                parent_pid: None,
            }
        }

        /// 设置任务名称。
        #[must_use]
        pub fn name(mut self, name: &'static str) -> Self {
            self.name = name;
            self
        }

        /// 设置父任务 PID。
        #[must_use]
        pub fn parent(mut self, pid: Pid) -> Self {
            self.parent_pid = Some(pid);
            self
        }

        /// 创建任务并加入就绪队列。
        ///
        /// # Panics
        ///
        /// 当前核心调度状态尚未初始化，或当前核心 ID 超出调度数组范围时 panic。
        pub fn spawn(self) -> TaskRef {
            do_spawn(self.name, self.entry, self.arg, self.parent_pid)
        }
    }

    /// 创建内核线程并加入就绪队列（无父任务）。
    ///
    /// # Panics
    ///
    /// 当前核心调度状态尚未初始化，或当前核心 ID 超出调度数组范围时 panic。
    pub fn spawn_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> TaskRef {
        do_spawn(name, entry, arg, None)
    }

    /// 创建内核线程并加入就绪队列，指定父任务。
    ///
    /// # Panics
    ///
    /// 当前核心调度状态尚未初始化，或当前核心 ID 超出调度数组范围时 panic。
    pub fn spawn_kernel_thread_with_parent(
        name: &'static str,
        entry: fn(usize),
        arg: usize,
        parent_pid: Option<Pid>,
    ) -> TaskRef {
        do_spawn(name, entry, arg, parent_pid)
    }

    /// 内部实现——供 TaskBuilder 和 spawn_kernel_thread 共用。
    fn do_spawn(
        name: &'static str,
        entry: fn(usize),
        arg: usize,
        parent_pid: Option<Pid>,
    ) -> TaskRef {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 已持有本核调度锁，该核心调度状态不会被其他路径并发修改。
        let sched = unsafe { per_cpu_sched(core_id) };

        let pid = table.alloc_pid();

        let task = Arc::new(TaskControlBlock::new_kernel_thread(
            pid, name, entry, arg, parent_pid,
        ));
        table.register_task(task.clone());
        sched.scheduler.enqueue(task.clone());

        log::info!(
            "Task [pid={}] \"{}\" created (parent={:?})",
            pid,
            name,
            parent_pid
        );
        task
    }

    /// 按 PID 查找任务。
    pub fn find_task(pid: Pid) -> Option<TaskRef> {
        let table = TASK_TABLE.lock();
        table.tasks.iter().find(|t| t.pid() == pid).cloned()
    }
    /// 挂起当前任务指定 tick 数。
    ///
    /// 必须在持有 TASK_TABLE 锁时原子完成「加入睡眠队列 + 设置状态」，
    /// 否则另一核心的 `wake_expired_sleepers()` 可能在窗口期内遗漏该任务。
    ///
    /// # Panics
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn sleep(ticks: u64) {
        let task = super::sched::current_task();
        let now = global_tick::current();
        task.set_wake_tick(now + ticks);

        {
            let mut table = TASK_TABLE.lock();
            table.sleep_queue.push(task.clone());
            task.set_state(TaskState::Sleeping);
        }

        schedule();
    }

    /// 挂起当前任务指定毫秒数。
    ///
    /// # Panics
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn sleep_ms(ms: u64) {
        let ticks = (ms * config::TIMER_FREQ_HZ).div_ceil(1000);
        sleep(ticks);
    }
    /// 在指定资源上阻塞当前任务。
    ///
    /// 必须在持有 TASK_TABLE 锁时原子完成「加入等待队列 + 设置状态」，
    /// 否则另一核心的 `wakeup_one()` 可能在窗口期内遗漏该任务。
    ///
    /// # Panics
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn block_on(resource: ResourceId) {
        let task = super::sched::current_task();

        {
            let mut table = TASK_TABLE.lock();
            table.add_waiter(resource, task.clone());
            task.set_state(TaskState::Blocked);
        }

        schedule();
    }

    /// 唤醒在指定资源上阻塞的一个任务。
    ///
    /// # Panics
    /// 当前核心调度状态未初始化时 panic。
    pub fn wakeup_one(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 已持有本核调度锁，该核心调度状态不会被其他路径并发修改。
        let sched = unsafe { per_cpu_sched(core_id) };
        table.wake_one(sched, resource);
    }

    /// 唤醒在指定资源上阻塞的所有任务。
    ///
    /// # Panics
    ///
    /// 当前核心调度状态未初始化时 panic。
    #[expect(dead_code, reason = "广播唤醒入口保留给后续条件变量和批量资源通知")]
    pub fn wakeup_all(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 已持有本核调度锁，该核心调度状态不会被其他路径并发修改。
        let sched = unsafe { per_cpu_sched(core_id) };
        table.wake_all(sched, resource);
    }
    /// 退出当前任务。
    ///
    /// # Panics
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn exit(code: i32) -> ! {
        let task = super::sched::current_task();
        let pid = task.pid();
        task.set_exit_code(code);
        task.set_state(TaskState::Exited);
        log::info!(
            "Task [pid={}] \"{}\" exited with code {}",
            pid,
            task.name(),
            code
        );

        // 一次性获取两把锁，合并两次 wakeup
        {
            let core_id = per_cpu::current_core_id();
            let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
            let mut table = TASK_TABLE.lock();
            // SAFETY: 已持有本核调度锁，该核心调度状态不会被其他路径并发修改。
            let sched = unsafe { per_cpu_sched(core_id) };
            table.wake_one(sched, ResourceId::ChildExit(pid));
            table.wake_one(sched, ResourceId::ChildExit(0));
        }

        schedule();
        unreachable!("已退出任务被重新调度: pid={pid}, exit_code={code}");
    }

    /// 等待子进程退出。
    ///
    /// # Errors
    ///
    /// 当前任务没有匹配的子任务时返回 [`TaskError::NoChildFound`]。
    ///
    /// # Panics
    ///
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn wait_child(child_pid: usize) -> Result<(Pid, i32), TaskError> {
        loop {
            {
                let mut table = TASK_TABLE.lock();
                let caller_pid = super::sched::current_task().pid();

                let found = table.tasks.iter().find(|t| {
                    t.parent_pid() == Some(caller_pid)
                        && t.state() == TaskState::Exited
                        && (child_pid == 0 || t.pid() == child_pid)
                });

                if let Some(child) = found {
                    let pid = child.pid();
                    let code = child.exit_code();
                    table.tasks.retain(|t| t.pid() != pid);
                    return Ok((pid, code));
                }

                let has_children = table.tasks.iter().any(|t| {
                    t.parent_pid() == Some(caller_pid) && (child_pid == 0 || t.pid() == child_pid)
                });
                if !has_children {
                    return Err(TaskError::NoChildFound);
                }
            }

            block_on(ResourceId::ChildExit(child_pid));
        }
    }
    /// 克隆当前任务——创建子内核线程。
    ///
    /// # Errors
    ///
    /// 当前实现的内核线程 spawn 路径直接返回新任务；保留 `Result` 用于后续向上传递
    /// 任务表容量或栈分配失败。
    ///
    /// # Panics
    ///
    /// 当前任务或调度器状态未初始化时 panic。
    pub fn clone_kernel_thread(
        name: &'static str,
        entry: fn(usize),
        arg: usize,
    ) -> Result<Pid, TaskError> {
        let parent = super::sched::current_task();
        let child = spawn_kernel_thread_with_parent(name, entry, arg, Some(parent.pid()));
        Ok(child.pid())
    }
    /// 向指定任务发送信号。
    ///
    /// # Errors
    ///
    /// 目标任务不存在时返回 [`TaskError::TaskNotFound`]。
    ///
    /// # Panics
    ///
    /// 当前核心调度状态未初始化时 panic。
    pub fn send_signal(pid: Pid, sig: Signal) -> Result<(), TaskError> {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let table = TASK_TABLE.lock();
        // SAFETY: 已持有本核调度锁，该核心调度状态不会被其他路径并发修改。
        let sched = unsafe { per_cpu_sched(core_id) };

        let task = table
            .tasks
            .iter()
            .find(|t| t.pid() == pid)
            .ok_or(TaskError::TaskNotFound)?;

        task.raise_signal(1 << (sig as u32));

        if sig == Signal::SIGCONT && task.state() == TaskState::Stopped {
            task.set_state(TaskState::Ready);
            sched.scheduler.enqueue(task.clone());
        }

        Ok(())
    }
}

// 引导 + 公开 API（保持 pub）
pub use api::{
    TaskBuilder, find_task, init, init_smp, spawn_kernel_thread, spawn_kernel_thread_with_parent,
};

// syscall 网关后的实现（pub(crate)——外部调用者通过 syscall:: 进入）
pub(crate) use api::{
    block_on, clone_kernel_thread, exit, send_signal, sleep_ms, wait_child, wakeup_one,
};
