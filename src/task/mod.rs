//! 任务管理子系统——任务控制块、状态机、调度器接口。

pub mod resource_id;
pub mod scheduler;
pub mod signal;
pub mod state;
pub mod tcb;

// ─── TaskManager（仅非测试模式） ─────────────────────────────────────────────

#[cfg(not(test))]
mod manager {
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use core::cell::SyncUnsafeCell;

    use crate::arch::{ArchOps, CalleeSavedContext};
    use crate::config::MAX_CORE_COUNT;
    use crate::error::{ErrorCode, KResult};
    use crate::per_cpu;
    use crate::sync::SpinLock;
    use crate::task::resource_id::ResourceId;
    use crate::task::scheduler::Scheduler;
    use crate::task::scheduler::fifo::FifoScheduler;
    use crate::task::signal::{Signal, SignalAction, SignalMask, first_deliverable};
    use crate::task::state::TaskState;
    use crate::task::tcb::{Pid, TaskControlBlock, TaskRef};

    // ─── switch_to 外部声明 ──────────────────────────────────────────────

    unsafe extern "C" {
        fn switch_to(prev: *mut CalleeSavedContext, next: *const CalleeSavedContext);
    }

    // ─── 全局状态 ────────────────────────────────────────────────────────

    /// 调度锁 — 保护 SCHED_STATE 的访问。
    /// 使用 SpinLock<()> 因为 lock_raw/unlock_raw 需要跨越 switch_to。
    static SCHED_LOCK: SpinLock<()> = SpinLock::new((), "sched");

    /// 等待队列条目
    struct WaitQueue {
        resource: ResourceId,
        waiters: Vec<TaskRef>,
    }

    /// 调度状态 — 由 SCHED_LOCK 保护
    struct SchedState {
        scheduler: FifoScheduler,
        tasks: Vec<TaskRef>,
        current: [Option<TaskRef>; MAX_CORE_COUNT],
        idle: [Option<TaskRef>; MAX_CORE_COUNT],
        next_pid: Pid,
        /// 睡眠队列——按 wake_tick 排列的睡眠任务
        sleep_queue: Vec<TaskRef>,
        /// 等待队列——按资源分组的阻塞任务
        wait_queues: Vec<WaitQueue>,
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

    // ─── 内部辅助 ────────────────────────────────────────────────────────

    /// 唤醒到期的睡眠任务——在 schedule() 入口处调用。
    ///
    /// 扫描 sleep_queue，将 wake_tick <= current_tick 的任务移入就绪队列。
    fn wake_expired_sleepers(state: &mut SchedState) {
        let now = crate::arch::Arch::get_current_tick();
        let mut i = 0;
        while i < state.sleep_queue.len() {
            let task = &state.sleep_queue[i];
            if task.wake_tick() != 0 && task.wake_tick() <= now {
                let task = state.sleep_queue.remove(i);
                task.set_wake_tick(0);
                task.set_state(TaskState::Ready);
                state.scheduler.enqueue(task);
            } else {
                i += 1;
            }
        }
    }

    /// 唤醒等待指定资源的一个任务——内部版本，调用者已持有 SCHED_LOCK。
    fn wake_blocked_on(state: &mut SchedState, resource: ResourceId) {
        if let Some(wq) = state
            .wait_queues
            .iter_mut()
            .find(|wq| wq.resource == resource)
        {
            if !wq.waiters.is_empty() {
                let task = wq.waiters.remove(0);
                task.set_state(TaskState::Ready);
                state.scheduler.enqueue(task);
            }
        }
        state.wait_queues.retain(|wq| !wq.waiters.is_empty());
    }

    /// 检查当前任务的待处理信号并执行默认动作。
    ///
    /// 在 schedule() 入口处调用（已持有 SCHED_LOCK），
    /// 处理 SIGKILL 等需要立即响应的信号。
    fn deliver_pending_signals(state: &mut SchedState, core_id: usize) {
        let task = match state.current[core_id].as_ref() {
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
            // 清除已投递的信号
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
                    // 唤醒等待此子进程退出的父任务
                    wake_blocked_on(state, ResourceId::ChildExit(pid));
                    wake_blocked_on(state, ResourceId::ChildExit(0));
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
                sleep_queue: Vec::new(),
                wait_queues: Vec::new(),
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
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };

        let pid = state.next_pid;
        state.next_pid += 1;

        let task = Arc::new(TaskControlBlock::new_kernel_thread(
            pid, name, entry, arg, parent_pid,
        ));
        state.tasks.push(task.clone());
        state.scheduler.enqueue(task.clone());

        log::info!(
            "Task [pid={}] \"{}\" created (parent={:?})",
            pid,
            name,
            parent_pid
        );
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

    /// 按 PID 查找任务。
    pub fn find_task(pid: Pid) -> Option<TaskRef> {
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };
        state.tasks.iter().find(|t| t.pid() == pid).cloned()
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

        // 1a. 唤醒到期的睡眠任务
        wake_expired_sleepers(state);

        // 1b. 投递信号
        deliver_pending_signals(state, core_id);

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
        // Sleeping/Blocked/Exited/Stopped 的任务不放回队列——
        // 已由 sleep/block_on/exit 在调用 schedule() 前设置好状态

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

    // ─── sleep ──────────────────────────────────────────────────────────

    /// 挂起当前任务指定 tick 数。
    ///
    /// 设置 wake_tick = current_tick + ticks，将任务移入 sleep_queue，
    /// 状态变为 Sleeping，然后调用 schedule()。
    pub fn sleep(ticks: u64) {
        let task = current_task();
        let now = crate::arch::Arch::get_current_tick();
        task.set_wake_tick(now + ticks);
        task.set_state(TaskState::Sleeping);

        // 将任务加入 sleep_queue（在 SCHED_LOCK 保护下）
        {
            let _guard = SCHED_LOCK.lock();
            // SAFETY: 持有 SCHED_LOCK
            let state = unsafe { sched_state() };
            state.sleep_queue.push(task);
        }

        schedule();
    }

    /// 挂起当前任务指定毫秒数。
    pub fn sleep_ms(ms: u64) {
        let tps = crate::arch::Arch::ticks_per_second();
        let ticks = (ms * tps + 999) / 1000; // 向上取整
        sleep(ticks);
    }

    // ─── block / wakeup ─────────────────────────────────────────────────

    /// 在指定资源上阻塞当前任务。
    ///
    /// 状态变为 Blocked，加入对应资源的等待队列，调用 schedule()。
    pub fn block_on(resource: ResourceId) {
        let task = current_task();
        task.set_state(TaskState::Blocked);

        {
            let _guard = SCHED_LOCK.lock();
            // SAFETY: 持有 SCHED_LOCK
            let state = unsafe { sched_state() };
            if let Some(wq) = state
                .wait_queues
                .iter_mut()
                .find(|wq| wq.resource == resource)
            {
                wq.waiters.push(task);
            } else {
                state.wait_queues.push(WaitQueue {
                    resource,
                    waiters: alloc::vec![task],
                });
            }
        }

        schedule();
    }

    /// 唤醒在指定资源上阻塞的一个任务（FIFO 顺序）。
    pub fn wakeup_one(resource: ResourceId) {
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };
        wake_blocked_on(state, resource);
    }

    /// 唤醒在指定资源上阻塞的所有任务。
    pub fn wakeup_all(resource: ResourceId) {
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };
        let wq = match state
            .wait_queues
            .iter_mut()
            .find(|wq| wq.resource == resource)
        {
            Some(wq) => wq,
            None => return,
        };
        for task in wq.waiters.drain(..) {
            task.set_state(TaskState::Ready);
            state.scheduler.enqueue(task);
        }
        state.wait_queues.retain(|wq| !wq.waiters.is_empty());
    }

    // ─── exit / wait ────────────────────────────────────────────────────

    /// 退出当前任务——设置退出码，进入 Exited（僵尸）状态。
    ///
    /// 唤醒等待在 `ChildExit(pid)` 或 `ChildExit(0)`（任意子进程）上的父任务。
    pub fn exit(code: i32) -> ! {
        let task = current_task();
        let pid = task.pid();
        task.set_exit_code(code);
        task.set_state(TaskState::Exited);
        log::info!(
            "Task [pid={}] \"{}\" exited with code {}",
            pid,
            task.name(),
            code
        );

        // 唤醒等待此子进程的父任务
        wakeup_one(ResourceId::ChildExit(pid));
        // 也唤醒 wait(-1) 等待任意子进程的父任务
        wakeup_one(ResourceId::ChildExit(0));

        schedule();
        unreachable!("exited task was rescheduled");
    }

    /// 等待子进程退出——阻塞直到指定子进程（或任意子进程）退出。
    ///
    /// # 参数
    /// - `child_pid`：指定子进程 PID，0 表示任意子进程
    ///
    /// # 返回值
    /// `Ok((pid, exit_code))`，或 `Err(TaskNoChildFound)` 若无匹配子进程
    pub fn wait_child(child_pid: usize) -> KResult<(Pid, i32)> {
        loop {
            // 查找匹配的已退出子进程
            {
                let _guard = SCHED_LOCK.lock();
                // SAFETY: 持有 SCHED_LOCK
                let state = unsafe { sched_state() };
                let caller_pid = {
                    let core_id = per_cpu::current_core_id();
                    state.current[core_id]
                        .as_ref()
                        .expect("wait_child: no current task")
                        .pid()
                };

                // 查找已退出的子进程
                let found = state.tasks.iter().find(|t| {
                    t.parent_pid() == Some(caller_pid)
                        && t.state() == TaskState::Exited
                        && (child_pid == 0 || t.pid() == child_pid)
                });

                if let Some(child) = found {
                    let pid = child.pid();
                    let code = child.exit_code();
                    // 从任务列表中移除（回收僵尸）
                    state.tasks.retain(|t| t.pid() != pid);
                    return Ok((pid, code));
                }

                // 检查是否存在匹配的子进程（可能还在运行）
                let has_children = state.tasks.iter().any(|t| {
                    t.parent_pid() == Some(caller_pid) && (child_pid == 0 || t.pid() == child_pid)
                });
                if !has_children {
                    return Err(ErrorCode::TaskNoChildFound);
                }
            }

            // 子进程存在但未退出——阻塞等待
            block_on(ResourceId::ChildExit(child_pid));
        }
    }

    // ─── clone ──────────────────────────────────────────────────────────

    /// 克隆当前任务——创建子内核线程（P5b 简化版，不复制地址空间）。
    ///
    /// # 参数
    /// - `name`：子任务名称
    /// - `entry`：入口函数
    /// - `arg`：入口函数参数
    ///
    /// # 返回值
    /// 子任务的 PID
    pub fn clone_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> KResult<Pid> {
        let parent = current_task();
        let child = spawn_kernel_thread_with_parent(name, entry, arg, Some(parent.pid()));
        Ok(child.pid())
    }

    // ─── signal ─────────────────────────────────────────────────────────

    /// 向指定任务发送信号。
    pub fn send_signal(pid: Pid, sig: Signal) -> KResult<()> {
        let _guard = SCHED_LOCK.lock();
        // SAFETY: 持有 SCHED_LOCK
        let state = unsafe { sched_state() };

        let task = state
            .tasks
            .iter()
            .find(|t| t.pid() == pid)
            .ok_or(ErrorCode::SignalTaskNotFound)?;

        task.raise_signal(1 << (sig as u32));

        // 如果任务被停止且收到 SIGCONT，唤醒它
        if sig == Signal::SIGCONT && task.state() == TaskState::Stopped {
            task.set_state(TaskState::Ready);
            state.scheduler.enqueue(task.clone());
        }

        Ok(())
    }

    // ─── mutex ──────────────────────────────────────────────────────────

    /// 内核阻塞互斥锁
    ///
    /// 竞争时任务进入 Blocked 状态（让出 CPU），而非自旋等待。
    /// 适用于可能长时间持有的锁。
    pub struct KMutex {
        /// 互斥锁 ID（用于 ResourceId）
        id: u64,
        /// 当前持有者 PID（NO_OWNER 表示未持有）
        owner: core::sync::atomic::AtomicUsize,
    }

    const KMUTEX_NO_OWNER: usize = usize::MAX;

    /// KMutex ID 计数器
    static NEXT_MUTEX_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

    impl KMutex {
        /// 创建新的阻塞互斥锁。
        pub fn new() -> Self {
            Self {
                id: NEXT_MUTEX_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
                owner: core::sync::atomic::AtomicUsize::new(KMUTEX_NO_OWNER),
            }
        }

        /// 获取锁——如果锁已被持有，当前任务进入 Blocked 状态。
        pub fn lock(&self) {
            loop {
                let current_pid = current_task().pid();
                // 尝试 CAS 获取锁
                match self.owner.compare_exchange(
                    KMUTEX_NO_OWNER,
                    current_pid,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                ) {
                    Ok(_) => return, // 获取成功
                    Err(owner) => {
                        if owner == current_pid {
                            panic!("KMutex: recursive lock by pid={}", current_pid);
                        }
                        // 锁被其他任务持有——阻塞等待
                        block_on(ResourceId::Mutex(self.id));
                    }
                }
            }
        }

        /// 释放锁——唤醒一个等待任务。
        pub fn unlock(&self) {
            self.owner
                .store(KMUTEX_NO_OWNER, core::sync::atomic::Ordering::Release);
            wakeup_one(ResourceId::Mutex(self.id));
        }
    }
}

#[cfg(not(test))]
pub use manager::*;
