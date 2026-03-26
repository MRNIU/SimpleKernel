//! 任务管理子系统——任务控制块、状态机、调度器接口。

pub mod scheduler;
pub mod state;
pub mod tcb;

// ─── TaskManager（仅非测试模式） ─────────────────────────────────────────────

#[cfg(not(test))]
mod manager {
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use core::cell::SyncUnsafeCell;

    use crate::config::MAX_CORE_COUNT;
    use crate::per_cpu;
    use crate::sync::SpinLock;
    use crate::task::scheduler::Scheduler;
    use crate::task::scheduler::fifo::FifoScheduler;
    use crate::task::state::TaskState;
    use crate::task::tcb::{CalleeSavedContextWrapper, Pid, TaskControlBlock, TaskRef};

    // ─── switch_to 外部声明 ──────────────────────────────────────────────

    unsafe extern "C" {
        fn switch_to(prev: *mut CalleeSavedContextWrapper, next: *const CalleeSavedContextWrapper);
    }

    // ─── 全局状态 ────────────────────────────────────────────────────────

    /// 调度锁 — 保护 SCHED_STATE 的访问。
    /// 使用 SpinLock<()> 因为 lock_raw/unlock_raw 需要跨越 switch_to。
    static SCHED_LOCK: SpinLock<()> = SpinLock::new((), "sched");

    /// 调度状态 — 由 SCHED_LOCK 保护
    struct SchedState {
        scheduler: FifoScheduler,
        tasks: Vec<TaskRef>,
        current: [Option<TaskRef>; MAX_CORE_COUNT],
        idle: [Option<TaskRef>; MAX_CORE_COUNT],
        next_pid: Pid,
    }

    static SCHED_STATE: SyncUnsafeCell<Option<SchedState>> = SyncUnsafeCell::new(None);

    /// 获取 SchedState 的可变引用。
    ///
    /// # Safety
    ///
    /// 调用者必须持有 SCHED_LOCK。
    unsafe fn sched_state() -> &'static mut SchedState {
        // SAFETY: 调用者持有 SCHED_LOCK，保证独占访问
        unsafe { &mut *SCHED_STATE.get() }
            .as_mut()
            .expect("sched_state: SCHED_STATE 未初始化")
    }

    // ─── 公开 API ────────────────────────────────────────────────────────

    /// BSP 初始化——创建 idle 任务并初始化调度状态。
    pub fn init() {
        let core_id = per_cpu::current_core_id();

        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        let mut current: [Option<TaskRef>; MAX_CORE_COUNT] = core::array::from_fn(|_| None);
        let mut idle_arr: [Option<TaskRef>; MAX_CORE_COUNT] = core::array::from_fn(|_| None);

        current[core_id] = Some(idle.clone());
        idle_arr[core_id] = Some(idle);

        // SAFETY: 此时仅 BSP 核心运行，无并发访问
        unsafe {
            *SCHED_STATE.get() = Some(SchedState {
                scheduler: FifoScheduler::new(),
                tasks: Vec::new(),
                current,
                idle: idle_arr,
                next_pid: 1,
            });
        }

        log::info!("TaskInit: idle task created for core {}", core_id);
    }

    /// 从核初始化——为当前核创建 idle 任务。
    pub fn init_smp() {
        let core_id = per_cpu::current_core_id();

        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };
        state.current[core_id] = Some(idle.clone());
        state.idle[core_id] = Some(idle);

        log::info!("TaskInit: idle task created for core {}", core_id);
    }

    /// 创建内核线程并加入就绪队列。
    pub fn spawn_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> TaskRef {
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };

        let pid = state.next_pid;
        state.next_pid += 1;

        let task = Arc::new(TaskControlBlock::new_kernel_thread(pid, name, entry, arg));
        state.tasks.push(task.clone());
        state.scheduler.enqueue(task.clone());

        log::info!("Task [pid={}] \"{}\" created", pid, name);
        task
    }

    /// 获取当前核上正在运行的任务。
    pub fn current_task() -> TaskRef {
        // SAFETY: current[core_id] 仅在持有 SCHED_LOCK 时修改；
        // 读取时无需锁——当前核的 current 不会被其他核修改
        let state = unsafe { &*SCHED_STATE.get() }
            .as_ref()
            .expect("current_task: SCHED_STATE 未初始化");
        let core_id = per_cpu::current_core_id();
        state.current[core_id]
            .as_ref()
            .expect("current_task: 当前核无运行任务")
            .clone()
    }

    /// 释放 schedule() 中通过 lock_raw 获取的调度锁。
    ///
    /// # Safety
    ///
    /// 仅供 `kernel_thread_bootstrap` 在新任务首次运行时调用。
    /// 调用者必须保证当前确实持有 SCHED_LOCK。
    pub unsafe fn release_sched_lock() {
        // SAFETY: 调用者保证持有 SCHED_LOCK
        unsafe { SCHED_LOCK.unlock_raw() };
    }

    /// 调度函数——选择下一个任务并执行上下文切换。
    pub fn schedule() {
        let core_id = per_cpu::current_core_id();

        // 1. 通过 lock_raw 获取调度锁（需跨越 switch_to）
        // SAFETY: 下方保证在每条路径上都调用 unlock_raw
        unsafe { SCHED_LOCK.lock_raw() };

        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };

        // 2. 取出当前任务
        let prev = state.current[core_id]
            .take()
            .expect("schedule: no current task");

        // 3. 若 prev 仍为 Running（主动让出），放回就绪队列
        if prev.state() == TaskState::Running {
            prev.set_state(TaskState::Ready);
            if !prev.is_idle() {
                state.scheduler.enqueue(prev.clone());
            }
        }

        // 4. 选取下一个任务（无就绪任务则回退到 idle）
        let next = state.scheduler.pick_next().unwrap_or_else(|| {
            state.idle[core_id]
                .as_ref()
                .expect("schedule: no idle task")
                .clone()
        });
        next.set_state(TaskState::Running);
        state.current[core_id] = Some(next.clone());

        // 5. 若为同一任务，直接释放锁返回
        if Arc::ptr_eq(&prev, &next) {
            // SAFETY: 持有 SCHED_LOCK
            unsafe { SCHED_LOCK.unlock_raw() };
            return;
        }

        // 6. 获取上下文原始指针
        // SAFETY: 持有 SCHED_LOCK，IRQ 关闭，无并发访问
        let prev_ctx = unsafe { prev.ctx_mut_ptr() };
        let next_ctx = unsafe { next.ctx_mut_ptr() };

        // 7. 在 switch 前释放 Arc 引用（防止析构函数在错误的栈上运行）
        drop(prev);
        drop(next);

        // 8. 上下文切换
        // SAFETY: prev_ctx 和 next_ctx 均有效，且由 SCHED_LOCK 保护
        unsafe { switch_to(prev_ctx, next_ctx) };

        // 9. 切换回来后释放调度锁
        // （新任务通过 kernel_thread_bootstrap 中的 release_sched_lock 释放）
        // SAFETY: 持有 SCHED_LOCK
        unsafe { SCHED_LOCK.unlock_raw() };
    }

    /// 主动让出 CPU。
    pub fn yield_now() {
        schedule();
    }

    /// 退出当前任务。
    pub fn exit(code: i32) -> ! {
        let task = current_task();
        task.set_exit_code(code);
        task.set_state(TaskState::Exited);
        log::info!(
            "Task [pid={}] \"{}\" exited with code {}",
            task.pid(),
            task.name(),
            code
        );
        schedule();
        unreachable!("exited task was rescheduled");
    }
}

#[cfg(not(test))]
pub use manager::*;
