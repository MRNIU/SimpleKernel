# sync — 内核同步原语

## 分层架构

```text
Layer 2: 类型别名（公开 API）
  SpinLock<T>    = Mutex<RawSpinLock, T>      不操作中断
  SpinLockIrq<T> = IrqSafe<RawSpinLock, T>    获取时关中断

Layer 1: 组合层（两个独立维度）
  Mutex<R, T>        数据保护 + RAII guard
  IrqSafe<R, T>      Mutex + 关中断 + 锁序检查

Layer 0: 原始锁机制（trait 抽象）
  trait RawLock       acquire / try_acquire / release
  RawSpinLock         TTAS（test-and-test-and-set）实现
```

更换锁算法（TTAS → ticket → MCS）只需新增一个 `impl RawLock`，
Layer 1 和 Layer 2 自动适配。

中断状态管理由 `interrupt_state` crate 提供（`HeldInterrupts` proof token）。

## 核心类型

| 类型 | 用途 | 中断安全 |
|------|------|----------|
| `SpinLock<T>` | 不在中断 handler 中获取的锁 | 否 |
| `SpinLockIrq<T>` | 中断 handler 也会获取的锁（调度锁、堆锁等） | 是 |
| `HeldInterrupts` | 中断禁用的 proof token——`!Copy`, `!Clone`, `!Send` | — |

## 设计决策

### 1. loop-try-reopen：自旋期间恢复中断

竞争失败时**立即恢复中断**，在中断开启状态下自旋，
仅在 CAS 尝试获取的瞬间关闭中断。

```text
┌─ loop ──────────────────────────────────────────────┐
│  HeldInterrupts::hold()     ← 关中断               │
│  try_acquire()              ← CAS 尝试             │
│  ├─ 成功 → set_owner, post_acquire, return guard   │
│  └─ 失败 → drop(held)      ← 恢复中断             │
│            while is_locked() { spin_loop() }        │
└─────────────────────────────────────────────────────┘
```

避免"无界等待 + 全程关中断"的组合：
- 等待期间 timer tick、IPI（如 TLB shootdown）可正常响应
- 消除 Core A 等 Core B 响应 IPI、Core B 关中断等 Core A 释放锁的活锁

TTAS 等待时间无上界，全程关中断不可接受。

### 2. ManuallyDrop 显式析构顺序

`IrqSafeGuard` 析构必须严格遵守：弹出锁栈 → 释放锁 → 恢复中断。
若依赖字段声明顺序（[RFC 1857]），重构时交换字段会**静默破坏**此不变量。
采用 `ManuallyDrop` + 显式 `drop()`，将析构顺序从隐式布局依赖提升为显式代码控制。

[RFC 1857]: https://rust-lang.github.io/rfcs/1857-stabilize-drop-order.html

### 3. try_lock 快速路径

关中断是昂贵操作（保存/恢复 CPU 状态寄存器）。
`IrqSafe::try_lock()` 先用 `Relaxed` load 检查锁状态，
锁已被持有时直接返回 `None`，跳过无谓的中断状态切换。

### 4. MutexGuard 是 !Send

通过 `PhantomData<*mut ()>` 防止 guard 跨核心移动。
持锁任务被调度器迁移到另一个核心后 guard 会在错误的核心释放，
破坏 owner 追踪语义。`IrqSafeGuard` 因包含 `HeldInterrupts`（`!Send`）
已自动获得此保护。

### 5. 锁级别与锁栈

所有 `IrqSafe` 锁都有明确的级别（`lock_level` 常量），
per-CPU `LockStack` 在获取时检查 `new_level > top_level`，违反则 panic。

```rust
pub mod lock_level {
    pub const SCHED: u8 = 0;           // 调度锁——最先获取
    pub const TASK_TABLE: u8 = 1;      // 任务表锁
    pub const INTERRUPT_THREADS: u8 = 2;
    pub const FRAME_ALLOC: u8 = 10;    // 帧分配器锁
    pub const HEAP: u8 = 11;           // 堆分配器锁
    pub const CONSOLE: u8 = 200;       // 控制台锁——几乎可在任何上下文获取
}
```

### 6. 嵌套锁获取

任务窃取等场景需要在已关中断的情况下获取另一核心的同级别锁。
`try_lock_nested` 用 proof token 替代 unsafe 逃生舱：

```rust
let _guard = other_lock.try_lock_nested(&held)?;
```

编译期保证「中断已禁用」，RAII 保证「锁一定释放」。

## 注意事项

- **SpinLock 不可在中断 handler 中使用**——同核心递归加锁会 panic。
  此类场景必须使用 `SpinLockIrq`。
- **RawSpinLock 无公平性保证**——TTAS 不保证 FIFO，
  高争用场景可实现 `RawTicketLock` 等公平算法替换。
