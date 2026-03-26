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
    use crate::sync::spinlock::lock_level;
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

    // ─── Per-CPU 调度状态 ────────────────────────────────────────────────

    /// Per-CPU 调度锁数组——每个核心一把，保护对应核心的调度状态。
    ///
    /// 使用 `lock_raw()`/`unlock_raw()` 跨越 `switch_to`。
    /// 任务窃取时使用 `try_lock_raw_no_irq()` 获取其他核心的锁。
    static PER_CPU_SCHED_LOCK: [SpinLock<()>; MAX_CORE_COUNT] = [
        SpinLock::new_with_level((), "sched/0", lock_level::SCHED_LOCK),
        SpinLock::new_with_level((), "sched/1", lock_level::SCHED_LOCK),
        SpinLock::new_with_level((), "sched/2", lock_level::SCHED_LOCK),
        SpinLock::new_with_level((), "sched/3", lock_level::SCHED_LOCK),
    ];

    /// Per-CPU 调度状态（由对应的 PER_CPU_SCHED_LOCK[core_id] 保护）
    struct PerCpuSched {
        scheduler: FifoScheduler,
        current: Option<TaskRef>,
        idle: Option<TaskRef>,
    }

    static PER_CPU_SCHED: SyncUnsafeCell<[Option<PerCpuSched>; MAX_CORE_COUNT]> =
        SyncUnsafeCell::new([const { None }; MAX_CORE_COUNT]);

    /// # Safety
    ///
    /// 调用者必须持有 PER_CPU_SCHED_LOCK[core_id]。
    unsafe fn per_cpu_sched(core_id: usize) -> &'static mut PerCpuSched {
        // SAFETY: 调用者持有对应核心的调度锁
        let array = unsafe { &mut *PER_CPU_SCHED.get() };
        array[core_id].as_mut().expect("per_cpu_sched: 未初始化")
    }

    // ─── 全局任务表 ──────────────────────────────────────────────────────

    /// 全局任务表锁（级别 1，高于调度锁级别 0）
    static TASK_TABLE_LOCK: SpinLock<()> =
        SpinLock::new_with_level((), "task_table", lock_level::TASK_TABLE_LOCK);

    struct WaitQueue {
        resource: ResourceId,
        waiters: Vec<TaskRef>,
    }

    struct TaskTable {
        tasks: Vec<TaskRef>,
        next_pid: Pid,
        sleep_queue: Vec<TaskRef>,
        wait_queues: Vec<WaitQueue>,
    }

    static TASK_TABLE: SyncUnsafeCell<Option<TaskTable>> = SyncUnsafeCell::new(None);

    /// # Safety
    ///
    /// 调用者必须持有 TASK_TABLE_LOCK。
    unsafe fn task_table() -> &'static mut TaskTable {
        unsafe { &mut *TASK_TABLE.get() }
            .as_mut()
            .expect("task_table: 未初始化")
    }

    // ─── 内部辅助 ────────────────────────────────────────────────────────

    /// 唤醒等待指定资源的一个任务——加入指定核心的就绪队列。
    ///
    /// 调用者必须同时持有 per-CPU 调度锁和 TASK_TABLE_LOCK。
    fn wake_blocked_on_inner(table: &mut TaskTable, sched: &mut PerCpuSched, resource: ResourceId) {
        if let Some(wq) = table
            .wait_queues
            .iter_mut()
            .find(|wq| wq.resource == resource)
        {
            if !wq.waiters.is_empty() {
                let task = wq.waiters.remove(0);
                task.set_state(TaskState::Ready);
                sched.scheduler.enqueue(task);
            }
        }
        table.wait_queues.retain(|wq| !wq.waiters.is_empty());
    }

    /// 唤醒到期的睡眠任务。
    ///
    /// 调用者必须同时持有 per-CPU 调度锁和 TASK_TABLE_LOCK。
    fn wake_expired_sleepers(table: &mut TaskTable, sched: &mut PerCpuSched) {
        let now = crate::arch::Arch::get_current_tick();
        let mut i = 0;
        while i < table.sleep_queue.len() {
            let task = &table.sleep_queue[i];
            if task.wake_tick() != 0 && task.wake_tick() <= now {
                let task = table.sleep_queue.remove(i);
                task.set_wake_tick(0);
                task.set_state(TaskState::Ready);
                sched.scheduler.enqueue(task);
            } else {
                i += 1;
            }
        }
    }

    /// 检查当前任务的待处理信号并执行默认动作。
    ///
    /// 调用者必须同时持有 per-CPU 调度锁和 TASK_TABLE_LOCK。
    fn deliver_pending_signals(table: &mut TaskTable, sched: &mut PerCpuSched) {
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
                    wake_blocked_on_inner(table, sched, ResourceId::ChildExit(pid));
                    wake_blocked_on_inner(table, sched, ResourceId::ChildExit(0));
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

    /// 从其他核心窃取一个任务——当本核就绪队列为空时调用。
    ///
    /// 调用者已持有 PER_CPU_SCHED_LOCK[my_core]（通过 lock_raw）。
    fn try_steal(my_core: usize) -> Option<TaskRef> {
        let mut best_core = None;
        let mut best_size = 0usize;

        for core in 0..MAX_CORE_COUNT {
            if core == my_core {
                continue;
            }
            // SAFETY: 只读快照，不需要严格一致性
            let sched = match unsafe { &*PER_CPU_SCHED.get() }[core].as_ref() {
                Some(s) => s,
                None => continue,
            };
            let size = sched.scheduler.queue_size();
            if size > best_size {
                best_size = size;
                best_core = Some(core);
            }
        }

        let victim = best_core?;
        if best_size <= 1 {
            return None;
        }

        // SAFETY: 中断已被本核 lock_raw 禁用
        if !unsafe { PER_CPU_SCHED_LOCK[victim].try_lock_raw_no_irq() } {
            return None;
        }

        // SAFETY: 持有 victim 的调度锁
        let victim_sched = unsafe { per_cpu_sched(victim) };
        let stolen = victim_sched.scheduler.steal_one();

        // SAFETY: 释放 victim 的锁（不操作中断）
        unsafe { PER_CPU_SCHED_LOCK[victim].unlock_raw_no_irq() };

        if let Some(ref task) = stolen {
            log::info!(
                "Balance: core {} stole pid={} from core {}",
                my_core,
                task.pid(),
                victim
            );
        }

        stolen
    }

    // ─── 公开 API ────────────────────────────────────────────────────────

    /// BSP 初始化。
    pub fn init() {
        let core_id = per_cpu::current_core_id();
        let idle = Arc::new(TaskControlBlock::new_idle(0, core_id));

        // SAFETY: 此时仅 BSP 核心运行，无并发访问
        unsafe {
            (&mut *PER_CPU_SCHED.get())[core_id] = Some(PerCpuSched {
                scheduler: FifoScheduler::new(),
                current: Some(idle.clone()),
                idle: Some(idle),
            });
            *TASK_TABLE.get() = Some(TaskTable {
                tasks: Vec::new(),
                next_pid: 1,
                sleep_queue: Vec::new(),
                wait_queues: Vec::new(),
            });
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
                scheduler: FifoScheduler::new(),
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
        let _table_guard = TASK_TABLE_LOCK.lock();
        // SAFETY: 持有两把锁
        let table = unsafe { task_table() };
        let sched = unsafe { per_cpu_sched(core_id) };

        let pid = table.next_pid;
        table.next_pid += 1;

        let task = Arc::new(TaskControlBlock::new_kernel_thread(
            pid, name, entry, arg, parent_pid,
        ));
        table.tasks.push(task.clone());
        sched.scheduler.enqueue(task.clone());

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
        let core_id = per_cpu::current_core_id();
        // SAFETY: 本核 current 仅在持有本核调度锁时修改，其他核不会修改
        let sched = unsafe { &*PER_CPU_SCHED.get() }[core_id]
            .as_ref()
            .expect("current_task: 未初始化");
        sched
            .current
            .as_ref()
            .expect("current_task: 当前核无运行任务")
            .clone()
    }

    /// 按 PID 查找任务。
    pub fn find_task(pid: Pid) -> Option<TaskRef> {
        let _guard = TASK_TABLE_LOCK.lock();
        let table = unsafe { task_table() };
        table.tasks.iter().find(|t| t.pid() == pid).cloned()
    }

    /// 释放 schedule() 中通过 lock_raw 获取的本核调度锁。
    ///
    /// # Safety
    ///
    /// 仅供 `kernel_thread_bootstrap` 在新任务首次运行时调用。
    pub unsafe fn release_sched_lock() {
        let core_id = per_cpu::current_core_id();
        unsafe { PER_CPU_SCHED_LOCK[core_id].unlock_raw() };
    }

    /// 调度函数——选择下一个任务并执行上下文切换。
    pub fn schedule() {
        let core_id = per_cpu::current_core_id();

        // 1. 获取本核调度锁（lock_raw 跨越 switch_to）
        unsafe { PER_CPU_SCHED_LOCK[core_id].lock_raw() };

        // 1a. 获取任务表锁，唤醒到期睡眠任务 + 投递信号
        {
            let _table_guard = TASK_TABLE_LOCK.lock();
            let table = unsafe { task_table() };
            let sched = unsafe { per_cpu_sched(core_id) };
            wake_expired_sleepers(table, sched);
            deliver_pending_signals(table, sched);
        }

        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };

        // 2. 取出当前任务
        let prev = sched.current.take().expect("schedule: no current task");

        // 3. 若 prev 仍为 Running（主动让出），放回本核就绪队列
        if prev.state() == TaskState::Running {
            prev.set_state(TaskState::Ready);
            if !prev.is_idle() {
                sched.scheduler.enqueue(prev.clone());
            }
        }

        // 4. 从本核队列选取，若为空则尝试窃取，再为空则回退到 idle
        let next = sched
            .scheduler
            .pick_next()
            .or_else(|| try_steal(core_id))
            .unwrap_or_else(|| sched.idle.as_ref().expect("schedule: no idle task").clone());
        next.set_state(TaskState::Running);
        sched.current = Some(next.clone());

        // 5. 若为同一任务，直接释放锁返回
        if Arc::ptr_eq(&prev, &next) {
            unsafe { PER_CPU_SCHED_LOCK[core_id].unlock_raw() };
            return;
        }

        // 6. 上下文切换
        let prev_ctx = unsafe { prev.ctx_mut_ptr() };
        let next_ctx = unsafe { next.ctx_mut_ptr() };
        drop(prev);
        drop(next);

        unsafe { switch_to(prev_ctx, next_ctx) };

        // 7. 切换回来后释放调度锁（核心 ID 可能已变——任务迁移后在新核心上恢复）
        unsafe { PER_CPU_SCHED_LOCK[per_cpu::current_core_id()].unlock_raw() };
    }

    /// 主动让出 CPU。
    pub fn yield_now() {
        schedule();
    }

    // ─── sleep ──────────────────────────────────────────────────────────

    /// 挂起当前任务指定 tick 数。
    pub fn sleep(ticks: u64) {
        let task = current_task();
        let now = crate::arch::Arch::get_current_tick();
        task.set_wake_tick(now + ticks);
        task.set_state(TaskState::Sleeping);

        {
            let _guard = TASK_TABLE_LOCK.lock();
            let table = unsafe { task_table() };
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
        let task = current_task();
        task.set_state(TaskState::Blocked);

        {
            let _guard = TASK_TABLE_LOCK.lock();
            let table = unsafe { task_table() };
            if let Some(wq) = table
                .wait_queues
                .iter_mut()
                .find(|wq| wq.resource == resource)
            {
                wq.waiters.push(task);
            } else {
                table.wait_queues.push(WaitQueue {
                    resource,
                    waiters: alloc::vec![task],
                });
            }
        }

        schedule();
    }

    /// 唤醒在指定资源上阻塞的一个任务。
    pub fn wakeup_one(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let _table_guard = TASK_TABLE_LOCK.lock();
        let table = unsafe { task_table() };
        let sched = unsafe { per_cpu_sched(core_id) };
        wake_blocked_on_inner(table, sched, resource);
    }

    /// 唤醒在指定资源上阻塞的所有任务。
    pub fn wakeup_all(resource: ResourceId) {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let _table_guard = TASK_TABLE_LOCK.lock();
        let table = unsafe { task_table() };
        let sched = unsafe { per_cpu_sched(core_id) };
        let wq = match table
            .wait_queues
            .iter_mut()
            .find(|wq| wq.resource == resource)
        {
            Some(wq) => wq,
            None => return,
        };
        for task in wq.waiters.drain(..) {
            task.set_state(TaskState::Ready);
            sched.scheduler.enqueue(task);
        }
        table.wait_queues.retain(|wq| !wq.waiters.is_empty());
    }

    // ─── exit / wait ────────────────────────────────────────────────────

    /// 退出当前任务。
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

        wakeup_one(ResourceId::ChildExit(pid));
        wakeup_one(ResourceId::ChildExit(0));

        schedule();
        unreachable!("exited task was rescheduled");
    }

    /// 等待子进程退出。
    pub fn wait_child(child_pid: usize) -> KResult<(Pid, i32)> {
        loop {
            {
                let _guard = TASK_TABLE_LOCK.lock();
                let table = unsafe { task_table() };
                let caller_pid = current_task().pid();

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
        let parent = current_task();
        let child = spawn_kernel_thread_with_parent(name, entry, arg, Some(parent.pid()));
        Ok(child.pid())
    }

    // ─── signal ─────────────────────────────────────────────────────────

    /// 向指定任务发送信号。
    pub fn send_signal(pid: Pid, sig: Signal) -> KResult<()> {
        let core_id = per_cpu::current_core_id();
        let _sched_guard = PER_CPU_SCHED_LOCK[core_id].lock();
        let _table_guard = TASK_TABLE_LOCK.lock();
        let table = unsafe { task_table() };
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

    // ─── mutex ──────────────────────────────────────────────────────────

    /// 内核阻塞互斥锁
    pub struct KMutex {
        id: u64,
        owner: core::sync::atomic::AtomicUsize,
    }

    const KMUTEX_NO_OWNER: usize = usize::MAX;

    static NEXT_MUTEX_ID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

    impl KMutex {
        pub fn new() -> Self {
            Self {
                id: NEXT_MUTEX_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
                owner: core::sync::atomic::AtomicUsize::new(KMUTEX_NO_OWNER),
            }
        }

        pub fn lock(&self) {
            loop {
                let current_pid = current_task().pid();
                match self.owner.compare_exchange(
                    KMUTEX_NO_OWNER,
                    current_pid,
                    core::sync::atomic::Ordering::Acquire,
                    core::sync::atomic::Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(owner) => {
                        if owner == current_pid {
                            panic!("KMutex: recursive lock by pid={}", current_pid);
                        }
                        block_on(ResourceId::Mutex(self.id));
                    }
                }
            }
        }

        pub fn unlock(&self) {
            self.owner
                .store(KMUTEX_NO_OWNER, core::sync::atomic::Ordering::Release);
            wakeup_one(ResourceId::Mutex(self.id));
        }
    }
}

#[cfg(not(test))]
pub use manager::*;
