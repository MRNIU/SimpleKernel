# P5a: Core Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the minimal task subsystem so that multiple kernel threads run, context-switch, and are scheduled by a FIFO scheduler on all cores.

**Architecture:** ArceOS-style TCB design — atomic state (lock-free), `UnsafeCell` for context/sched-data (accessed only under sched_lock with IRQ off), `Arc<TCB>` for shared ownership. A global `SpinLock<()>` (sched_lock) serializes scheduling decisions; lock handoff via existing `lock_raw`/`unlock_raw` across `switch_to`. Bootstrap context becomes idle task; new threads start via `kernel_thread_entry` → `kernel_thread_bootstrap`.

**Tech Stack:** `alloc::sync::Arc`, `alloc::collections::VecDeque`, `core::sync::atomic`, `core::cell::UnsafeCell`, existing `SpinLock`, existing `switch_to` / `kernel_thread_entry` assembly.

**Scope:** P5a only — FIFO scheduler, kernel threads, single global run queue. Deferred to P5b: clone/exit/wait, sleep, signal, blocking mutex, SMP load balancing, CFS/RR schedulers.

---

## Design Decisions (TCB Ownership)

调研了 rCore-Tutorial、Redox、ArceOS、Linux Rust 四个内核后选择 ArceOS 模式：

| 方面 | rCore (SpinLock\<Inner\>) | 本方案 (Atomic + UnsafeCell) |
|------|--------------------------|------|
| 状态转换 | 获取锁 → 修改 → 释放 | `AtomicU8::compare_exchange`（无锁） |
| 上下文访问 | 获取锁 → 取裸指针 → 释放锁 → switch | `UnsafeCell::get()`（sched_lock 已保护） |
| 调度器队列 | `VecDeque<Arc<TCB>>` | 同（P5a 使用全局队列） |
| 适用场景 | 单核 | SMP 就绪（P5b 扩展为 per-CPU 队列） |

**核心不变量**：
- `context: UnsafeCell` — 仅在持有 sched_lock（IRQ off）时访问
- `state: AtomicU8` — 任何时候都可读，写入使用 `compare_exchange`
- `TaskRef = Arc<TaskControlBlock>` — 调度器队列和 current_task 各持有一个 Arc 引用

---

## File Structure

```
src/task/
├── mod.rs              — TaskManager 单例, init(), schedule(), spawn_kernel_thread(),
│                         current_task(), exit(), SCHED_LOCK, kernel_thread_bootstrap
├── tcb.rs              — TaskControlBlock, TaskRef, Pid, KernelStack
├── state.rs            — TaskState enum, transition() (编译期穷举)
└── scheduler/
    ├── mod.rs          — Scheduler trait
    └── fifo.rs         — FifoScheduler (VecDeque<TaskRef>)

Modified files:
├── src/main.rs                     — 替换 phase4 后的 spin_loop 为 task::init + idle loop
├── src/per_cpu.rs                  — (不修改 PerCpu 结构；current_task 存于 SchedState)
├── src/arch/riscv64/mod.rs         — kernel_thread_bootstrap 实现
├── src/arch/aarch64/mod.rs         — kernel_thread_bootstrap 实现
├── src/arch/riscv64/timer.rs       — tick 中设置 need_resched
└── src/arch/aarch64/timer.rs       — tick 中设置 need_resched
```

---

## Task 1: TaskState + Transitions

**Files:**
- Create: `src/task/state.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests for state transitions**

```rust
// src/task/state.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_transitions() {
        assert_eq!(transition(TaskState::Ready, TaskMsg::Schedule), Ok(TaskState::Running));
        assert_eq!(transition(TaskState::Running, TaskMsg::Yield), Ok(TaskState::Ready));
        assert_eq!(
            transition(TaskState::Running, TaskMsg::Exit { code: 0 }),
            Ok(TaskState::Exited { exit_code: 0 })
        );
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(transition(TaskState::Ready, TaskMsg::Yield).is_err());
        assert!(transition(TaskState::Exited { exit_code: 0 }, TaskMsg::Schedule).is_err());
    }

    #[test]
    fn atomic_state_roundtrip() {
        use core::sync::atomic::{AtomicU8, Ordering};
        let a = AtomicU8::new(TaskState::Ready.as_u8());
        assert_eq!(TaskState::from_u8(a.load(Ordering::Relaxed)), Some(TaskState::Ready));
    }
}
```

- [ ] **Step 2: Run tests — expect compile failure (types don't exist)**

Run: `cargo test -- state`
Expected: FAIL (module not found)

- [ ] **Step 3: Implement TaskState, TaskMsg, transition()**

```rust
// src/task/state.rs

/// 任务状态 — 编码为 u8 供 AtomicU8 使用
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 已创建未入队
    UnInit = 0,
    /// 就绪，在调度队列中
    Ready = 1,
    /// 正在运行
    Running = 2,
    /// 定时睡眠中（P5b）
    Sleeping = 3,
    /// 资源阻塞中（P5b）
    Blocked = 4,
    /// 已退出（可回收）
    Exited = 5,
}

impl TaskState {
    pub const fn as_u8(self) -> u8 { self as u8 }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::UnInit),
            1 => Some(Self::Ready),
            2 => Some(Self::Running),
            3 => Some(Self::Sleeping),
            4 => Some(Self::Blocked),
            5 => Some(Self::Exited),
            _ => None,
        }
    }
}

/// 状态转换消息
#[derive(Debug, Clone, Copy)]
pub enum TaskMsg {
    /// 入队就绪
    Schedule,
    /// 被调度器选中
    PickUp,
    /// 主动让出
    Yield,
    /// 退出
    Exit { code: i32 },
    /// 唤醒（从 Sleeping/Blocked 恢复）
    Wakeup,
}

/// 非法状态转换
#[derive(Debug, Clone, Copy)]
pub struct InvalidTransition {
    pub from: TaskState,
    pub msg: TaskMsg,
}

/// 状态转换 — 编译器保证所有组合被处理
pub fn transition(state: TaskState, msg: TaskMsg) -> Result<TaskState, InvalidTransition> {
    match (state, msg) {
        (TaskState::UnInit,   TaskMsg::Schedule)      => Ok(TaskState::Ready),
        (TaskState::Ready,    TaskMsg::PickUp)         => Ok(TaskState::Running),
        (TaskState::Running,  TaskMsg::Yield)          => Ok(TaskState::Ready),
        (TaskState::Running,  TaskMsg::Exit { .. })    => Ok(TaskState::Exited),
        (TaskState::Sleeping, TaskMsg::Wakeup)         => Ok(TaskState::Ready),
        (TaskState::Blocked,  TaskMsg::Wakeup)         => Ok(TaskState::Ready),
        (from, msg) => Err(InvalidTransition { from, msg }),
    }
}
```

注意：P5a 不需要 Zombie/Stopped 状态（无 parent/wait 机制），Exited 不携带 exit_code（存在 TCB 字段中）。P5b 扩展时添加。

- [ ] **Step 4: Register module — add `pub mod task;` to main.rs, `pub mod state;` to task/mod.rs**

Create minimal `src/task/mod.rs`:
```rust
pub mod state;
```

Add to `src/main.rs` (after `mod sync;`):
```rust
mod task;
```

- [ ] **Step 5: Run tests — expect PASS**

Run: `cargo test -- state`
Expected: all 3 tests pass

- [ ] **Step 6: Commit**

```bash
git add src/task/state.rs src/task/mod.rs src/main.rs
git commit --signoff -m "feat(P5a): add TaskState enum and transition function with unit tests"
```

---

## Task 2: Scheduler Trait + FifoScheduler

**Files:**
- Create: `src/task/scheduler/mod.rs`
- Create: `src/task/scheduler/fifo.rs`
- Modify: `src/task/mod.rs` — add `pub mod scheduler;`

- [ ] **Step 1: Write failing tests for FifoScheduler**

```rust
// src/task/scheduler/fifo.rs — tests at bottom
#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::tcb::TaskControlBlock;
    use alloc::sync::Arc;

    fn make_task(pid: usize) -> TaskRef {
        Arc::new(TaskControlBlock::new_for_test(pid, "test"))
    }

    #[test]
    fn enqueue_and_pick() {
        let mut sched = FifoScheduler::new();
        let t1 = make_task(1);
        let t2 = make_task(2);
        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());

        let next = sched.pick_next().expect("should have task");
        assert_eq!(next.pid(), 1);
        let next = sched.pick_next().expect("should have task");
        assert_eq!(next.pid(), 2);
        assert!(sched.pick_next().is_none());
    }

    #[test]
    fn is_empty_and_size() {
        let mut sched = FifoScheduler::new();
        assert!(sched.is_empty());
        sched.enqueue(make_task(1));
        assert!(!sched.is_empty());
        assert_eq!(sched.queue_size(), 1);
    }
}
```

- [ ] **Step 2: Implement Scheduler trait**

```rust
// src/task/scheduler/mod.rs
pub mod fifo;

use crate::task::tcb::TaskRef;

/// 调度器接口
///
/// 实现此 trait 以添加新的调度算法。
/// P5a 仅实现 FIFO；P5b 添加 RoundRobin 和 CFS。
pub trait Scheduler: Send {
    /// 将就绪任务加入队列
    fn enqueue(&mut self, task: TaskRef);

    /// 从队列中选取下一个运行的任务（移出队列）
    fn pick_next(&mut self) -> Option<TaskRef>;

    /// 定时器 tick 回调 — 返回 true 表示当前任务时间片耗尽需调度
    fn task_tick(&mut self, _current: &crate::task::tcb::TaskControlBlock) -> bool {
        false // FIFO 不抢占
    }

    fn queue_size(&self) -> usize;
    fn is_empty(&self) -> bool;
}
```

- [ ] **Step 3: Implement FifoScheduler**

```rust
// src/task/scheduler/fifo.rs
use super::Scheduler;
use crate::task::tcb::TaskRef;
use alloc::collections::VecDeque;

/// FIFO 先来先服务调度器
///
/// 任务按入队顺序运行，不抢占。主动 yield 后重新排到队尾。
pub struct FifoScheduler {
    queue: VecDeque<TaskRef>,
}

impl FifoScheduler {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Scheduler for FifoScheduler {
    fn enqueue(&mut self, task: TaskRef) {
        self.queue.push_back(task);
    }

    fn pick_next(&mut self) -> Option<TaskRef> {
        self.queue.pop_front()
    }

    fn queue_size(&self) -> usize {
        self.queue.len()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
```

- [ ] **Step 4: Register modules in task/mod.rs**

```rust
pub mod scheduler;
pub mod state;
```

- [ ] **Step 5: Run tests — will fail because tcb module doesn't exist yet (that's expected, tests reference it)**

Note: tests depend on Task 3 (tcb.rs). Mark this step as "tests deferred to after Task 3".

- [ ] **Step 6: Commit trait and FIFO implementation (tests run after Task 3)**

```bash
git add src/task/scheduler/
git commit --signoff -m "feat(P5a): add Scheduler trait and FifoScheduler implementation"
```

---

## Task 3: TaskControlBlock + KernelStack

**Files:**
- Create: `src/task/tcb.rs`
- Modify: `src/task/mod.rs` — add `pub mod tcb;`
- Modify: `src/config.rs` — add KERNEL_STACK_SIZE

- [ ] **Step 1: Add KERNEL_STACK_SIZE constant**

Add to `src/config.rs`:
```rust
/// 内核线程栈大小（16KB，与 boot.S 中 DEFAULT_STACK_SIZE 一致）
pub const KERNEL_STACK_SIZE: usize = 16 * 1024;
```

- [ ] **Step 2: Implement TCB**

```rust
// src/task/tcb.rs

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI32, AtomicU8, Ordering};

use alloc::sync::Arc;

use super::state::TaskState;

/// 进程 ID
pub type Pid = usize;

/// 任务引用 — Arc 共享所有权
pub type TaskRef = Arc<TaskControlBlock>;

/// 任务控制块
///
/// 设计参考 ArceOS：
/// - `state`: AtomicU8，无锁状态转换
/// - `context`: UnsafeCell，仅在持有 sched_lock（IRQ off）时访问
/// - 不可变字段（pid, name, is_idle）创建后不变，无需同步
///
/// # Safety
/// `context` 的访问必须满足：持有 sched_lock 且中断已关闭。
pub struct TaskControlBlock {
    pid: Pid,
    name: &'static str,
    is_idle: bool,
    state: AtomicU8,
    exit_code: AtomicI32,
    #[cfg(not(test))]
    context: UnsafeCell<crate::arch_context::CalleeSavedContext>,
    #[cfg(not(test))]
    kstack: Option<KernelStack>,
}

// SAFETY: state 是原子的；context/kstack 仅在 sched_lock（IRQ off）下访问
unsafe impl Send for TaskControlBlock {}
unsafe impl Sync for TaskControlBlock {}

impl TaskControlBlock {
    /// 创建 idle 任务（使用当前栈，无需分配）
    #[cfg(not(test))]
    pub fn new_idle(pid: Pid, core_id: usize) -> Self {
        let name = match core_id {
            0 => "idle/0",
            1 => "idle/1",
            2 => "idle/2",
            3 => "idle/3",
            _ => "idle/?",
        };
        Self {
            pid,
            name,
            is_idle: true,
            state: AtomicU8::new(TaskState::Running.as_u8()),
            exit_code: AtomicI32::new(0),
            context: UnsafeCell::new(crate::arch_context::CalleeSavedContext::default()),
            kstack: None, // idle 使用 boot 栈
        }
    }

    /// 创建内核线程
    #[cfg(not(test))]
    pub fn new_kernel_thread(
        pid: Pid,
        name: &'static str,
        entry: fn(usize),
        arg: usize,
    ) -> Self {
        let kstack = KernelStack::new();
        let context = init_kernel_thread_context(&kstack, entry, arg);
        Self {
            pid,
            name,
            is_idle: false,
            state: AtomicU8::new(TaskState::Ready.as_u8()),
            exit_code: AtomicI32::new(0),
            context: UnsafeCell::new(context),
            kstack: Some(kstack),
        }
    }

    /// 测试用构造 —— 无 context/kstack
    #[cfg(test)]
    pub fn new_for_test(pid: Pid, name: &'static str) -> Self {
        Self {
            pid,
            name,
            is_idle: false,
            state: AtomicU8::new(TaskState::Ready.as_u8()),
            exit_code: AtomicI32::new(0),
        }
    }

    pub fn pid(&self) -> Pid { self.pid }
    pub fn name(&self) -> &str { self.name }
    pub fn is_idle(&self) -> bool { self.is_idle }

    pub fn state(&self) -> TaskState {
        TaskState::from_u8(self.state.load(Ordering::Acquire))
            .expect("invalid task state")
    }

    pub fn set_state(&self, new: TaskState) {
        self.state.store(new.as_u8(), Ordering::Release);
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Relaxed)
    }

    pub fn set_exit_code(&self, code: i32) {
        self.exit_code.store(code, Ordering::Relaxed);
    }

    /// 获取 context 的可变裸指针
    ///
    /// # Safety
    /// 调用方必须持有 sched_lock 且中断已关闭。
    #[cfg(not(test))]
    pub unsafe fn ctx_mut_ptr(&self) -> *mut crate::arch_context::CalleeSavedContext {
        self.context.get()
    }
}

/// 内核栈
#[cfg(not(test))]
pub struct KernelStack {
    data: alloc::vec::Vec<u8>,
}

#[cfg(not(test))]
impl KernelStack {
    pub fn new() -> Self {
        let mut data = alloc::vec![0u8; crate::config::KERNEL_STACK_SIZE];
        // 确保 16 字节对齐（Vec 通常已对齐，此处为防御性编程）
        assert!(data.as_ptr() as usize % 16 == 0, "stack not 16-byte aligned");
        Self { data }
    }

    /// 栈顶地址（高地址端）
    pub fn top(&self) -> usize {
        self.data.as_ptr() as usize + self.data.len()
    }
}

/// 构造内核线程的初始 CalleeSavedContext
///
/// switch_to 恢复此 context 后，CPU 跳转到 kernel_thread_entry，
/// 然后调用 kernel_thread_bootstrap(entry, arg)。
#[cfg(not(test))]
fn init_kernel_thread_context(
    kstack: &KernelStack,
    entry: fn(usize),
    arg: usize,
) -> crate::arch_context::CalleeSavedContext {
    // kernel_thread_entry 由 switch.S 定义
    unsafe extern "C" { fn kernel_thread_entry(); }

    let mut ctx = crate::arch_context::CalleeSavedContext::default();

    #[cfg(target_arch = "riscv64")]
    {
        // ra = kernel_thread_entry（switch_to 的 ret 跳转到此）
        ctx.ra = kernel_thread_entry as unsafe extern "C" fn() as u64;
        // sp = 栈顶
        ctx.sp = kstack.top() as u64;
        // s0 = entry 函数指针, s1 = arg
        ctx.s0 = entry as usize as u64;
        ctx.s1 = arg as u64;
    }

    #[cfg(target_arch = "aarch64")]
    {
        // pc = kernel_thread_entry（RestoreCalleeSavedContext 的 br x10 跳转到此）
        ctx.pc = kernel_thread_entry as unsafe extern "C" fn() as u64;
        // sp = 栈顶
        ctx.sp = kstack.top() as u64;
        // x19 = entry, x20 = arg（kernel_thread_entry 传递给 kernel_thread_bootstrap）
        ctx.regs[0] = entry as usize as u64; // x19
        ctx.regs[1] = arg as u64;            // x20
    }

    ctx
}
```

注意：`crate::arch_context` 是一个模块别名，需要在 `task/mod.rs` 中用 `use` 或直接写完整路径。实际路径是 `crate::arch::{riscv64,aarch64}::context`。由于 `arch` 模块是 `pub(crate)`，任务模块可直接访问。

考虑到 `arch` 模块在 `#[cfg(not(test))]` 下才存在，所有引用 arch 的代码需要相应的 cfg gate。

- [ ] **Step 3: Register module**

Update `src/task/mod.rs`:
```rust
pub mod scheduler;
pub mod state;
pub mod tcb;
```

- [ ] **Step 4: Run Task 2 tests (FifoScheduler)**

Run: `cargo test -- fifo`
Expected: PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```bash
git add src/task/tcb.rs src/task/mod.rs src/config.rs
git commit --signoff -m "feat(P5a): add TaskControlBlock with ArceOS-style atomic state and UnsafeCell context"
```

---

## Task 4: TaskManager + init()

**Files:**
- Modify: `src/task/mod.rs` — add TaskManager, SCHED_LOCK, init(), spawn_kernel_thread()
- Modify: `src/main.rs` — call task::init() and task::spawn

- [ ] **Step 1: Implement TaskManager core in task/mod.rs**

```rust
// src/task/mod.rs
pub mod scheduler;
pub mod state;
pub mod tcb;

#[cfg(not(test))]
use alloc::sync::Arc;
#[cfg(not(test))]
use alloc::vec::Vec;
#[cfg(not(test))]
use core::cell::SyncUnsafeCell;

#[cfg(not(test))]
use crate::config::MAX_CORE_COUNT;
#[cfg(not(test))]
use crate::sync::SpinLock;
#[cfg(not(test))]
use tcb::{Pid, TaskRef};

#[cfg(not(test))]
use scheduler::fifo::FifoScheduler;
#[cfg(not(test))]
use scheduler::Scheduler;
#[cfg(not(test))]
use state::TaskState;

// switch_to 由 switch.S 提供
#[cfg(not(test))]
unsafe extern "C" {
    fn switch_to(
        prev: *mut crate::arch::CalleeSavedContextAlias,
        next: *const crate::arch::CalleeSavedContextAlias,
    );
}

/// 调度锁 — 保护 SCHED_STATE 的访问。
/// 使用 SpinLock<()> 因为 lock_raw/unlock_raw 需要跨越 switch_to。
#[cfg(not(test))]
static SCHED_LOCK: SpinLock<()> = SpinLock::new((), "sched");

/// 调度状态 — 由 SCHED_LOCK 保护
#[cfg(not(test))]
struct SchedState {
    scheduler: FifoScheduler,
    tasks: Vec<TaskRef>,
    current: [Option<TaskRef>; MAX_CORE_COUNT],
    idle: [Option<TaskRef>; MAX_CORE_COUNT],
    next_pid: Pid,
}

#[cfg(not(test))]
static SCHED_STATE: SyncUnsafeCell<Option<SchedState>> = SyncUnsafeCell::new(None);

/// 获取 SchedState 可变引用（必须在持有 SCHED_LOCK 时调用）
#[cfg(not(test))]
unsafe fn sched_state() -> &'static mut SchedState {
    // SAFETY: 调用方持有 SCHED_LOCK
    unsafe { (*SCHED_STATE.get()).as_mut().expect("SchedState 未初始化") }
}

/// 主核任务初始化
///
/// 创建 idle 任务（当前 bootstrap 上下文），设为 current。
/// 必须在 memory::init() 之后调用（需要堆分配）。
#[cfg(not(test))]
pub fn init() {
    let core_id = crate::per_cpu::current_core_id();

    // 初始化 SchedState
    // SAFETY: 仅在 init 中调用一次，此时无其他核心访问
    unsafe {
        *SCHED_STATE.get() = Some(SchedState {
            scheduler: FifoScheduler::new(),
            tasks: Vec::new(),
            current: core::array::from_fn(|_| None),
            idle: core::array::from_fn(|_| None),
            next_pid: 1,
        });
    }

    // 创建 idle 任务（代表当前 bootstrap 上下文）
    let idle = Arc::new(tcb::TaskControlBlock::new_idle(0, core_id));
    let state = unsafe { sched_state() };
    state.tasks.push(idle.clone());
    state.idle[core_id] = Some(idle.clone());
    state.current[core_id] = Some(idle);
    state.next_pid = 1;

    log::info!("TaskInit: idle task created for core {}", core_id);
}

/// 从核任务初始化
#[cfg(not(test))]
pub fn init_smp() {
    let core_id = crate::per_cpu::current_core_id();

    // 需要 sched_lock 因为主核可能正在访问 SchedState
    let _guard = SCHED_LOCK.lock();
    let state = unsafe { sched_state() };

    let idle = Arc::new(tcb::TaskControlBlock::new_idle(0, core_id));
    state.tasks.push(idle.clone());
    state.idle[core_id] = Some(idle.clone());
    state.current[core_id] = Some(idle);

    log::info!("TaskInitSMP: idle task created for core {}", core_id);
}

/// 创建内核线程
#[cfg(not(test))]
pub fn spawn_kernel_thread(name: &'static str, entry: fn(usize), arg: usize) -> TaskRef {
    let _guard = SCHED_LOCK.lock();
    let state = unsafe { sched_state() };

    let pid = state.next_pid;
    state.next_pid += 1;

    let task = Arc::new(tcb::TaskControlBlock::new_kernel_thread(pid, name, entry, arg));
    state.tasks.push(task.clone());
    state.scheduler.enqueue(task.clone());

    log::info!("Task [pid={}] \"{}\" created", pid, name);
    task
}

/// 获取当前核心正在运行的任务
#[cfg(not(test))]
pub fn current_task() -> TaskRef {
    let core_id = crate::per_cpu::current_core_id();
    // SAFETY: current 仅在 sched_lock 下修改；读取在 IRQ off 下安全
    // （SpinLock 内部或中断处理器中都会 IRQ off）
    let state = unsafe { &*SCHED_STATE.get() }
        .as_ref()
        .expect("SchedState 未初始化");
    state.current[core_id]
        .as_ref()
        .expect("no current task")
        .clone()
}
```

注意：`CalleeSavedContextAlias` 需要在 `arch/mod.rs` 中添加一个类型别名，使 `switch_to` 签名不用写 cfg。

- [ ] **Step 2: Add CalleeSavedContext type alias to arch/mod.rs**

在 `src/arch/mod.rs` 中添加：
```rust
#[cfg(target_arch = "riscv64")]
pub type CalleeSavedContextAlias = riscv64::context::CalleeSavedContext;

#[cfg(target_arch = "aarch64")]
pub type CalleeSavedContextAlias = aarch64::context::CalleeSavedContext;
```

- [ ] **Step 3: Run `cargo test` — verify existing tests still pass**

- [ ] **Step 4: Commit**

```bash
git add src/task/mod.rs src/arch/mod.rs
git commit --signoff -m "feat(P5a): add TaskManager with init, spawn_kernel_thread, sched_lock"
```

---

## Task 5: schedule() + kernel_thread_bootstrap

**Files:**
- Modify: `src/task/mod.rs` — add schedule(), exit()
- Modify: `src/arch/riscv64/mod.rs` — kernel_thread_bootstrap 实现
- Modify: `src/arch/aarch64/mod.rs` — kernel_thread_bootstrap 实现

- [ ] **Step 1: Implement schedule()**

在 `src/task/mod.rs` 中添加：

```rust
/// 调度 — 选择下一个任务并切换
///
/// 1. 获取 sched_lock（lock_raw，不创建 guard）
/// 2. 将当前任务放回队列（如果还是 Running → 改为 Ready）
/// 3. 从调度器选择下一个任务（无任务则选 idle）
/// 4. 若 next == current，释放锁并返回
/// 5. switch_to(prev_ctx, next_ctx)
/// 6. 在新上下文中释放 sched_lock（unlock_raw）
#[cfg(not(test))]
pub fn schedule() {
    let core_id = crate::per_cpu::current_core_id();

    // SAFETY: schedule 在中断关闭或内核代码中调用
    unsafe { SCHED_LOCK.lock_raw() };

    let state = unsafe { sched_state() };

    // 取出当前任务
    let prev = state.current[core_id]
        .take()
        .expect("schedule: no current task");

    // 如果 prev 仍在运行（主动 yield），放回队列
    if prev.state() == TaskState::Running {
        prev.set_state(TaskState::Ready);
        if !prev.is_idle() {
            state.scheduler.enqueue(prev.clone());
        }
    }

    // 选择下一个任务
    let next = state.scheduler.pick_next().unwrap_or_else(|| {
        state.idle[core_id]
            .as_ref()
            .expect("schedule: no idle task")
            .clone()
    });

    next.set_state(TaskState::Running);
    state.current[core_id] = Some(next.clone());

    // 同一任务则无需切换
    if Arc::ptr_eq(&prev, &next) {
        // SAFETY: 释放 sched_lock
        unsafe { SCHED_LOCK.unlock_raw() };
        return;
    }

    // 获取上下文裸指针
    // SAFETY: 持有 sched_lock，IRQ off
    let prev_ctx = unsafe { prev.ctx_mut_ptr() };
    let next_ctx = unsafe { next.ctx_mut_ptr() };

    // 释放 Arc 引用（防止 Arc 析构在 switch_to 之后的旧栈上跑）
    drop(prev);
    drop(next);

    // SAFETY: 上下文指针有效，sched_lock 跨越 switch_to
    unsafe { switch_to(prev_ctx, next_ctx) };

    // === 现在在新任务的上下文中 ===
    // 对于恢复的任务：释放 sched_lock
    // 对于新任务：kernel_thread_bootstrap 中释放
    // 因此此处仅恢复的任务会执行到这里
    unsafe { SCHED_LOCK.unlock_raw() };
}

/// 主动让出 CPU
#[cfg(not(test))]
pub fn yield_now() {
    schedule();
}

/// 退出当前任务
#[cfg(not(test))]
pub fn exit(code: i32) -> ! {
    let task = current_task();
    task.set_exit_code(code);
    task.set_state(TaskState::Exited);
    log::info!("Task [pid={}] \"{}\" exited with code {}", task.pid(), task.name(), code);
    schedule();
    unreachable!("exited task was rescheduled");
}
```

- [ ] **Step 2: Implement kernel_thread_bootstrap for both architectures**

修改 `src/arch/riscv64/mod.rs`：
```rust
#[unsafe(no_mangle)]
pub extern "C" fn kernel_thread_bootstrap(entry: usize, arg: usize) -> ! {
    // 释放 schedule() 中通过 lock_raw 获取的 sched_lock
    // SAFETY: schedule() 在 switch_to 前获取了 sched_lock，
    // 新任务首次运行时负责释放
    unsafe { crate::task::release_sched_lock() };

    // 调用入口函数
    let entry_fn: fn(usize) = unsafe { core::mem::transmute(entry) };
    entry_fn(arg);

    // 入口函数返回 → 退出任务
    crate::task::exit(0);
}
```

修改 `src/arch/aarch64/mod.rs` 同理（相同代码）。

- [ ] **Step 3: Add release_sched_lock helper**

在 `src/task/mod.rs` 中添加：
```rust
/// 释放 sched_lock — 供 kernel_thread_bootstrap 调用
///
/// # Safety
/// 调用方必须在 sched_lock 被 lock_raw 持有的上下文中调用。
#[cfg(not(test))]
pub unsafe fn release_sched_lock() {
    unsafe { SCHED_LOCK.unlock_raw() };
}
```

- [ ] **Step 4: Cross-compile check**

Run: `cargo xtask build --arch riscv64`
Expected: build succeeds

Run: `cargo xtask build --arch aarch64`
Expected: build succeeds

- [ ] **Step 5: Commit**

```bash
git add src/task/mod.rs src/arch/riscv64/mod.rs src/arch/aarch64/mod.rs
git commit --signoff -m "feat(P5a): implement schedule(), kernel_thread_bootstrap, and task exit"
```

---

## Task 6: Connect to P4 (Timer + Bootstrap)

**Files:**
- Modify: `src/main.rs` — 替换 idle loop，调用 task::init/spawn/schedule
- Modify: `src/arch/riscv64/timer.rs` — tick 中设置 need_resched
- Modify: `src/arch/aarch64/timer.rs` — tick 中设置 need_resched

- [ ] **Step 1: Modify bootstrap() to use task system**

修改 `src/main.rs` 中的 `bootstrap()`:
```rust
#[cfg(not(test))]
fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    logging::init();
    early_init(Arch::dtb_addr(argc, argv));
    phase2_smoke_test();
    memory::init();
    phase3_smoke_test();
    Arch::init_interrupt();
    Arch::init_timer();

    // P5: 任务初始化
    task::init();
    task::spawn_kernel_thread("test_a", test_thread_a, 0);
    task::spawn_kernel_thread("test_b", test_thread_b, 0);

    Arch::wake_secondary_cores();
    phase4_smoke_test();
    phase5_smoke_test();

    // Idle loop — bootstrap 上下文成为 idle 任务
    loop {
        let per_cpu = unsafe { per_cpu::current_per_cpu() };
        if per_cpu.preempt.need_resched.swap(false, core::sync::atomic::Ordering::Acquire) {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
```

- [ ] **Step 2: Modify bootstrap_smp() to use task system**

```rust
#[cfg(not(test))]
fn bootstrap_smp(argc: i32, argv: *const *const u8) -> ! {
    let core_id = Arch::secondary_core_id(argc, argv);
    memory::init_smp();
    Arch::init_interrupt_smp();
    task::init_smp();
    Arch::init_timer_smp(core_id);
    log::info!("SMP: core {} online", core_id);

    // Idle loop
    loop {
        let per_cpu = unsafe { per_cpu::current_per_cpu() };
        if per_cpu.preempt.need_resched.swap(false, core::sync::atomic::Ordering::Acquire) {
            task::schedule();
        }
        core::hint::spin_loop();
    }
}
```

- [ ] **Step 3: Add test threads and phase5_smoke_test**

```rust
#[cfg(not(test))]
fn test_thread_a(_arg: usize) {
    for i in 0..5 {
        log::info!("test_a: iteration {}", i);
        task::yield_now();
    }
    log::info!("test_a: done");
}

#[cfg(not(test))]
fn test_thread_b(_arg: usize) {
    for i in 0..5 {
        log::info!("test_b: iteration {}", i);
        task::yield_now();
    }
    log::info!("test_b: done");
}

#[cfg(not(test))]
fn phase5_smoke_test() {
    log::info!("Phase 5 ready — {} threads spawned", 2);
}
```

- [ ] **Step 4: Modify timer handlers to set need_resched**

`src/arch/riscv64/timer.rs` — 在 `handle_timer` 中 `exit_hardirq()` 之后添加：
```rust
    // 通知 idle loop 检查调度
    per_cpu.preempt.need_resched.store(true, Ordering::Release);
```

`src/arch/aarch64/timer.rs` — 同理。

- [ ] **Step 5: Run `cargo test` — verify host tests still pass**

- [ ] **Step 6: Cross-compile both architectures**

Run: `cargo xtask build --arch riscv64 && cargo xtask build --arch aarch64`

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/arch/riscv64/timer.rs src/arch/aarch64/timer.rs
git commit --signoff -m "feat(P5a): connect task system to bootstrap and timer tick"
```

---

## Task 7: QEMU Smoke Test

- [ ] **Step 1: Run riscv64 in QEMU**

Run: `cargo xtask run --arch riscv64`

Expected output (关键行):
```
TaskInit: idle task created for core 0
Task [pid=1] "test_a" created
Task [pid=2] "test_b" created
Phase 5 ready — 2 threads spawned
test_a: iteration 0
test_b: iteration 0
test_a: iteration 1
test_b: iteration 1
...
test_a: done
Task [pid=1] "test_a" exited with code 0
test_b: done
Task [pid=2] "test_b" exited with code 0
SMP: core 1 online
TaskInitSMP: idle task created for core 1
Tick #10 (core 0)
Tick #10 (core 1)
```

- [ ] **Step 2: Run aarch64 in QEMU**

Run: `cargo xtask run --arch aarch64`
Expected: 类似输出

- [ ] **Step 3: Fix any issues discovered during QEMU testing**

常见问题清单：
- 栈对齐不够 → 检查 KernelStack::top() 的 16 字节对齐
- switch_to 崩溃 → 检查 CalleeSavedContext 初始化与汇编偏移是否对应
- 死锁 → 检查 sched_lock 的 lock_raw/unlock_raw 配对
- page fault → 检查新线程栈是否在 identity-mapped 区域内（堆分配的内存应该在 RAM range 内）

- [ ] **Step 4: Commit final fixes if any**

```bash
git add -u
git commit --signoff -m "fix(P5a): fix issues discovered during QEMU smoke testing"
```

---

## Exit Criteria

| 标准 | 验证方法 |
|------|---------|
| TaskState 单元测试通过 | `cargo test -- state` |
| FifoScheduler 单元测试通过 | `cargo test -- fifo` |
| QEMU riscv64: 多线程交替运行 | test_a/test_b 交替输出 |
| QEMU aarch64: 多线程交替运行 | 同上 |
| 线程退出后不再被调度 | exited 后无该任务日志 |
| 从核正常启动 | "SMP: core 1 online" |
| Timer tick 持续触发 | "Tick #N" 持续递增 |

---

## P5b Scope (Deferred)

P5b 在 P5a 验证通过后单独规划：

- [ ] Round-Robin 调度器（时间片抢占）
- [ ] CFS 调度器（vruntime 公平调度）
- [ ] clone/exit/wait 生命周期
- [ ] sleep（定时挂起 + 唤醒队列）
- [ ] signal（信号传递 + SIGKILL）
- [ ] 阻塞 mutex（竞争时任务阻塞）
- [ ] SMP 负载均衡（per-CPU 队列 + 任务窃取）
- [ ] user_slice（用户空间安全复制）
