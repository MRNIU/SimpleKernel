//! 任务管理子系统——任务控制块、状态机、调度器接口。

pub mod mutex;
pub mod resource_id;
pub mod scheduler;
pub mod signal;
pub mod state;
pub mod tcb;

#[cfg(not(test))]
mod sched;
#[cfg(not(test))]
mod task_table;

#[cfg(not(test))]
pub use sched::{current_task, release_sched_lock, schedule, yield_now};

// ─── 公开 API（仅非测试模式） ─────────────────────────────────────────────

#[cfg(not(test))]
mod api {
    use alloc::sync::Arc;

    use crate::arch::ArchOps;
    use crate::error::{ErrorCode, KResult};
    use crate::per_cpu;
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
    pub fn init() {
        let core_id = per_cpu::current_core_id();
        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        // SAFETY: 此时仅 BSP 核心运行，无并发访问
        unsafe {
            (&mut *PER_CPU_SCHED.get())[core_id] = Some(PerCpuSched {
                scheduler: SchedPolicy::default_policy(),
                current: Some(idle.clone()),
                idle: Some(idle),
            });
        }

        // 初始化任务表
        {
            let mut table = TASK_TABLE.lock();
            *table = TaskTable::new();
        }

        log::info!("TaskInit: idle task created for core {}", core_id);
    }

    /// 从核初始化。
    pub fn init_smp() {
        let core_id = per_cpu::current_core_id();
        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        // SAFETY: 每个核心仅写自己的 slot
        unsafe {
            (&mut *PER_CPU_SCHED.get())[core_id] = Some(PerCpuSched {
                scheduler: SchedPolicy::default_policy(),
                current: Some(idle.clone()),
                idle: Some(idle),
            });
        }

        log::info!("TaskInit: idle task created for core {}", core_id);
    }

    /// 创建内核线程并加入就绪队列（无父任务）。
    pub fn spawn_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> TaskRef {
        spawn_kernel_thread_with_parent(name, entry, arg, None)
    }

    /// 创建内核线程并加入就绪队列，指定父任务。
    pub fn spawn_kernel_thread_with_parent(
        name: &'static str,
        entry: fn(usize),
        arg: usize,
        parent_pid: Option<Pid>,
    ) -> TaskRef {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 持有本核调度锁
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

    // ─── sleep ──────────────────────────────────────────────────────────

    /// 挂起当前任务指定 tick 数。
    pub fn sleep(ticks: u64) {
        let task = super::sched::current_task();
        let now = crate::arch::Arch::get_current_tick();
        task.set_wake_tick(now + ticks);
        task.set_state(TaskState::Sleeping);

        {
            let mut table = TASK_TABLE.lock();
            table.sleep_queue.push(task);
        }

        schedule();
    }

    /// 挂起当前任务指定毫秒数。
    pub fn sleep_ms(ms: u64) {
        let tps = crate::arch::Arch::ticks_per_second();
        let ticks = (ms * tps + 999) / 1000;
        sleep(ticks);
    }

    // ─── block / wakeup ─────────────────────────────────────────────────

    /// 在指定资源上阻塞当前任务。
    pub fn block_on(resource: ResourceId) {
        let task = super::sched::current_task();
        task.set_state(TaskState::Blocked);

        {
            let mut table = TASK_TABLE.lock();
            table.add_waiter(resource, task);
        }

        schedule();
    }

    /// 唤醒在指定资源上阻塞的一个任务。
    pub fn wakeup_one(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };
        table.wake_one(sched, resource);
    }

    /// 唤醒在指定资源上阻塞的所有任务。
    pub fn wakeup_all(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let mut table = TASK_TABLE.lock();
        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };
        table.wake_all(sched, resource);
    }

    // ─── exit / wait ────────────────────────────────────────────────────

    /// 退出当前任务。
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
            // SAFETY: 持有本核调度锁
            let sched = unsafe { per_cpu_sched(core_id) };
            table.wake_one(sched, ResourceId::ChildExit(pid));
            table.wake_one(sched, ResourceId::ChildExit(0));
        }

        schedule();
        unreachable!("exited task was rescheduled");
    }

    /// 等待子进程退出。
    pub fn wait_child(child_pid: usize) -> KResult<(Pid, i32)> {
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
                    return Err(ErrorCode::TaskNoChildFound);
                }
            }

            block_on(ResourceId::ChildExit(child_pid));
        }
    }

    // ─── clone ──────────────────────────────────────────────────────────

    /// 克隆当前任务——创建子内核线程。
    pub fn clone_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> KResult<Pid> {
        let parent = super::sched::current_task();
        let child = spawn_kernel_thread_with_parent(name, entry, arg, Some(parent.pid()));
        Ok(child.pid())
    }

    // ─── signal ─────────────────────────────────────────────────────────

    /// 向指定任务发送信号。
    pub fn send_signal(pid: Pid, sig: Signal) -> KResult<()> {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let table = TASK_TABLE.lock();
        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };

        let task = table
            .tasks
            .iter()
            .find(|t| t.pid() == pid)
            .ok_or(ErrorCode::SignalTaskNotFound)?;

        task.raise_signal(1 << (sig as u32));

        if sig == Signal::SIGCONT && task.state() == TaskState::Stopped {
            task.set_state(TaskState::Ready);
            sched.scheduler.enqueue(task.clone());
        }

        Ok(())
    }
}

#[cfg(not(test))]
pub use api::*;
