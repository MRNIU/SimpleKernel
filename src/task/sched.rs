//! Per-CPU 调度核心——调度锁、调度状态、上下文切换、任务窃取。

use alloc::sync::Arc;
use core::cell::SyncUnsafeCell;

use crate::arch::CalleeSavedContext;
use crate::task::scheduler::SchedPolicy;
use crate::task::scheduler::Scheduler;
use crate::task::state::TaskState;
use crate::task::tcb::TaskRef;
use config::MAX_CORE_COUNT;
use sync::SpinLock;
use sync::spinlock::lock_level;

// ─── switch_to 外部声明 ──────────────────────────────────────────────

unsafe extern "C" {
    fn switch_to(prev: *mut CalleeSavedContext, next: *const CalleeSavedContext);
}

// ─── Per-CPU 调度锁 ──────────────────────────────────────────────────

/// Per-CPU 调度锁数组——每个核心一把，保护对应核心的调度状态。
///
/// 使用 `lock_raw()`/`unlock_raw()` 跨越 `switch_to`。
/// 任务窃取时使用 `try_lock_raw_no_irq()` 获取其他核心的锁。
pub(super) static PER_CPU_SCHED_LOCK: [SpinLock<()>; MAX_CORE_COUNT] =
    [const { SpinLock::new_with_level((), "sched", lock_level::SCHED_LOCK) }; MAX_CORE_COUNT];

// ─── Per-CPU 调度状态 ────────────────────────────────────────────────

/// Per-CPU 调度状态（由对应的 PER_CPU_SCHED_LOCK[core_id] 保护）
pub(super) struct PerCpuSched {
    pub(super) scheduler: SchedPolicy,
    pub(super) current: Option<TaskRef>,
    pub(super) idle: Option<TaskRef>,
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

// ─── 任务窃取 ──────────────────────────────────────────────────────────

/// 从其他核心窃取一个任务——当本核就绪队列为空时调用。
///
/// 调用者已持有 PER_CPU_SCHED_LOCK[my_core]（通过 lock_raw）。
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

// ─── 调度函数 ────────────────────────────────────────────────────────

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
        let mut table = super::task_table::TASK_TABLE.lock();
        // SAFETY: 持有本核调度锁
        let sched = unsafe { per_cpu_sched(core_id) };
        table.wake_expired_sleepers(sched);
        table.deliver_pending_signals(sched);
    }

    // SAFETY: 持有本核调度锁
    let sched = unsafe { per_cpu_sched(core_id) };

    // 2. 取出当前任务
    let prev = sched.current.take().expect("schedule: no current task");

    // 3. 若 prev 仍为 Running（主动让出），放回本核就绪队列
    if prev.state() == TaskState::Running {
        prev.set_state(TaskState::Ready);
        if !prev.is_idle() {
            sched.scheduler.put_prev(prev.clone());
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
                unsafe { per_cpu::current_per_cpu() }
                    .preempt
                    .need_resched
                    .store(true, core::sync::atomic::Ordering::Release);
            }
        }
    }
}
