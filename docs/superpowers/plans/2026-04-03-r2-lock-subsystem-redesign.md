<!-- Copyright The SimpleKernel Contributors -->

# R2 Lock Subsystem Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign kernel lock subsystem: SpinLock gains preemption disable + lock ordering + interrupt-context panic; SpinLockIrq requires mandatory lock level; both get `.with()` closure API; spin-timeout feature; lock stack works on host.

**Architecture:** Two lock types map directly to Linux `spin_lock`/`spin_lock_irqsave`. `PreemptGuard` (new) and `HeldInterrupts` (existing) provide execution-context protection as composable RAII tokens in `interrupt_state` crate. Lock stack unified across both lock types. Lazy preemption model (no scheduler coupling).

**Tech Stack:** Rust nightly-2026-03-24, `#![no_std]`, per-CPU via `#[cpu_local]`, `AtomicU32`/`AtomicBool` for counters.

**Conventions (from CLAUDE.md):**
- `git commit --signoff` on every commit
- `#[expect(..., reason = "...")]` not `#[allow(...)]`
- Chinese comments; `// SAFETY:` prefix in English, explanation in Chinese
- `/// ` doc comments with `# Safety`/`# Errors`/`# Panics` sections in English headers, Chinese body
- No `.unwrap()` — use `.expect("reason with data")`
- Every `unsafe` block needs `// SAFETY:` comment

---

## File Structure

### Existing files to modify

| File | Change |
|------|--------|
| `crates/interrupt_state/src/lib.rs` | Add `PreemptGuard`, `PREEMPT_DISABLE_COUNT`, `NEED_RESCHED`, `preemptible()`, `set_need_resched_on()`, `check_and_clear_need_resched()` |
| `crates/config/src/lib.rs` | Add `SPINLOCK_TIMEOUT` constant |
| `crates/sync/src/lib.rs` | Update type aliases, re-exports, lock stack host enablement |
| `crates/sync/src/raw/mod.rs` | No change |
| `crates/sync/src/raw/ttas.rs` | Add spin-timeout logic |
| `crates/sync/src/mutex.rs` | Add lock level + lock stack + `PreemptGuard` + `with()` + `is_in_interrupt` check |
| `crates/sync/src/irq_safe.rs` | Remove `new()` default, keep only mandatory-level `new()`, add `new_unordered()`, add `with()`, remove `#[cfg(target_os = "none")]` gates on lock stack |
| `crates/sync/src/lock_stack.rs` | Add `UNSPECIFIED` constant, update `check_order()` |
| `crates/sync/Cargo.toml` | Add `spin-timeout` feature |
| `src/preempt.rs` | Slim down to re-export from `interrupt_state` |
| `crates/per_cpu/src/lib.rs` | Remove stale `#![feature(sync_unsafe_cell)]` |
| `crates/heap/src/lib.rs` | Remove stale `#![feature(sync_unsafe_cell)]`, update `SpinLock::new()` |

### Downstream call site updates (mechanical)

| File | Change |
|------|--------|
| `src/fs/ramfs.rs:56` | `SpinLock::new(inodes, "ramfs")` → add level |
| `src/fs/mod.rs:32` | `SpinLock::new(Vec::new(), "mount_table")` → add level |
| `src/fs/fd_table.rs:66` | `SpinLock::new(file, "fd")` → add level |
| `src/device/virtio.rs:129` | `sync::SpinLock::new(blk, "virtio_blk")` → add level |
| `src/device/manager.rs:11` | `SpinLock::new(Vec::new(), "dev_mgr")` → add level |
| `src/device/hal.rs:24` | `SpinLock::new(BTreeMap::new(), "dma_tracker")` → add level |
| `src/task/tcb.rs:118,149` | `sync::SpinLock::new(FileDescriptorTable::new(), "fd_table")` → add level |
| `src/panic.rs:36` | `SpinLock::new(ObserverRegistry::new(), "panic_observers")` → add level |
| `src/smoke_test.rs:14,107` | Test SpinLock → add level |
| `src/logging.rs:16` | `SpinLockIrq::new_with_level(...)` → `SpinLockIrq::new(...)` |
| `src/task/sched.rs:31` | `SpinLockIrq::new_with_level(...)` → `SpinLockIrq::new(...)` |
| `src/task/task_table.rs:144` | `SpinLockIrq::new_with_level(...)` → `SpinLockIrq::new(...)` |
| `crates/frame_allocator/src/alloc.rs:41` | `SpinLockIrq::new_with_level(...)` → `SpinLockIrq::new(...)` |
| `crates/page_allocator/src/lib.rs:29` | `SpinLockIrq::new_with_level(...)` → `SpinLockIrq::new(...)` |
| `crates/heap/src/lib.rs:52` | `SpinLock::new(Heap::empty(), "heap")` → add level |
| `tests/system/src/sync_tests.rs` | Update all `SpinLock::new()` calls |

---

## Task 1: Add PreemptGuard to interrupt_state

**Files:**
- Modify: `crates/interrupt_state/src/lib.rs`

This is the foundation — `PreemptGuard` is a proof token for "preemption is disabled", analogous to `HeldInterrupts` for "interrupts are disabled". Also moves `PREEMPT_DISABLE_COUNT`, `NEED_RESCHED`, `preemptible()`, `check_and_clear_need_resched()`, `set_need_resched_on()` from `src/preempt.rs` into this crate.

- [ ] **Step 1: Write tests for PreemptGuard**

Add to the `#[cfg(test)] mod tests` block in `crates/interrupt_state/src/lib.rs`:

```rust
/// PreemptGuard 大小为 0（只有 PhantomData）。
#[test]
fn preempt_guard_is_zst() {
    assert_eq!(core::mem::size_of::<PreemptGuard>(), 0);
}

/// disable 后 preemptible 返回 false。
#[test]
fn preempt_disable_makes_non_preemptible() {
    let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
    assert!(preemptible());
    let guard = PreemptGuard::disable();
    assert!(!preemptible());
    drop(guard);
    assert!(preemptible());
}

/// 嵌套 PreemptGuard 正确处理。
#[test]
fn nested_preempt_guard() {
    let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
    let outer = PreemptGuard::disable();
    assert!(!preemptible());
    let inner = PreemptGuard::disable();
    assert!(!preemptible());
    drop(inner);
    assert!(!preemptible());
    drop(outer);
    assert!(preemptible());
}

/// 中断上下文中不可抢占。
#[test]
fn not_preemptible_in_interrupt() {
    let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
    let irq = HardIrqGuard::enter();
    assert!(!preemptible());
    drop(irq);
    assert!(preemptible());
}

/// check_and_clear_need_resched 原子交换。
#[test]
fn check_and_clear_need_resched_works() {
    let _guard = TEST_LOCK.lock().expect("TEST_LOCK poisoned");
    assert!(!check_and_clear_need_resched());
    NEED_RESCHED.get().store(true, Ordering::Relaxed);
    assert!(check_and_clear_need_resched());
    assert!(!check_and_clear_need_resched());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p interrupt_state -- preempt 2>&1 | head -30`
Expected: compilation errors — `PreemptGuard`, `preemptible`, `check_and_clear_need_resched`, `NEED_RESCHED` not found.

- [ ] **Step 3: Implement PreemptGuard and preemption state**

In `crates/interrupt_state/src/lib.rs`, add after the `SOFTIRQ_COUNT` declaration (around line 119):

```rust
/// 抢占关闭计数（>0 表示抢占被禁用）
#[cpu_local]
static PREEMPT_DISABLE_COUNT: AtomicU32 = AtomicU32::new(0);

/// 是否需要调度（原子：可被其他核心通过 IPI 设置）
#[cpu_local]
pub static NEED_RESCHED: AtomicBool = AtomicBool::new(false);

/// 抢占禁用的 proof token——持有时当前核心不会被调度器切换。
///
/// 与 [`HeldInterrupts`] 类似：`!Copy`, `!Send`, RAII Drop。
/// 但比关中断更轻量——中断仍可响应（timer tick、IPI），
/// 只是调度器检查 [`preemptible()`] 时会跳过。
///
/// 支持嵌套：内层 `disable()` 只增计数，只有最外层 `drop` 时计数才归零。
///
/// # 与 HeldInterrupts 的关系
///
/// 关中断 ⊃ 禁抢占（timer tick 被屏蔽 → 调度器无法介入）。
/// 所以 `SpinLockIrq` 不需要额外的 `PreemptGuard`。
pub struct PreemptGuard {
    _not_send: PhantomData<*const ()>,
}

impl PreemptGuard {
    /// 禁止抢占，返回 proof token。
    #[must_use]
    #[inline]
    pub fn disable() -> Self {
        PREEMPT_DISABLE_COUNT.get().fetch_add(1, Ordering::Relaxed);
        Self {
            _not_send: PhantomData,
        }
    }
}

impl Drop for PreemptGuard {
    #[inline]
    fn drop(&mut self) {
        PREEMPT_DISABLE_COUNT.get().fetch_sub(1, Ordering::Relaxed);
    }
}

/// 当前是否可以抢占——抢占计数为零且不在中断上下文中。
pub fn preemptible() -> bool {
    PREEMPT_DISABLE_COUNT.get().load(Ordering::Relaxed) == 0 && !is_in_interrupt()
}

/// 检查并清除当前核心的 `need_resched` 标志（原子操作，无需关中断）。
///
/// 用于 idle loop 轮询。
pub fn check_and_clear_need_resched() -> bool {
    NEED_RESCHED.get().swap(false, Ordering::Acquire)
}

/// 设置指定核心的 `need_resched` 标志（用于 IPI 跨核唤醒）。
///
/// # Safety
/// `target_core` 必须是有效的核心 ID。
pub unsafe fn set_need_resched_on(target_core: usize) {
    // SAFETY: 调用方保证 target_core 有效
    unsafe { NEED_RESCHED.get_on(target_core) }.store(true, Ordering::Release);
}
```

Also add `use core::sync::atomic::AtomicBool;` to the existing imports at the top if not already present (it's already there for `AtomicU32`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p interrupt_state -v 2>&1 | tail -20`
Expected: all tests pass including the new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/interrupt_state/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(interrupt_state): 添加 PreemptGuard 抢占禁用 proof token

新增抢占控制原语，为 SpinLock 的抢占保护做准备：
- PreemptGuard: RAII proof token（!Copy, !Send, 支持嵌套）
- PREEMPT_DISABLE_COUNT / NEED_RESCHED: per-CPU 状态
- preemptible(): 查询函数
- check_and_clear_need_resched() / set_need_resched_on(): 调度标志操作

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add UNSPECIFIED lock level and update check_order

**Files:**
- Modify: `crates/sync/src/lock_stack.rs`

- [ ] **Step 1: Write tests for UNSPECIFIED level**

Add to the implicit test area (there's no `#[cfg(test)]` module in lock_stack.rs yet). Create one at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// 空栈任何级别都合法。
    #[test]
    fn empty_stack_allows_any_level() {
        let stack = LockStack::new();
        assert!(stack.check_order(lock_level::SCHED));
        assert!(stack.check_order(lock_level::CONSOLE));
        assert!(stack.check_order(lock_level::UNSPECIFIED));
    }

    /// UNSPECIFIED 级别跳过检查。
    #[test]
    fn unspecified_skips_order_check() {
        let mut stack = LockStack::new();
        stack.push(core::ptr::null(), lock_level::CONSOLE);
        // CONSOLE=200 是最高有效级别，但 UNSPECIFIED 应跳过
        assert!(stack.check_order(lock_level::UNSPECIFIED));
    }

    /// 栈顶为 UNSPECIFIED 时，下一个锁也跳过检查。
    #[test]
    fn unspecified_on_top_skips_check() {
        let mut stack = LockStack::new();
        stack.push(core::ptr::null(), lock_level::UNSPECIFIED);
        assert!(stack.check_order(lock_level::SCHED));
    }

    /// 正常级别严格递增。
    #[test]
    fn normal_levels_must_increase() {
        let mut stack = LockStack::new();
        stack.push(core::ptr::null(), lock_level::SCHED);
        assert!(stack.check_order(lock_level::TASK_TABLE));
        assert!(!stack.check_order(lock_level::SCHED));
    }

    /// push/pop 配对。
    #[test]
    fn push_pop_roundtrip() {
        let mut stack = LockStack::new();
        let ptr = 0x1234 as *const ();
        stack.push(ptr, lock_level::SCHED);
        assert_eq!(stack.depth(), 1);
        stack.pop(ptr);
        assert_eq!(stack.depth(), 0);
    }

    /// pop 指针不匹配时 panic。
    #[test]
    #[should_panic(expected = "lock stack corrupted")]
    fn pop_mismatch_panics() {
        let mut stack = LockStack::new();
        stack.push(0x1234 as *const (), lock_level::SCHED);
        stack.pop(0x5678 as *const ());
    }
}
```

- [ ] **Step 2: Run tests to verify UNSPECIFIED tests fail**

Run: `cargo test -p sync -- lock_stack 2>&1 | head -20`
Expected: `lock_level::UNSPECIFIED` not found.

- [ ] **Step 3: Add UNSPECIFIED constant and update check_order**

In `crates/sync/src/lock_stack.rs`:

Add to `lock_level` module:
```rust
/// 不参与锁序检查——调试 / 诊断锁专用。
/// 锁栈对 UNSPECIFIED 级别跳过顺序校验，但仍记录用于诊断。
pub const UNSPECIFIED: u8 = 255;
```

Replace `check_order` method:
```rust
/// 检查锁顺序是否合法。
///
/// - `UNSPECIFIED` 级别始终通过（不参与排序）
/// - 栈顶为 `UNSPECIFIED` 时也跳过检查
/// - 其他情况：新锁级别必须严格大于栈顶级别
///
/// 返回 `true` 表示顺序合法，`false` 表示违反顺序。
#[inline]
pub fn check_order(&self, new_level: u8) -> bool {
    if new_level == lock_level::UNSPECIFIED {
        return true;
    }
    if self.depth == 0 {
        return true;
    }
    let top = self.entries[self.depth - 1].level;
    if top == lock_level::UNSPECIFIED {
        return true;
    }
    new_level > top
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p sync -- lock_stack -v 2>&1 | tail -20`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sync/src/lock_stack.rs
git commit --signoff -m "$(cat <<'EOF'
feat(sync): 添加 UNSPECIFIED 锁级别，锁栈跳过其顺序检查

UNSPECIFIED (255) 用于不关心锁序的诊断锁和尚未定义级别的锁。
锁栈对 UNSPECIFIED 跳过顺序校验，但仍记录用于诊断输出。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Add SPINLOCK_TIMEOUT to config

**Files:**
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: Add SPINLOCK_TIMEOUT constant**

In `crates/config/src/lib.rs`, add after line 81 (`LOCK_STACK_DEPTH`):

```rust
/// 自旋锁超时阈值（循环次数）——超过此值 panic。
///
/// 帮助定位死锁：超时时 panic 信息包含锁名称、owner 核心等诊断数据。
/// 仅在 `spin-timeout` feature 启用时生效。
pub const SPINLOCK_TIMEOUT: u64 = 100_000_000;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p config`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(config): 添加 SPINLOCK_TIMEOUT 自旋锁超时阈值常量

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Restructure SpinLock — preempt guard, lock level, interrupt check, with(), lock stack

This is the core task. `Mutex<R, T>` becomes the internal implementation of `SpinLock<T>`, gaining:
- Lock level parameter
- `PreemptGuard` in guard
- `is_in_interrupt()` check
- Lock stack integration (on all targets, not just bare metal)
- `.with()` closure API

**Files:**
- Modify: `crates/sync/src/mutex.rs`
- Modify: `crates/sync/src/lib.rs`

- [ ] **Step 1: Update Mutex to support lock levels and PreemptGuard**

Rewrite `crates/sync/src/mutex.rs`:

```rust
//! 自旋互斥锁——禁抢占 + 数据保护 + RAII guard + 锁序检查。

use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use interrupt_state::PreemptGuard;

use crate::lock_stack::lock_level;
use crate::raw::{RawLock, RawSpinLock};

/// 自旋锁——持锁期间禁止抢占，中断仍可响应。
///
/// 适用于：任务间共享数据，中断 handler 不访问。
/// 不适用于：中断 handler 也会获取的数据（用 [`SpinLockIrq`](crate::SpinLockIrq)）。
///
/// # 保护机制
///
/// - 禁止抢占 → 防止同核心另一任务竞争
/// - 禁止迁移 → 抢占禁用时调度器不介入，自然不迁移
/// - owner 追踪 → 检测递归加锁
/// - 锁序检查 → 防止 ABBA 死锁
///
/// # Panics
///
/// - 同一核心递归加锁
/// - 在中断上下文中调用（应使用 `SpinLockIrq`）
/// - 锁级别顺序违反
pub struct Mutex<R: RawLock, T> {
    pub(crate) raw: R,
    pub(crate) data: UnsafeCell<T>,
    level: u8,
}

// SAFETY: Mutex 通过 RawLock 的原子操作保证互斥访问。
// T: Send 即可安全跨线程——锁提供独占访问，不需要 T: Sync。
unsafe impl<R: RawLock, T: Send> Send for Mutex<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for Mutex<R, T> {}

impl<T> Mutex<RawSpinLock, T> {
    /// 创建自旋锁，指定锁级别用于锁序检查。
    ///
    /// 尚未确定锁序关系的锁使用 [`lock_level::UNSPECIFIED`]。
    #[must_use]
    pub const fn new(data: T, name: &'static str, level: u8) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
            level,
        }
    }
}

impl<R: RawLock, T> Mutex<R, T> {
    /// 获取锁，返回 RAII guard。
    ///
    /// # Panics
    ///
    /// - 在中断上下文中调用
    /// - 同一核心递归加锁
    /// - 锁级别顺序违反
    pub fn lock(&self) -> MutexGuard<'_, R, T> {
        #[cfg(target_os = "none")]
        assert!(
            !interrupt_state::is_in_interrupt(),
            "SpinLock '{}': 在中断上下文中调用，应使用 SpinLockIrq",
            self.raw.name(),
        );

        let preempt = PreemptGuard::disable();
        self.raw.check_recursive();
        self.raw.acquire();
        self.raw.set_owner();
        self.push_lock_stack();
        MutexGuard {
            mutex: self,
            preempt,
            _not_send: PhantomData,
        }
    }

    /// 尝试获取锁，不阻塞。
    ///
    /// # Panics
    ///
    /// 在中断上下文中调用。
    pub fn try_lock(&self) -> Option<MutexGuard<'_, R, T>> {
        #[cfg(target_os = "none")]
        assert!(
            !interrupt_state::is_in_interrupt(),
            "SpinLock '{}': 在中断上下文中调用，应使用 SpinLockIrq",
            self.raw.name(),
        );

        let preempt = PreemptGuard::disable();
        if self.raw.try_acquire() {
            self.raw.set_owner();
            self.push_lock_stack();
            Some(MutexGuard {
                mutex: self,
                preempt,
                _not_send: PhantomData,
            })
        } else {
            None
        }
    }

    /// 闭包方式访问——临界区 = 闭包体，自动获取和释放。
    pub fn with<Ret>(&self, f: impl FnOnce(&mut T) -> Ret) -> Ret {
        let mut guard = self.lock();
        f(&mut *guard)
    }

    /// 查询锁是否被持有。
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 锁名称（诊断用）。
    pub fn name(&self) -> &'static str {
        self.raw.name()
    }

    fn push_lock_stack(&self) {
        // 短暂关中断保护 per-CPU 锁栈访问
        let _held = interrupt_state::HeldInterrupts::hold();
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        if !stack.check_order(self.level) {
            panic!(
                "SpinLock '{}': lock order violation (level={})",
                self.raw.name(),
                self.level,
            );
        }
        stack.push(self as *const Self as *const (), self.level);
    }

    fn pop_lock_stack(&self) {
        let _held = interrupt_state::HeldInterrupts::hold();
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        stack.pop(self as *const Self as *const ());
    }
}

impl<R: RawLock, T> fmt::Debug for Mutex<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mutex")
            .field("name", &self.raw.name())
            .field("locked", &self.raw.is_locked())
            .field("level", &self.level)
            .finish()
    }
}

/// RAII guard——丢弃时释放锁并恢复抢占。
///
/// 析构顺序：弹出锁栈 → 清除 owner → 释放锁 → 恢复抢占。
/// `!Send`——guard 必须在获取锁的同一核心上释放。
pub struct MutexGuard<'a, R: RawLock, T> {
    mutex: &'a Mutex<R, T>,
    preempt: PreemptGuard,
    /// `*mut ()` 是 `!Send`——使整个 guard 也变为 `!Send`。
    _not_send: PhantomData<*mut ()>,
}

impl<R: RawLock, T> Deref for MutexGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.mutex.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for MutexGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<R: RawLock, T> Drop for MutexGuard<'_, R, T> {
    fn drop(&mut self) {
        self.mutex.pop_lock_stack();
        self.mutex.raw.clear_owner();
        self.mutex.raw.release();
        // self.preempt 自动 drop → 恢复抢占
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for MutexGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock_stack::lock_level;

    type SpinLock<T> = Mutex<RawSpinLock, T>;

    /// 基本的加锁/解锁流程
    #[test]
    fn lock_and_unlock() {
        let lock = SpinLock::new(42u32, "test", lock_level::UNSPECIFIED);
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// guard 提供可变访问
    #[test]
    fn guard_provides_mutable_access() {
        let lock = SpinLock::new(0u32, "test_mut", lock_level::UNSPECIFIED);
        {
            let mut guard = lock.lock();
            *guard = 99;
        }
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }

    /// try_lock 在锁空闲时成功
    #[test]
    fn try_lock_succeeds_when_free() {
        let lock = SpinLock::new(7u32, "try_lock_test", lock_level::UNSPECIFIED);
        let guard = lock.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.expect("lock should succeed"), 7);
    }

    /// try_lock 在锁已持有时失败
    #[test]
    fn try_lock_fails_when_held() {
        let lock = SpinLock::new(0u32, "try_lock_held", lock_level::UNSPECIFIED);
        let _g = lock.lock();
        let second = lock.try_lock();
        assert!(second.is_none());
    }

    /// guard 析构时自动释放锁
    #[test]
    fn guard_drop_releases_lock() {
        let lock = SpinLock::new(0u32, "drop_test", lock_level::UNSPECIFIED);
        {
            let _g = lock.lock();
            assert!(lock.is_locked());
        }
        assert!(!lock.is_locked());
    }

    /// with 闭包 API
    #[test]
    fn with_closure_api() {
        let lock = SpinLock::new(0u32, "with_test", lock_level::UNSPECIFIED);
        lock.with(|data| *data = 42);
        let val = lock.with(|data| *data);
        assert_eq!(val, 42);
    }

    /// 多线程并发自增验证互斥正确性
    #[test]
    fn concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(SpinLock::new(0u64, "concurrent", lock_level::UNSPECIFIED));
        let mut handles = Vec::new();

        for _ in 0..4 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut g = lock.lock();
                    *g += 1;
                }
            }));
        }

        for h in handles {
            h.join().expect("线程应正常结束");
        }

        let g = lock.lock();
        assert_eq!(*g, 4000, "并发计数器最终值应为 4000");
    }

    /// 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn recursive_lock_panics() {
        let lock = SpinLock::new(0u32, "recursive", lock_level::UNSPECIFIED);
        let _g = lock.lock();
        let _g2 = lock.lock();
    }
}
```

- [ ] **Step 2: Update lib.rs type aliases**

In `crates/sync/src/lib.rs`, update the `SpinLock`/`SpinLockGuard` aliases and re-exports:

```rust
//! 内核同步原语——分层自旋锁、中断安全锁和锁序检查。
//!
//! 架构设计详见 `crates/sync/README.md`。

#![cfg_attr(not(test), no_std)]

pub(crate) mod irq_safe;
pub mod lock_stack;
pub(crate) mod mutex;
pub(crate) mod raw;

use per_cpu::cpu_local;

/// Per-CPU 锁栈——强制锁获取顺序，防止死锁。
#[cpu_local]
pub static LOCK_STACK: lock_stack::LockStack = lock_stack::LockStack::new();

/// 自旋锁——禁抢占 + 自旋，不操作中断。
pub type SpinLock<T> = mutex::Mutex<raw::RawSpinLock, T>;

/// 自旋锁 RAII guard。
pub type SpinLockGuard<'a, T> = mutex::MutexGuard<'a, raw::RawSpinLock, T>;

/// 中断安全的自旋锁。
pub type SpinLockIrq<T> = irq_safe::IrqSafe<raw::RawSpinLock, T>;

/// 中断安全的自旋锁 RAII guard。
pub type SpinLockIrqGuard<'a, T> = irq_safe::IrqSafeGuard<'a, raw::RawSpinLock, T>;

pub use interrupt_state::HeldInterrupts;
pub use lock_stack::lock_level;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p sync -- mutex -v 2>&1 | tail -20`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/sync/src/mutex.rs crates/sync/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(sync): SpinLock 添加抢占禁用、锁级别、中断上下文检查和闭包 API

SpinLock::new() 现在要求指定锁级别。获取锁时：
- 禁止抢占（PreemptGuard）
- 检测中断上下文误用（裸机环境 panic）
- 纳入统一锁栈进行锁序检查
- 提供 .with() 闭包 API

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Restructure SpinLockIrq — mandatory level, remove default new(), with(), host lock stack

**Files:**
- Modify: `crates/sync/src/irq_safe.rs`

- [ ] **Step 1: Restructure IrqSafe — mandatory level, new_unordered, with(), remove cfg gates on lock stack**

In `crates/sync/src/irq_safe.rs`, apply the following changes:

**a)** Replace both `impl<T> IrqSafe<RawSpinLock, T>` constructor blocks (lines 24-43) with:

```rust
impl<T> IrqSafe<RawSpinLock, T> {
    /// 创建中断安全锁——必须指定锁级别。
    #[must_use]
    pub const fn new(data: T, name: &'static str, level: u8) -> Self {
        Self {
            mutex: Mutex::new(data, name, level),
            level,
        }
    }

    /// 创建不参与锁序检查的中断安全锁——诊断/调试用。
    #[must_use]
    pub const fn new_unordered(data: T, name: &'static str) -> Self {
        Self::new(data, name, lock_level::UNSPECIFIED)
    }
}
```

Note: `Mutex::new()` now takes 3 args. The `level` is stored in both `IrqSafe` and the inner `Mutex` (IrqSafe's own lock stack logic uses `self.level`, but the inner mutex's level is not used by IrqSafe since IrqSafe has its own push/pop — set inner mutex level to the same value for consistency).

Wait, actually looking more carefully: `Mutex::new()` now requires a level. But `IrqSafe` has its own `post_acquire`/`pop_lock_stack` that bypasses Mutex's lock stack. The inner Mutex's `lock()` is never called directly by IrqSafe — IrqSafe calls `self.mutex.try_lock()` which goes through Mutex's `try_lock()`. But now Mutex's try_lock() also does `push_lock_stack`... this is a problem.

Let me reconsider. IrqSafe uses `self.mutex.try_lock()` internally, but IrqSafe has its OWN lock stack logic in `post_acquire()`. If Mutex's `try_lock()` also pushes to lock stack, we'd get double pushes.

The solution: IrqSafe should use the raw lock directly, not go through Mutex's lock/try_lock (which now includes preempt guard and lock stack). IrqSafe needs access to the raw lock and data cell directly.

Let me redesign: IrqSafe should compose `RawLock` + `UnsafeCell<T>` directly (not wrap Mutex). This is actually cleaner.

**Revised `irq_safe.rs`:**

```rust
//! 中断安全锁——关中断 + 自旋 + 锁序检查。

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

use crate::lock_stack::lock_level;
use crate::raw::{RawLock, RawSpinLock};
use interrupt_state::HeldInterrupts;

/// 中断安全锁——获取时禁用中断，释放时恢复。
pub struct IrqSafe<R: RawLock, T> {
    raw: R,
    data: UnsafeCell<T>,
    level: u8,
}

// SAFETY: 锁通过 RawLock 原子操作保证互斥访问。
// T: Send 即可——锁提供独占访问，不需要 T: Sync。
unsafe impl<R: RawLock, T: Send> Send for IrqSafe<R, T> {}
unsafe impl<R: RawLock, T: Send> Sync for IrqSafe<R, T> {}

impl<T> IrqSafe<RawSpinLock, T> {
    /// 创建中断安全锁——必须指定锁级别。
    #[must_use]
    pub const fn new(data: T, name: &'static str, level: u8) -> Self {
        Self {
            raw: RawSpinLock::new(name),
            data: UnsafeCell::new(data),
            level,
        }
    }

    /// 创建不参与锁序检查的中断安全锁——诊断 / 调试用。
    #[must_use]
    pub const fn new_unordered(data: T, name: &'static str) -> Self {
        Self::new(data, name, lock_level::UNSPECIFIED)
    }
}

impl<R: RawLock, T> IrqSafe<R, T> {
    /// 获取锁，返回 RAII guard（loop-try-reopen 模式）。
    ///
    /// # Panics
    /// 同一核心递归加锁，或锁级别顺序违反。
    pub fn lock(&self) -> IrqSafeGuard<'_, R, T> {
        loop {
            let held = HeldInterrupts::hold();

            // 关中断后检测：保证 per-CPU owner_core 与当前核心一致
            self.raw.check_recursive();

            if self.raw.try_acquire() {
                self.raw.set_owner();
                self.post_acquire();
                return IrqSafeGuard {
                    irq_safe: self,
                    held: ManuallyDrop::new(held),
                };
            }

            drop(held);

            while self.raw.is_locked() {
                core::hint::spin_loop();
            }
        }
    }

    /// 尝试获取锁，不阻塞。
    pub fn try_lock(&self) -> Option<IrqSafeGuard<'_, R, T>> {
        if self.raw.is_locked() {
            return None;
        }

        let held = HeldInterrupts::hold();

        if self.raw.try_acquire() {
            self.raw.set_owner();
            self.post_acquire();
            Some(IrqSafeGuard {
                irq_safe: self,
                held: ManuallyDrop::new(held),
            })
        } else {
            None
        }
    }

    /// 闭包方式访问——临界区 = 闭包体，自动获取和释放。
    pub fn with<Ret>(&self, f: impl FnOnce(&mut T) -> Ret) -> Ret {
        let mut guard = self.lock();
        f(&mut *guard)
    }

    /// 嵌套获取——调用者已关中断，用 proof token 证明。
    ///
    /// 跳过中断管理和锁序检查，保留 RAII 自动释放。
    /// 典型场景：任务窃取时获取另一核心的同级别调度锁。
    pub fn try_lock_nested(&self, _proof: &HeldInterrupts) -> Option<IrqSafeNestedGuard<'_, R, T>> {
        if self.raw.try_acquire() {
            self.raw.set_owner();
            Some(IrqSafeNestedGuard { irq_safe: self })
        } else {
            None
        }
    }

    /// 查询锁是否被持有。
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// 锁名称（诊断用）。
    pub fn name(&self) -> &'static str {
        self.raw.name()
    }

    fn post_acquire(&self) {
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        if !stack.check_order(self.level) {
            panic!(
                "SpinLockIrq '{}': lock order violation (level={})",
                self.raw.name(),
                self.level,
            );
        }
        stack.push(self as *const Self as *const (), self.level);
    }

    fn pop_lock_stack(&self) {
        // SAFETY: 中断已禁用，无同核心并发访问
        let stack = unsafe { crate::LOCK_STACK.get_mut() };
        stack.pop(self as *const Self as *const ());
    }
}

impl<R: RawLock, T> fmt::Debug for IrqSafe<R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IrqSafe")
            .field("name", &self.raw.name())
            .field("locked", &self.raw.is_locked())
            .field("level", &self.level)
            .finish()
    }
}

/// RAII guard（中断安全版）——析构顺序由 `ManuallyDrop` 显式控制。
pub struct IrqSafeGuard<'a, R: RawLock, T> {
    irq_safe: &'a IrqSafe<R, T>,
    held: ManuallyDrop<HeldInterrupts>,
}

impl<R: RawLock, T> Deref for IrqSafeGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> Drop for IrqSafeGuard<'_, R, T> {
    fn drop(&mut self) {
        // 1. 弹出锁栈（中断仍禁用，per-CPU 访问安全）
        self.irq_safe.pop_lock_stack();

        // 2. 释放锁（clear_owner + release）
        self.irq_safe.raw.clear_owner();
        self.irq_safe.raw.release();

        // 3. 恢复中断（若获取前中断已启用）
        // SAFETY: held 在此之后不再被访问，且仅 drop 一次
        unsafe { ManuallyDrop::drop(&mut self.held) };
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for IrqSafeGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/// 嵌套锁 RAII guard——不持有 `HeldInterrupts`（调用者负责中断管理）。
///
/// 由 [`IrqSafe::try_lock_nested`] 返回，在析构时自动释放锁。
pub struct IrqSafeNestedGuard<'a, R: RawLock, T> {
    irq_safe: &'a IrqSafe<R, T>,
}

impl<R: RawLock, T> Deref for IrqSafeNestedGuard<'_, R, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &*self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> DerefMut for IrqSafeNestedGuard<'_, R, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: guard 持有锁，独占访问
        unsafe { &mut *self.irq_safe.data.get() }
    }
}

impl<R: RawLock, T> Drop for IrqSafeNestedGuard<'_, R, T> {
    fn drop(&mut self) {
        self.irq_safe.raw.clear_owner();
        self.irq_safe.raw.release();
    }
}

impl<R: RawLock, T: fmt::Debug> fmt::Debug for IrqSafeNestedGuard<'_, R, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type SpinLockIrq<T> = IrqSafe<RawSpinLock, T>;

    /// 基本加锁/解锁流程
    #[test]
    fn irq_lock_and_unlock() {
        let lock = SpinLockIrq::new(42u32, "irq_test", lock_level::UNSPECIFIED);
        {
            let guard = lock.lock();
            assert_eq!(*guard, 42);
        }
        assert!(!lock.is_locked());
    }

    /// try_lock 成功
    #[test]
    fn irq_try_lock() {
        let lock = SpinLockIrq::new(0u32, "irq_try", lock_level::UNSPECIFIED);
        assert!(lock.try_lock().is_some());
    }

    /// try_lock 在锁已持有时失败
    #[test]
    fn irq_try_lock_fails_when_held() {
        let lock = SpinLockIrq::new(0u32, "irq_try_held", lock_level::UNSPECIFIED);
        let _g = lock.lock();
        assert!(lock.try_lock().is_none());
    }

    /// 带锁级别的构造
    #[test]
    fn irq_new_with_level() {
        let lock = SpinLockIrq::new(0u32, "leveled", lock_level::SCHED);
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

    /// new_unordered
    #[test]
    fn irq_new_unordered() {
        let lock = SpinLockIrq::new_unordered(0u32, "unordered");
        let _g = lock.lock();
        assert!(lock.is_locked());
    }

    /// with 闭包 API
    #[test]
    fn irq_with_closure() {
        let lock = SpinLockIrq::new(0u32, "irq_with", lock_level::UNSPECIFIED);
        lock.with(|data| *data = 42);
        let val = lock.with(|data| *data);
        assert_eq!(val, 42);
    }

    /// 多线程并发访问
    #[test]
    fn irq_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let lock = Arc::new(SpinLockIrq::new(
            Vec::<usize>::new(),
            "irq_concurrent",
            lock_level::UNSPECIFIED,
        ));
        let mut handles = Vec::new();

        for i in 0..4 {
            let lock = Arc::clone(&lock);
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    let mut g = lock.lock();
                    g.push(i * 100 + j);
                }
            }));
        }

        for h in handles {
            h.join().expect("线程应正常结束");
        }

        let g = lock.lock();
        assert_eq!(g.len(), 400, "应有 4x100=400 个元素");
    }

    /// 嵌套锁获取与自动释放
    #[test]
    fn try_lock_nested_and_drop() {
        let lock = SpinLockIrq::new((), "nested_test", lock_level::UNSPECIFIED);
        let held = HeldInterrupts::hold();
        {
            let guard = lock.try_lock_nested(&held);
            assert!(guard.is_some());
            assert!(lock.is_locked());
        }
        assert!(!lock.is_locked());
        drop(held);
    }

    /// 嵌套锁在锁已持有时获取失败
    #[test]
    fn try_lock_nested_fails_when_held() {
        let lock = SpinLockIrq::new((), "nested_fail", lock_level::UNSPECIFIED);
        let _g = lock.lock();
        let held = HeldInterrupts::hold();
        assert!(lock.try_lock_nested(&held).is_none());
    }

    /// 递归加锁应 panic
    #[test]
    #[should_panic(expected = "recursive lock")]
    fn irq_recursive_lock_panics() {
        let lock = SpinLockIrq::new(0u32, "irq_recursive", lock_level::UNSPECIFIED);
        let _g = lock.lock();
        let _g2 = lock.lock();
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p sync -v 2>&1 | tail -30`
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/sync/src/irq_safe.rs
git commit --signoff -m "$(cat <<'EOF'
refactor(sync): SpinLockIrq 必须指定锁级别，移除默认构造器

- new() 现在要求 level 参数（原 new_with_level 语义）
- 新增 new_unordered() 用于不关心锁序的场景
- IrqSafe 直接组合 RawLock + UnsafeCell（不再包装 Mutex）
- 移除锁栈的 #[cfg(target_os = "none")] 门控，宿主机测试也启用
- 新增 .with() 闭包 API

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Add spin-timeout feature

**Files:**
- Modify: `crates/sync/Cargo.toml`
- Modify: `crates/sync/src/raw/ttas.rs`

- [ ] **Step 1: Add feature to Cargo.toml**

In `crates/sync/Cargo.toml`, add:

```toml
[features]
spin-timeout = []
```

- [ ] **Step 2: Add timeout logic to RawSpinLock::acquire**

In `crates/sync/src/raw/ttas.rs`, replace the `acquire` method:

```rust
#[inline]
fn acquire(&self) {
    #[cfg(feature = "spin-timeout")]
    let mut spin_count: u64 = 0;

    while self
        .locked
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        while self.locked.load(Ordering::Relaxed) {
            core::hint::spin_loop();

            #[cfg(feature = "spin-timeout")]
            {
                spin_count += 1;
                if spin_count >= config::SPINLOCK_TIMEOUT {
                    panic!(
                        "SpinLock '{}': spin timeout after {} iterations \
                         (owner_core={}, current_core={})",
                        self.name,
                        spin_count,
                        self.owner_core.load(Ordering::Relaxed),
                        per_cpu::current_core_id(),
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify compilation with and without feature**

Run: `cargo check -p sync && cargo check -p sync --features spin-timeout`
Expected: both succeed.

- [ ] **Step 4: Commit**

```bash
git add crates/sync/Cargo.toml crates/sync/src/raw/ttas.rs
git commit --signoff -m "$(cat <<'EOF'
feat(sync): 添加 spin-timeout feature，自旋超时时 panic

启用 spin-timeout 后，自旋锁超过 config::SPINLOCK_TIMEOUT 次循环
未获取到锁时 panic，输出锁名称、owner 核心等诊断信息。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Update all downstream call sites

**Files:** All files listed in the "Downstream call site updates" table above.

This is a mechanical task — update constructor signatures.

- `SpinLock::new(data, name)` → `SpinLock::new(data, name, lock_level::UNSPECIFIED)`
  (proper levels to be determined in R5/R6 audits)
- `SpinLockIrq::new_with_level(data, name, level)` → `SpinLockIrq::new(data, name, level)`

- [ ] **Step 1: Update kernel SpinLock call sites**

Update each `SpinLock::new(data, name)` to `SpinLock::new(data, name, sync::lock_level::UNSPECIFIED)`:

- `src/fs/ramfs.rs:56` — `SpinLock::new(inodes, "ramfs", sync::lock_level::UNSPECIFIED)`
- `src/fs/mod.rs:32` — `SpinLock::new(Vec::new(), "mount_table", sync::lock_level::UNSPECIFIED)`
- `src/fs/fd_table.rs:66` — `SpinLock::new(file, "fd", sync::lock_level::UNSPECIFIED)` (add `use sync::lock_level;`)
- `src/device/virtio.rs:129` — `sync::SpinLock::new(blk, "virtio_blk", sync::lock_level::UNSPECIFIED)`
- `src/device/manager.rs:11` — `SpinLock::new(Vec::new(), "dev_mgr", sync::lock_level::UNSPECIFIED)` (add `use sync::lock_level;`)
- `src/device/hal.rs:24` — `SpinLock::new(BTreeMap::new(), "dma_tracker", sync::lock_level::UNSPECIFIED)` (add `use sync::lock_level;`)
- `src/task/tcb.rs:118,149` — `sync::SpinLock::new(FileDescriptorTable::new(), "fd_table", sync::lock_level::UNSPECIFIED)`
- `src/panic.rs:36` — `SpinLock::new(ObserverRegistry::new(), "panic_observers", sync::lock_level::UNSPECIFIED)` (add `use sync::lock_level;`)
- `src/smoke_test.rs:14` — `SpinLock::new(42u32, "smoke_test", sync::lock_level::UNSPECIFIED)` (add `use sync::lock_level;`)
- `src/smoke_test.rs:107` — `SpinLock::new(0, "test_counter", sync::lock_level::UNSPECIFIED)`

- [ ] **Step 2: Update SpinLockIrq call sites**

Rename `new_with_level` → `new`:

- `src/logging.rs:16` — `SpinLockIrq::new((), "console", sync::lock_level::CONSOLE)`
- `src/task/sched.rs:31` — `SpinLockIrq::new((), "sched", lock_level::SCHED)`
- `src/task/task_table.rs:144` — `SpinLockIrq::new(TaskTable::EMPTY, "task_table", lock_level::TASK_TABLE)`

- [ ] **Step 3: Update crate call sites**

- `crates/heap/src/lib.rs:52` — `SpinLock::new(Heap::empty(), "heap", sync_crate::lock_level::UNSPECIFIED)`
- `crates/frame_allocator/src/alloc.rs:41-44` — `SpinLockIrq::new(FrameAllocatorInner::new(), "frame_alloc", sync_crate::lock_level::FRAME_ALLOC)`
- `crates/page_allocator/src/lib.rs:29-32` — `SpinLockIrq::new(PageAllocatorInner::new(), "page_alloc", sync_crate::lock_level::PAGE_ALLOC)`

- [ ] **Step 4: Update system test**

In `tests/system/src/sync_tests.rs`:

```rust
//! 同步原语测试——验证 SpinLock 基本功能。

use crate::framework::TestCase;
use sync::lock_level;
use sync::SpinLock;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "spinlock_basic",
            run: test_spinlock_basic,
        },
        TestCase {
            name: "spinlock_modify",
            run: test_spinlock_modify,
        },
        TestCase {
            name: "spinlock_not_held_after_drop",
            run: test_spinlock_not_held_after_drop,
        },
    ]
}

fn test_spinlock_basic() {
    let lock = SpinLock::new(42u32, "test_basic", lock_level::UNSPECIFIED);
    let guard = lock.lock();
    assert_eq!(*guard, 42);
}

fn test_spinlock_modify() {
    let lock = SpinLock::new(0u32, "test_modify", lock_level::UNSPECIFIED);
    {
        let mut guard = lock.lock();
        *guard = 99;
    }
    {
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }
}

fn test_spinlock_not_held_after_drop() {
    let lock = SpinLock::new(0u32, "test_drop", lock_level::UNSPECIFIED);
    {
        let _guard = lock.lock();
    }
    assert!(!lock.is_locked());
}
```

- [ ] **Step 5: Build check all targets**

Run: `cargo check 2>&1 | tail -5`
Expected: success (no errors).

- [ ] **Step 6: Run unit tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/ crates/heap/ crates/frame_allocator/ crates/page_allocator/ tests/
git commit --signoff -m "$(cat <<'EOF'
refactor: 更新所有 SpinLock/SpinLockIrq 调用方适配新 API

- SpinLock::new() 添加 lock_level::UNSPECIFIED（具体级别待各模块审计时确定）
- SpinLockIrq::new_with_level() 改为 SpinLockIrq::new()

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Slim down src/preempt.rs

**Files:**
- Modify: `src/preempt.rs`

`PREEMPT_DISABLE_COUNT`、`NEED_RESCHED`、`preemptible()`、`check_and_clear_need_resched()` 已移入 `interrupt_state`。`src/preempt.rs` 改为 re-export + 保留 `set_need_resched_on()`（如果它使用了内核内部 API）或也 re-export。

- [ ] **Step 1: Rewrite src/preempt.rs**

```rust
//! 抢占控制——re-export `interrupt_state` 的抢占原语。
//!
//! 抢占状态（`PREEMPT_DISABLE_COUNT`、`NEED_RESCHED`）及其操作函数
//! 定义在 `interrupt_state` crate 中，供 `sync` crate 直接访问。
//! 本模块为内核代码提供便捷的 re-export。

pub use interrupt_state::NEED_RESCHED;
pub use interrupt_state::check_and_clear_need_resched;
pub use interrupt_state::preemptible;
pub use interrupt_state::set_need_resched_on;
```

- [ ] **Step 2: Update internal references**

Search for `crate::preempt::` in the kernel source and update to use either the re-export path or `interrupt_state::` directly. Key files:

- `src/timer.rs` — `crate::preempt::NEED_RESCHED` → stays via re-export
- `src/task/sched.rs` — any references to `crate::preempt::NEED_RESCHED`
- `src/task/mod.rs` — `crate::preempt::preemptible()` if any

- [ ] **Step 3: Build check**

Run: `cargo check 2>&1 | tail -5`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add src/preempt.rs src/timer.rs src/task/
git commit --signoff -m "$(cat <<'EOF'
refactor(preempt): 精简为 interrupt_state 的 re-export

抢占状态和操作函数已移入 interrupt_state crate，
src/preempt.rs 改为 re-export 层。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: Cleanup — stale feature flags, allow → expect

**Files:**
- Modify: `crates/per_cpu/src/lib.rs`
- Modify: `crates/heap/src/lib.rs`

- [ ] **Step 1: Remove stale sync_unsafe_cell feature flags**

In `crates/per_cpu/src/lib.rs:20`, remove:
```rust
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]
```

In `crates/heap/src/lib.rs:9`, remove:
```rust
#![feature(sync_unsafe_cell)]
```

- [ ] **Step 2: Fix #[allow] → #[expect] in per_cpu**

In `crates/per_cpu/src/lib.rs:287`, replace:
```rust
#[allow(clippy::mut_from_ref)] // 故意的 per-CPU 内部可变性：每个 CPU 拥有独立副本
```
with:
```rust
#[expect(clippy::mut_from_ref, reason = "per-CPU 内部可变性：每个 CPU 拥有独立副本")]
```

- [ ] **Step 3: Build + test**

Run: `cargo check && cargo test 2>&1 | tail -10`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/per_cpu/src/lib.rs crates/heap/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
fix: 移除已稳定的 sync_unsafe_cell feature flag，#[allow] 改为 #[expect]

- per_cpu/heap: SyncUnsafeCell 已在 Rust 1.82 稳定，移除 feature gate
- per_cpu: #[allow(clippy::mut_from_ref)] 改为 #[expect] + reason

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: Update documentation

**Files:**
- Modify: `crates/sync/README.md`
- Modify: `crates/interrupt_state/README.md`
- Modify: `crates/per_cpu/README.md`

- [ ] **Step 1: Update sync/README.md**

Key changes:
- Update architecture diagram to show PreemptGuard in SpinLock
- Update core types table: SpinLock now disables preemption + has lock level
- Update lock level table: add PAGE_ALLOC and UNSPECIFIED
- Add `.with()` closure API example
- Add `is_in_interrupt()` check documentation
- Remove `new_with_level` references (now just `new`)

- [ ] **Step 2: Update interrupt_state/README.md**

Key changes:
- Add `PreemptGuard` to public API section
- Add preemption functions (`preemptible()`, `check_and_clear_need_resched()`, etc.)
- Fix module structure diagram (`arch/` directory, not `held.rs`/`arch.rs`)
- Update "与 sync crate 的关系" section

- [ ] **Step 3: Update per_cpu/README.md**

Key changes:
- Remove phantom API listings (`in_interrupt()`, `preemptible()`, `enter_hardirq()`, etc.)
- Only list API that actually exists in the crate: `percpu_init()`, `percpu_init_smp()`, `current_core_id()`, `CpuLocal<T>`
- Update per-CPU variable table: only `CORE_ID` is in this crate; note others are in their respective crates

- [ ] **Step 4: Commit**

```bash
git add crates/sync/README.md crates/interrupt_state/README.md crates/per_cpu/README.md
git commit --signoff -m "$(cat <<'EOF'
docs(R2): 更新 sync/interrupt_state/per_cpu README

- sync: 反映 PreemptGuard、必选锁级别、闭包 API、中断上下文检查
- interrupt_state: 添加 PreemptGuard 文档，修正模块结构图
- per_cpu: 移除不属于本 crate 的幽灵 API

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Run system tests in QEMU

**Files:** None (verification only)

- [ ] **Step 1: Run system tests for riscv64**

Run: `cargo xtask test --arch riscv64 2>&1 | tail -20`
Expected: all tests pass.

- [ ] **Step 2: Run system tests for aarch64**

Run: `cargo xtask test --arch aarch64 2>&1 | tail -20`
Expected: all tests pass.

- [ ] **Step 3: Run unit tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -- -D warnings 2>&1 | tail -10`
Expected: no warnings.
