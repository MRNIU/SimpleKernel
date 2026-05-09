// Copyright The SimpleKernel Contributors

//! 任务控制块（TCB）与内核栈——任务的核心数据结构。

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use alloc::sync::Arc;

use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

use crate::task::state::{AtomicTaskState, TaskState};

use crate::fs::fd_table::FileDescriptorTable;

/// 进程/任务 ID 类型
pub type Pid = usize;

/// 任务的引用计数指针
pub type TaskRef = Arc<TaskControlBlock>;

/// 内核线程栈
///
/// 通过显式 [`Layout`] 在堆上分配，确保生命周期与 TCB 一致，并把
/// RISC-V psABI / AAPCS64 要求的 16 字节栈对齐写入分配契约。
pub struct KernelStack {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl KernelStack {
    /// 内核线程栈 ABI 对齐。
    pub const ALIGN: usize = 16;

    /// 分配一个新的内核栈（大小由 `config::KERNEL_STACK_SIZE` 决定）。
    pub fn new() -> Self {
        let layout = Self::layout();
        // SAFETY: layout 由 `KernelStack::layout()` 构造，size 非零且对齐合法。
        let ptr = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));
        Self { ptr, layout }
    }

    /// 返回栈顶地址（栈从高地址向低地址增长）。
    pub fn top(&self) -> usize {
        let top = self.ptr.as_ptr() as usize + self.layout.size();
        assert_eq!(
            top % Self::ALIGN,
            0,
            "KernelStack::top: 栈顶未按 {} 字节对齐: top={:#x}",
            Self::ALIGN,
            top
        );
        top
    }

    fn layout() -> Layout {
        assert!(
            config::KERNEL_STACK_SIZE.is_multiple_of(Self::ALIGN),
            "KernelStack::layout: KERNEL_STACK_SIZE={} 不是 {} 的整数倍",
            config::KERNEL_STACK_SIZE,
            Self::ALIGN
        );
        Layout::from_size_align(config::KERNEL_STACK_SIZE, Self::ALIGN)
            .expect("KernelStack::layout: 无效的内核栈 Layout")
    }

    #[cfg(test)]
    fn bottom_for_test(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
}

impl Default for KernelStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // SAFETY: ptr/layout 来自同一次 `alloc_zeroed(layout)`，且只在 Drop 中释放一次。
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) };
    }
}

use crate::arch::CalleeSavedContext;

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
    /// 父任务 PID（None 表示无父任务，idle/init 等）
    parent_pid: Option<Pid>,
    /// 任务当前状态（原子）
    state: AtomicTaskState,
    /// 退出码（原子，仅 Exited 状态后有效）
    exit_code: AtomicI32,
    /// 睡眠唤醒 tick（0 表示非睡眠状态；原子：timer 检查时无需持锁）
    wake_tick: AtomicU64,
    /// 待处理信号位图（原子：可由其他核心设置）
    pending_signals: AtomicU32,
    /// 信号屏蔽位图（原子：可由任务自身修改）
    signal_mask: AtomicU32,
    /// 被调用者保存上下文
    context: core::cell::SyncUnsafeCell<CalleeSavedContext>,
    /// 内核栈（idle 任务无栈，使用 Option）
    kstack: Option<KernelStack>,
    /// 文件描述符表（每任务独立）
    fd_table: sync::SpinLock<FileDescriptorTable>,
}

// SAFETY: state 与 exit_code 是原子的；context 使用 SyncUnsafeCell（已实现 Sync）；
// kstack 仅在调度锁（IRQ off）下访问，同一时刻只有一个核持有
unsafe impl Send for TaskControlBlock {}
unsafe impl Sync for TaskControlBlock {}

impl TaskControlBlock {
    /// 创建 idle 任务（每个 CPU 核一个）。
    ///
    /// idle 任务已处于 Running 状态，无需内核栈（复用引导栈）。
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
            parent_pid: None,
            state: AtomicTaskState::new(TaskState::Running),
            exit_code: AtomicI32::new(0),
            wake_tick: AtomicU64::new(0),
            pending_signals: AtomicU32::new(0),
            signal_mask: AtomicU32::new(0),
            context: core::cell::SyncUnsafeCell::new(CalleeSavedContext::default()),
            kstack: None,
            fd_table: sync::SpinLock::new(
                FileDescriptorTable::new(),
                "fd_table",
                sync::lock_level::UNSPECIFIED,
            ),
        }
    }

    /// 创建内核线程任务并初始化上下文。
    ///
    /// 分配内核栈，将 `entry` 和 `arg` 编码到 `CalleeSavedContext` 中，
    /// 使 `switch_to` 后首次执行从 `kernel_thread_entry` 开始。
    pub fn new_kernel_thread(
        pid: Pid,
        name: &'static str,
        entry: fn(usize),
        arg: usize,
        parent_pid: Option<Pid>,
    ) -> Self {
        let kstack = KernelStack::new();
        let mut ctx = CalleeSavedContext::default();
        ctx.init_for_kernel_thread(kstack.top(), entry, arg);
        Self {
            pid,
            name,
            is_idle: false,
            parent_pid,
            state: AtomicTaskState::new(TaskState::Ready),
            exit_code: AtomicI32::new(0),
            wake_tick: AtomicU64::new(0),
            pending_signals: AtomicU32::new(0),
            signal_mask: AtomicU32::new(0),
            context: core::cell::SyncUnsafeCell::new(ctx),
            kstack: Some(kstack),
            fd_table: sync::SpinLock::new(
                FileDescriptorTable::new(),
                "fd_table",
                sync::lock_level::UNSPECIFIED,
            ),
        }
    }

    /// 创建测试用任务（仅在 `#[cfg(test)]` 下可用）。
    ///
    /// 使用默认上下文和空内核栈，仅用于单元测试调度器逻辑。
    #[cfg(test)]
    pub fn new_for_test(pid: Pid, name: &'static str) -> Self {
        Self {
            pid,
            name,
            is_idle: false,
            parent_pid: None,
            state: AtomicTaskState::new(TaskState::Ready),
            exit_code: AtomicI32::new(0),
            wake_tick: AtomicU64::new(0),
            pending_signals: AtomicU32::new(0),
            signal_mask: AtomicU32::new(0),
            context: core::cell::SyncUnsafeCell::new(CalleeSavedContext::default()),
            kstack: None,
            fd_table: sync::SpinLock::new(
                FileDescriptorTable::new(),
                "fd_table_test",
                sync::lock_level::UNSPECIFIED,
            ),
        }
    }

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
    ///
    /// debug 模式下校验状态转移合法性，非法转移触发 panic。
    pub fn set_state(&self, new: TaskState) {
        #[cfg(debug_assertions)]
        {
            use crate::task::state::TaskState::*;
            let old = self.state.load();
            let valid = matches!(
                (old, new),
                (UnInit, Ready)
                    | (Ready, Running)
                    | (Running, Ready)
                    | (Running, Sleeping)
                    | (Running, Blocked)
                    | (Running, Exited)
                    | (Running, Stopped)
                    | (Sleeping, Ready)
                    | (Blocked, Ready)
                    | (Stopped, Ready)
            );
            debug_assert!(
                valid,
                "非法状态转移: {:?} → {:?} (pid={})",
                old, new, self.pid
            );
        }
        self.state.store(new);
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

    /// 返回父任务 PID。
    pub fn parent_pid(&self) -> Option<Pid> {
        self.parent_pid
    }

    /// 读取睡眠唤醒 tick（0 表示非睡眠状态）。
    pub fn wake_tick(&self) -> u64 {
        self.wake_tick.load(Ordering::Acquire)
    }

    /// 设置睡眠唤醒 tick。
    pub fn set_wake_tick(&self, tick: u64) {
        self.wake_tick.store(tick, Ordering::Release);
    }

    /// 读取待处理信号位图。
    pub fn pending_signals(&self) -> u32 {
        self.pending_signals.load(Ordering::Acquire)
    }

    /// 原子设置一个待处理信号位。
    pub fn raise_signal(&self, sig_bit: u32) {
        self.pending_signals.fetch_or(sig_bit, Ordering::Release);
    }

    /// 原子清除一个待处理信号位。
    pub fn clear_signal(&self, sig_bit: u32) {
        self.pending_signals.fetch_and(!sig_bit, Ordering::Release);
    }

    /// 读取信号屏蔽位图。
    pub fn signal_mask(&self) -> u32 {
        self.signal_mask.load(Ordering::Acquire)
    }

    /// 写入信号屏蔽位图。
    pub fn set_signal_mask(&self, mask: u32) {
        self.signal_mask.store(mask, Ordering::Release);
    }

    /// 获取被调用者保存上下文的可变原始指针。
    ///
    /// # Safety
    ///
    /// 调用者必须持有调度锁（IRQ 关闭），且保证同一时刻只有一个核访问。
    pub unsafe fn ctx_mut_ptr(&self) -> *mut CalleeSavedContext {
        self.context.get()
    }

    /// 获取文件描述符表的引用。
    pub fn fd_table(&self) -> &sync::SpinLock<FileDescriptorTable> {
        &self.fd_table
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::state::TaskState;

    /// 验证 TCB 字段构造和各访问器的正确性。
    #[test]
    fn tcb_fields_and_accessors() {
        let tcb = TaskControlBlock::new_for_test(42, "worker");
        assert_eq!(tcb.pid(), 42);
        assert_eq!(tcb.name(), "worker");
        assert!(!tcb.is_idle());
        assert_eq!(tcb.parent_pid(), None);
        assert_eq!(tcb.state(), TaskState::Ready);
        assert_eq!(tcb.exit_code(), 0);
        assert_eq!(tcb.wake_tick(), 0);
        assert_eq!(tcb.pending_signals(), 0);
        assert_eq!(tcb.signal_mask(), 0);

        tcb.set_state(TaskState::Running);
        assert_eq!(tcb.state(), TaskState::Running);
        tcb.set_exit_code(-1);
        assert_eq!(tcb.exit_code(), -1);
        tcb.set_wake_tick(42);
        assert_eq!(tcb.wake_tick(), 42);
        tcb.set_signal_mask(0xFFFF_0000);
        assert_eq!(tcb.signal_mask(), 0xFFFF_0000);
    }

    /// 验证 TaskRef（Arc）克隆后引用计数正确。
    #[test]
    fn task_ref_arc_clone() {
        let r1: TaskRef = Arc::new(TaskControlBlock::new_for_test(7, "arc_test"));
        let r2 = Arc::clone(&r1);
        assert_eq!(r1.pid(), r2.pid());
        assert_eq!(Arc::strong_count(&r1), 2);
    }

    /// 验证信号位的原子设置和清除操作。
    #[test]
    fn signal_raise_and_clear() {
        let tcb = TaskControlBlock::new_for_test(10, "sig_test");
        tcb.raise_signal(1 << 1);
        tcb.raise_signal(1 << 3);
        assert_eq!(tcb.pending_signals(), (1 << 1) | (1 << 3));
        tcb.clear_signal(1 << 1);
        assert_eq!(tcb.pending_signals(), 1 << 3);
        tcb.clear_signal(1 << 3);
        assert_eq!(tcb.pending_signals(), 0);
    }

    /// 内核线程栈必须把 ABI 的 16 字节对齐写入分配契约。
    #[test]
    fn kernel_stack_uses_abi_alignment_contract() {
        let stack = KernelStack::new();
        assert_eq!(KernelStack::ALIGN, 16);
        assert_eq!(stack.bottom_for_test() % KernelStack::ALIGN, 0);
        assert_eq!(stack.top() % KernelStack::ALIGN, 0);
    }
}
