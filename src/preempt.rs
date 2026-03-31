//! 抢占控制——per-CPU 抢占计数器和调度请求标志。
//!
//! ## Per-CPU 状态
//!
//! | 变量 | 类型 | 说明 |
//! |------|------|------|
//! | `PREEMPT_DISABLE_COUNT` | `u32` | 抢占禁用嵌套深度 |
//! | `NEED_RESCHED` | `AtomicBool` | 调度请求标志（可跨核设置） |

use core::sync::atomic::{AtomicBool, Ordering};
use per_cpu::cpu_local;

/// 抢占关闭计数（>0 表示抢占被禁用）
#[cpu_local]
static PREEMPT_DISABLE_COUNT: u32 = 0;

/// 是否需要调度（原子：可被其他核心通过 IPI 设置）
#[cpu_local]
pub(crate) static NEED_RESCHED: AtomicBool = AtomicBool::new(false);

/// 检查并清除当前核心的 `need_resched` 标志（原子操作，无需关中断）。
///
/// 用于 idle loop 轮询。
pub fn check_and_clear_need_resched() -> bool {
    NEED_RESCHED.get().swap(false, Ordering::Acquire)
}

/// 当前是否可以抢占。
pub fn preemptible() -> bool {
    *PREEMPT_DISABLE_COUNT.get() == 0 && !interrupt_state::in_interrupt()
}

/// 设置指定核心的 `need_resched` 标志（用于 IPI 跨核唤醒）。
///
/// # Safety
/// `target_core` 必须是有效的核心 ID。
pub unsafe fn set_need_resched_on(target_core: usize) {
    unsafe { NEED_RESCHED.get_on(target_core) }.store(true, Ordering::Release);
}
