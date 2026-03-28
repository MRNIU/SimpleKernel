//! Per-CPU 调度核心——调度锁、调度状态、上下文切换、任务窃取。
//!
//! 采用 Theseus 风格的无锁切换：
//! 1. `HeldInterrupts::hold()` 禁用中断
//! 2. RAII guard 获取调度锁 → 选任务 → 提取裸指针 → guard drop 释放锁
//! 3. `switch_to` 在无锁状态下执行（中断仍禁用）
//! 4. 恢复中断
//!
//! `prev` 不在 `switch_to` 前入队，而是存入 `deferred_prev`，
//! 在下次 `schedule()` 开头才入队，避免其他核偷走 prev 导致数据竞争。

use alloc::sync::Arc;
use core::cell::SyncUnsafeCell;

use crate::arch::CalleeSavedContext;
use crate::task::scheduler::SchedPolicy;
use crate::task::scheduler::Scheduler;
use crate::task::state::TaskState;
use crate::task::tcb::TaskRef;
use config::MAX_CORE_COUNT;
use sync::SpinLockIrq;
use sync::spinlock::lock_level;
unsafe extern "C" {
    fn switch_to(prev: *mut CalleeSavedContext, next: *const CalleeSavedContext);
}
/// Per-CPU 调度锁数组——每个核心一把，保护对应核心的调度状态。
///
/// `schedule()` 使用 RAII guard（`lock()`）获取和释放，锁在 `switch_to` 前释放。
/// 任务窃取时使用 `try_lock_raw_no_irq()` 获取其他核心的锁。
pub(super) static PER_CPU_SCHED_LOCK: [SpinLockIrq<()>; MAX_CORE_COUNT] =
    [const { SpinLockIrq::new_with_level((), "sched", lock_level::SCHED_LOCK) }; MAX_CORE_COUNT];
/// 延迟入队的 prev 任务——在下次 `schedule()` 开始时放回就绪队列。
///
/// `switch_to` 前不将 prev 入队，避免其他核偷走后与当前核的
/// `switch_to` 写操作产生数据竞争。
struct DeferredPrev {
    task: TaskRef,
    /// 调度优先级快照（CFS: vruntime；其他调度器: 0）
    priority: i64,
}

/// Per-CPU 调度状态（由对应的 PER_CPU_SCHED_LOCK[core_id] 保护）
pub(super) struct PerCpuSched {
    pub(super) scheduler: SchedPolicy,
    pub(super) current: Option<TaskRef>,
    pub(super) idle: Option<TaskRef>,
    /// 延迟入队的 prev 任务
    deferred_prev: Option<DeferredPrev>,
}

impl PerCpuSched {
    pub(super) fn new(scheduler: SchedPolicy) -> Self {
        Self {
            scheduler,
            current: None,
            idle: None,
            deferred_prev: None,
        }
    }
}

pub(super) static PER_CPU_SCHED: SyncUnsafeCell<[Option<PerCpuSched>; MAX_CORE_COUNT]> =
    SyncUnsafeCell::new([const { None }; MAX_CORE_COUNT]);

/// # Safety
///
/// 调用者必须持有 PER_CPU_SCHED_LOCK[core_id]。
pub(super) unsafe fn per_cpu_sched(core_id: usize) -> &'static mut PerCpuSched {
    // SAFETY: 调用者持有对应核心的调度锁
    let array = unsafe { &mut *PER_CPU_SCHED.get() };
    array[core_id].as_mut().expect("per_cpu_sched: 未初始化")
}
/// 从其他核心窃取一个任务——当本核就绪队列为空时调用。
///
/// 调用者已持有 PER_CPU_SCHED_LOCK[my_core]（通过 RAII guard），中断已禁用。
/// 使用 `try_lock_raw_no_irq` 获取 victim 的锁：两核同时窃取对方时不会死锁，
/// 因为 try_lock 失败后立即返回 None，不会阻塞等待。
pub(super) fn try_steal(my_core: usize) -> Option<TaskRef> {
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

    // SAFETY: 中断已被外层 HeldInterrupts 禁用
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

/// 启用中断——供 `kernel_thread_bootstrap` 在新任务首次运行时调用。
///
/// 新任务通过 `switch_to` 首次获得 CPU 时，中断处于禁用状态
/// （由 `schedule()` 的 `HeldInterrupts::hold()` 禁用）。
/// 调度锁已在 `switch_to` 前由 RAII guard 释放，无需手动解锁。
///
/// # Safety
///
/// 仅供 `kernel_thread_bootstrap` 在新任务首次运行时调用一次。
pub unsafe fn bootstrap_enable_irq() {
    // SAFETY: 调度锁已释放，启用中断是安全的
    unsafe { sync::irq_enable() };
}

/// 调度函数——选择下一个任务并执行上下文切换。
///
/// 采用 Theseus 风格的无锁切换协议：
/// 1. 外层 `HeldInterrupts` 禁用中断
/// 2. RAII guard 获取调度锁 → 选任务 → 提取裸指针 → guard drop 释放锁
/// 3. `switch_to` 在无锁状态下执行（中断仍禁用）
/// 4. 返回后 `held` drop 恢复中断
pub fn schedule() {
    let core_id = per_cpu::current_core_id();

    // 1. 禁用中断——跨越整个 switch_to，guard drop 后仍保持禁用
    let held = sync::HeldInterrupts::hold();

    // 2. 在锁内完成所有调度决策，提取裸指针后释放锁
    let switch_ctx = {
        let _guard = PER_CPU_SCHED_LOCK[core_id].lock();

        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };

        // 2a. 将上次 switch 延迟的 prev 入队
        if let Some(dp) = sched.deferred_prev.take() {
            sched.scheduler.enqueue_prev_deferred(dp.task, dp.priority);
        }

        // 2b. 唤醒到期睡眠任务 + 投递信号
        {
            let mut table = super::task_table::TASK_TABLE.lock();
            table.wake_expired_sleepers(sched);
            table.deliver_pending_signals(sched);
        }

        // 2c. 取出当前任务
        let prev = sched.current.take().expect("schedule: no current task");

        // 2d. 若 prev 仍为 Running（主动让出），标记为 Ready 并延迟入队
        if prev.state() == TaskState::Running {
            prev.set_state(TaskState::Ready);
            if !prev.is_idle() {
                let priority = sched.scheduler.snapshot_current_priority();
                sched.deferred_prev = Some(DeferredPrev {
                    task: prev.clone(),
                    priority,
                });
            }
        }

        // 2e. 从本核队列选取，若为空则尝试窃取，再尝试延迟的 prev，最后回退到 idle
        let next = sched
            .scheduler
            .pick_next()
            .or_else(|| try_steal(core_id))
            .or_else(|| sched.deferred_prev.take().map(|dp| dp.task))
            .unwrap_or_else(|| sched.idle.as_ref().expect("schedule: no idle task").clone());
        next.set_state(TaskState::Running);
        sched.current = Some(next.clone());

        // 2f. 若为同一任务，直接返回（held drop 恢复中断）
        if Arc::ptr_eq(&prev, &next) {
            return;
        }

        // 2g. 提取裸指针（锁仍持有，安全）
        let prev_ctx = unsafe { prev.ctx_mut_ptr() };
        let next_ctx = unsafe { next.ctx_mut_ptr() };
        drop(prev);
        drop(next);
        (prev_ctx, next_ctx)
        // _guard drop: 释放锁，内层 HeldInterrupts（was_enabled=false）no-op
    };

    // 3. 无锁上下文切换（中断仍禁用）
    unsafe { switch_to(switch_ctx.0, switch_ctx.1) };

    // 4. 恢复中断（对于从 switch_to 返回的已有任务）
    //    新任务通过 kernel_thread_bootstrap → bootstrap_enable_irq 恢复中断
    drop(held);
}

/// 主动让出 CPU。
pub fn yield_now() {
    schedule();
}

/// 由定时器中断调用——推进调度器内部记账（时间片、vruntime 等）。
///
/// 使用 `try_lock` 避免与正在进行的 `schedule()` 死锁：
/// 如果调度锁已被持有（`schedule()` 正在上下文切换），跳过本次 tick。
pub fn timer_tick() {
    let core_id = per_cpu::current_core_id();
    if let Some(_guard) = PER_CPU_SCHED_LOCK[core_id].try_lock() {
        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };
        if let Some(current) = sched.current.as_ref() {
            if sched.scheduler.task_tick(current) {
                // 调度策略判定需要抢占（时间片到期 / vruntime 超过队首）
                per_cpu::NEED_RESCHED
                    .get()
                    .store(true, core::sync::atomic::Ordering::Release);
            }
        }
    }
}
