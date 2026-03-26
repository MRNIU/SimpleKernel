#[cfg(not(test))]
use crate::arch::ArchOps;
use crate::config;
use crate::memory::address::PhysAddr;
use core::cell::SyncUnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Once;

// P3 将字段类型升级为 PhysAddr，P4+ 读取这些字段
#[allow(dead_code)]
pub struct BasicInfo {
    pub physical_memory_addr: PhysAddr,
    pub physical_memory_size: usize,
    pub kernel_addr: PhysAddr,
    pub kernel_size: usize,
    pub elf_addr: PhysAddr,
    pub fdt_addr: PhysAddr,
    pub core_count: usize,
}

impl BasicInfo {
    #[must_use]
    #[allow(dead_code)] // P1 init.rs 中通过 BASIC_INFO.call_once() 使用
    pub const fn new() -> Self {
        Self {
            physical_memory_addr: PhysAddr::new(0),
            physical_memory_size: 0,
            kernel_addr: PhysAddr::new(0),
            kernel_size: 0,
            elf_addr: PhysAddr::new(0),
            fdt_addr: PhysAddr::new(0),
            core_count: 0,
        }
    }
}

pub static BASIC_INFO: Once<BasicInfo> = Once::new();

/// 锁栈条目——记录当前持有的 SpinLock 及其级别。
#[derive(Clone, Copy)]
pub struct LockStackEntry {
    /// 指向 SpinLock 的类型擦除原始指针，仅用于诊断比较，不会解引用。
    pub lock_ptr: *const (),
    /// 所持锁的级别。
    pub level: u8,
}

// SAFETY: lock_ptr 仅用于比较、从不解引用；访问发生在中断关闭的 per-CPU 上下文中。
unsafe impl Send for LockStackEntry {}
unsafe impl Sync for LockStackEntry {}

/// Per-CPU 锁栈，用于强制锁获取顺序。
///
/// 每个核心维护一个当前持有锁的栈。
/// 获取新锁时，SpinLock 检查新锁的级别是否高于栈顶。
pub struct LockStack {
    pub entries: [LockStackEntry; Self::MAX_DEPTH],
    pub depth: usize,
}

impl LockStack {
    pub const MAX_DEPTH: usize = 8;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [LockStackEntry {
                lock_ptr: core::ptr::null(),
                level: 0,
            }; Self::MAX_DEPTH],
            depth: 0,
        }
    }
}

/// 抢占状态 — 跟踪中断嵌套层数与调度标志
///
/// `hardirq_count`/`softirq_count`/`preempt_disable_count` 为 per-CPU 字段，
/// 仅由所属核心在中断关闭时访问，无需原子操作。
///
/// `need_resched`/`need_balance` 使用 `AtomicBool`，因为 P5+ 中其他核心
/// 可能通过 IPI 设置这些标志（例如唤醒任务时设置目标核心的 need_resched）。
pub struct PreemptState {
    /// 硬中断嵌套计数（>0 表示在 hardirq 上下文中）
    pub hardirq_count: u32,
    /// 软中断嵌套计数（>0 表示在 softirq 上下文中）
    pub softirq_count: u32,
    /// 抢占关闭计数（>0 表示抢占被禁用）
    pub preempt_disable_count: u32,
    /// 是否需要调度（原子：可被其他核心通过 IPI 设置）
    pub need_resched: AtomicBool,
    /// 是否需要负载均衡（原子：可被其他核心设置）
    pub need_balance: AtomicBool,
}

impl core::fmt::Debug for PreemptState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreemptState")
            .field("hardirq_count", &self.hardirq_count)
            .field("softirq_count", &self.softirq_count)
            .field("preempt_disable_count", &self.preempt_disable_count)
            .field("need_resched", &self.need_resched.load(Ordering::Relaxed))
            .field("need_balance", &self.need_balance.load(Ordering::Relaxed))
            .finish()
    }
}

impl PreemptState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hardirq_count: 0,
            softirq_count: 0,
            preempt_disable_count: 0,
            need_resched: AtomicBool::new(false),
            need_balance: AtomicBool::new(false),
        }
    }

    /// 进入硬中断上下文（递增 hardirq_count，饱和加法防止溢出）
    #[inline]
    #[allow(dead_code)]
    pub fn enter_hardirq(&mut self) {
        self.hardirq_count = self.hardirq_count.saturating_add(1);
    }

    /// 离开硬中断上下文
    #[inline]
    #[allow(dead_code)]
    pub fn exit_hardirq(&mut self) {
        self.hardirq_count = self.hardirq_count.saturating_sub(1);
    }

    /// 当前是否处于中断上下文（不可调度）
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn in_interrupt(&self) -> bool {
        self.hardirq_count > 0 || self.softirq_count > 0
    }

    /// 当前是否可以抢占
    #[inline]
    #[must_use]
    #[allow(dead_code)]
    pub fn preemptible(&self) -> bool {
        self.preempt_disable_count == 0 && !self.in_interrupt()
    }
}

#[repr(C, align(128))]
pub struct PerCpu {
    pub core_id: usize,
    pub lock_stack: LockStack,
    pub preempt: PreemptState,
}

impl PerCpu {
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self {
            core_id: id,
            lock_stack: LockStack::new(),
            preempt: PreemptState::new(),
        }
    }
}

static PER_CPU_ARRAY: SyncUnsafeCell<[PerCpu; config::MAX_CORE_COUNT]> = SyncUnsafeCell::new([
    PerCpu::new(0),
    PerCpu::new(1),
    PerCpu::new(2),
    PerCpu::new(3),
]);

/// 检查并清除当前核心的 `need_resched` 标志。
///
/// 与 `current_per_cpu()` 不同，此函数不要求中断关闭，
/// 因为 `need_resched` 是 `AtomicBool`，本身是原子操作。
/// 用于 idle loop 轮询。
#[cfg(not(test))]
pub fn check_and_clear_need_resched() -> bool {
    let core_id = current_core_id();
    // SAFETY: core_id < MAX_CORE_COUNT；AtomicBool::swap 是原子操作，无需同步
    let array = unsafe { &*PER_CPU_ARRAY.get() };
    array[core_id]
        .preempt
        .need_resched
        .swap(false, core::sync::atomic::Ordering::Acquire)
}

pub fn current_core_id() -> usize {
    #[cfg(not(test))]
    {
        crate::arch::Arch::core_id()
    }
    #[cfg(test)]
    {
        // 宿主机单元测试——始终返回核心 0
        0
    }
}

/// 返回当前核心的 `PerCpu` 数据的可变引用。
///
/// # Safety
/// 必须在中断关闭时调用（例如 SpinLock 临界区内），
/// 以防止同核心上的并发访问。
pub unsafe fn current_per_cpu() -> &'static mut PerCpu {
    // debug 模式下验证中断已关闭，防止误用
    #[cfg(all(debug_assertions, not(test)))]
    debug_assert!(
        !crate::arch::Arch::irq_enabled(),
        "current_per_cpu() 必须在中断关闭时调用"
    );

    let core_id = current_core_id();
    // SAFETY: core_id < MAX_CORE_COUNT 由硬件保证；
    // 调用方保证无并发访问（中断已关闭，已在上方断言验证）
    unsafe { &mut *(PER_CPU_ARRAY.get() as *mut PerCpu).add(core_id) }
}
