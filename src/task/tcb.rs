//! 任务控制块（TCB）与内核栈——任务的核心数据结构。

#[cfg(not(test))]
use alloc::sync::Arc;
#[cfg(test)]
use std::sync::Arc;

use core::sync::atomic::AtomicI32;

use crate::task::state::{AtomicTaskState, TaskState};

// ─── 类型别名 ─────────────────────────────────────────────────────────────────

/// 进程/任务 ID 类型
pub type Pid = usize;

/// 任务的引用计数指针
pub type TaskRef = Arc<TaskControlBlock>;

// ─── KernelStack ──────────────────────────────────────────────────────────────

/// 内核线程栈
///
/// 通过 `Vec<u8>` 在堆上分配，确保生命周期与 TCB 一致。
#[cfg(not(test))]
pub struct KernelStack {
    data: alloc::vec::Vec<u8>,
}

#[cfg(not(test))]
impl KernelStack {
    /// 分配一个新的内核栈（大小由 `config::KERNEL_STACK_SIZE` 决定）。
    pub fn new() -> Self {
        Self {
            data: alloc::vec![0u8; crate::config::KERNEL_STACK_SIZE],
        }
    }

    /// 返回栈顶地址（栈从高地址向低地址增长）。
    pub fn top(&self) -> usize {
        self.data.as_ptr() as usize + self.data.len()
    }
}

#[cfg(not(test))]
impl Default for KernelStack {
    fn default() -> Self {
        Self::new()
    }
}

// ─── init_kernel_thread_context ───────────────────────────────────────────────

/// 初始化内核线程的被调用者保存上下文，使 `switch_to` 后跳转到 `kernel_thread_entry`。
///
/// # Safety
///
/// 调用者必须确保 `ctx` 指针有效且当前未被其他线程访问。
#[cfg(all(not(test), target_arch = "riscv64"))]
unsafe fn init_kernel_thread_context(
    ctx: *mut crate::arch::riscv64::context::CalleeSavedContext,
    kstack: &KernelStack,
    entry: fn(usize),
    arg: usize,
) {
    unsafe extern "C" {
        fn kernel_thread_entry();
    }
    // SAFETY: 调用者保证 ctx 有效；各字段均为 POD 类型，直接写入安全
    unsafe {
        (*ctx).ra = kernel_thread_entry as u64;
        (*ctx).sp = kstack.top() as u64;
        (*ctx).s0 = entry as u64;
        (*ctx).s1 = arg as u64;
    }
}

/// 初始化内核线程的被调用者保存上下文，使 `switch_to` 后跳转到 `kernel_thread_entry`。
///
/// # Safety
///
/// 调用者必须确保 `ctx` 指针有效且当前未被其他线程访问。
#[cfg(all(not(test), target_arch = "aarch64"))]
unsafe fn init_kernel_thread_context(
    ctx: *mut crate::arch::aarch64::context::CalleeSavedContext,
    kstack: &KernelStack,
    entry: fn(usize),
    arg: usize,
) {
    unsafe extern "C" {
        fn kernel_thread_entry();
    }
    // SAFETY: 调用者保证 ctx 有效；各字段均为 POD 类型，直接写入安全
    unsafe {
        (*ctx).pc = kernel_thread_entry as u64;
        (*ctx).sp = kstack.top() as u64;
        // x19 → entry 函数指针（第一个参数）
        (*ctx).regs[0] = entry as u64;
        // x20 → arg（第二个参数）
        (*ctx).regs[1] = arg as u64;
    }
}

// ─── TaskControlBlock ─────────────────────────────────────────────────────────

/// 任务控制块（Task Control Block，TCB）
///
/// 描述一个任务（内核线程）的全部状态。字段分为两类：
/// - 只读标识符（`pid`、`name`、`is_idle`）：创建后不变
/// - 可变状态（`state`、`exit_code`）：原子操作，无锁读写
/// - 架构相关（`context`、`kstack`）：仅在持有调度锁（IRQ 关闭）时访问
pub struct TaskControlBlock {
    /// 任务 ID
    pid: Pid,
    /// 任务名称（静态字符串）
    name: &'static str,
    /// 是否为 idle 任务
    is_idle: bool,
    /// 任务当前状态（原子）
    state: AtomicTaskState,
    /// 退出码（原子，仅 Exited 状态后有效）
    exit_code: AtomicI32,
    /// 被调用者保存上下文（仅非测试模式）
    #[cfg(not(test))]
    context: core::cell::UnsafeCell<CalleeSavedContextWrapper>,
    /// 内核栈（idle 任务无栈，使用 Option）
    #[cfg(not(test))]
    kstack: Option<KernelStack>,
}

// SAFETY: state 与 exit_code 是原子的；context/kstack 仅在调度锁（IRQ off）下访问
unsafe impl Send for TaskControlBlock {}
// SAFETY: 同上
unsafe impl Sync for TaskControlBlock {}

/// 封装架构相关的 CalleeSavedContext，避免在 cfg 块内重复引用
#[cfg(all(not(test), target_arch = "riscv64"))]
type CalleeSavedContextWrapper = crate::arch::riscv64::context::CalleeSavedContext;

#[cfg(all(not(test), target_arch = "aarch64"))]
type CalleeSavedContextWrapper = crate::arch::aarch64::context::CalleeSavedContext;

impl TaskControlBlock {
    // ─── 构造函数 ──────────────────────────────────────────────────────────

    /// 创建 idle 任务（每个 CPU 核一个）。
    ///
    /// idle 任务已处于 Running 状态，无需内核栈（复用引导栈）。
    #[cfg(not(test))]
    pub fn new_idle(pid: Pid, core_id: usize) -> Self {
        // SAFETY: 静态字符串字面量生命周期为 'static
        let name: &'static str = match core_id {
            0 => "idle/0",
            1 => "idle/1",
            2 => "idle/2",
            3 => "idle/3",
            _ => "idle/N",
        };
        Self {
            pid,
            name,
            is_idle: true,
            state: AtomicTaskState::new(TaskState::Running),
            exit_code: AtomicI32::new(0),
            context: core::cell::UnsafeCell::new(CalleeSavedContextWrapper::default()),
            kstack: None,
        }
    }

    /// 创建内核线程任务并初始化上下文。
    ///
    /// 分配内核栈，将 `entry` 和 `arg` 编码到 `CalleeSavedContext` 中，
    /// 使 `switch_to` 后首次执行从 `kernel_thread_entry` 开始。
    #[cfg(not(test))]
    pub fn new_kernel_thread(pid: Pid, name: &'static str, entry: fn(usize), arg: usize) -> Self {
        let kstack = KernelStack::new();
        let mut ctx = CalleeSavedContextWrapper::default();
        // SAFETY: ctx 是本地变量，未被共享，指针有效
        unsafe {
            init_kernel_thread_context(&mut ctx as *mut _, &kstack, entry, arg);
        }
        Self {
            pid,
            name,
            is_idle: false,
            state: AtomicTaskState::new(TaskState::Ready),
            exit_code: AtomicI32::new(0),
            context: core::cell::UnsafeCell::new(ctx),
            kstack: Some(kstack),
        }
    }

    /// 创建测试用任务（仅在 `#[cfg(test)]` 下可用）。
    ///
    /// 不含上下文和内核栈，仅用于单元测试调度器逻辑。
    #[cfg(test)]
    pub fn new_for_test(pid: Pid, name: &'static str) -> Self {
        Self {
            pid,
            name,
            is_idle: false,
            state: AtomicTaskState::new(TaskState::Ready),
            exit_code: AtomicI32::new(0),
        }
    }

    // ─── 访问器 ────────────────────────────────────────────────────────────

    /// 返回任务 ID。
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// 返回任务名称。
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// 返回是否为 idle 任务。
    pub fn is_idle(&self) -> bool {
        self.is_idle
    }

    /// 读取当前任务状态（Acquire 语序）。
    pub fn state(&self) -> TaskState {
        self.state.load()
    }

    /// 写入任务状态（Release 语序）。
    pub fn set_state(&self, state: TaskState) {
        self.state.store(state);
    }

    /// 读取退出码（Acquire 语序）。
    pub fn exit_code(&self) -> i32 {
        self.exit_code.load(core::sync::atomic::Ordering::Acquire)
    }

    /// 写入退出码（Release 语序）。
    pub fn set_exit_code(&self, code: i32) {
        self.exit_code
            .store(code, core::sync::atomic::Ordering::Release);
    }

    /// 获取被调用者保存上下文的可变原始指针。
    ///
    /// # Safety
    ///
    /// 调用者必须持有调度锁（IRQ 关闭），且保证同一时刻只有一个核访问。
    #[cfg(not(test))]
    pub unsafe fn ctx_mut_ptr(&self) -> *mut CalleeSavedContextWrapper {
        self.context.get()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::state::TaskState;

    #[test]
    fn new_for_test_basic() {
        let tcb = TaskControlBlock::new_for_test(42, "worker");
        assert_eq!(tcb.pid(), 42);
        assert_eq!(tcb.name(), "worker");
        assert!(!tcb.is_idle());
        assert_eq!(tcb.state(), TaskState::Ready);
        assert_eq!(tcb.exit_code(), 0);
    }

    #[test]
    fn set_state_and_exit_code() {
        let tcb = TaskControlBlock::new_for_test(1, "t1");
        tcb.set_state(TaskState::Running);
        assert_eq!(tcb.state(), TaskState::Running);
        tcb.set_exit_code(-1);
        assert_eq!(tcb.exit_code(), -1);
    }

    #[test]
    fn task_ref_arc_clone() {
        let r1: TaskRef = Arc::new(TaskControlBlock::new_for_test(7, "arc_test"));
        let r2 = Arc::clone(&r1);
        assert_eq!(r1.pid(), r2.pid());
        assert_eq!(Arc::strong_count(&r1), 2);
    }
}
