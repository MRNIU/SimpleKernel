# sync

内核同步原语——分层架构的自旋锁、中断安全锁和 HeldInterrupts proof token。

## 概览

`sync` 将互斥、中断控制、锁序检查三个正交关注点分层组合，
避免在每种锁类型中重复实现 Deref/Drop/guard 逻辑。

分层架构：

```text
Layer 2: 类型别名（用户接触的具体类型）
  SpinLock<T>    = Mutex<RawSpinLock, T>
  SpinLockIrq<T> = IrqSafe<RawSpinLock, T>

Layer 1: 组合层（两个独立维度）
  Mutex<R, T>        — 数据保护 + RAII guard
  IrqSafe<R, T>      — Mutex + 关中断 + 锁序检查

Layer 0: 原始锁机制（trait 抽象）
  trait RawLock       — acquire / try_acquire / release
  RawSpinLock         — TTAS（test-and-test-and-set）实现
```

更换锁算法（TTAS → ticket → MCS）只需新增一个 `impl RawLock`，
Layer 1 和 Layer 2 自动适配。

## 核心类型

| 类型 | 用途 | 中断安全 |
|------|------|----------|
| `SpinLock<T>` | 不在中断 handler 中获取的锁 | 否 |
| `SpinLockIrq<T>` | 中断 handler 也会获取的锁（调度锁、堆锁等） | 是 |
| `HeldInterrupts` | 中断禁用的 proof token——`!Copy`, `!Clone`, `!Send` | — |

`HeldInterrupts` 借鉴 Theseus OS 的 intralingual 设计：
持有此类型即证明中断已被禁用，函数签名可要求 `&HeldInterrupts` 参数
在编译期强制调用方先关中断。`!Send` 防止跨核心传递。

## 锁序检查

`SpinLockIrq` 支持锁级别（`lock_level`）和 per-CPU 锁栈，
在运行时检测锁获取顺序违反，防止 ABBA 死锁：

```rust
pub mod lock_level {
    pub const SCHED_LOCK: u8 = 0;         // 调度锁——最先获取
    pub const TASK_TABLE_LOCK: u8 = 1;    // 任务表锁——在调度锁之后
    pub const UNCLASSIFIED: u8 = 0xFF;    // 跳过检查
}
```

获取新锁时，级别必须严格大于 per-CPU 栈顶的级别，否则 panic。

## IrqSafeGuard 的 Drop 顺序

`IrqSafeGuard` 利用 Rust 的字段析构顺序（[Reference §Destructors]）
保证「先释放锁，后恢复中断」：

1. 自定义 `Drop::drop()` → 弹出锁栈
2. `inner`（`MutexGuard`）析构 → clear_owner + release 锁
3. `_held`（`HeldInterrupts`）析构 → 恢复中断

[Reference §Destructors]: https://doc.rust-lang.org/reference/destructors.html

## 嵌套锁获取

任务窃取等场景需要在已关中断的情况下获取另一核心的同级别锁。
`try_lock_nested` 用 proof token 替代 unsafe 逃生舱：

```rust
// 调用者已持有 HeldInterrupts
let _guard = other_lock.try_lock_nested(&held)?;
// _guard drop 时自动释放，中断由外层 held 管理
```

编译期保证「中断已禁用」，RAII 保证「锁一定释放」。

## 模块结构

```
src/
├── lib.rs             crate 入口，类型别名 + re-export
├── raw.rs             Layer 0: RawLock trait + RawSpinLock (TTAS)
├── mutex.rs           Layer 1a: Mutex<R, T> + MutexGuard
├── irq_safe.rs        Layer 1b: IrqSafe<R, T> + IrqSafeGuard + lock_level
├── interrupt_ops.rs   HeldInterrupts proof token
├── irq.rs             架构中断控制原语（RISC-V sstatus / AArch64 DAIF）
└── lock_stack.rs      Per-CPU 锁获取顺序栈
```

## 使用示例

### SpinLock（不关中断）

```rust
use sync::SpinLock;

static MY_DATA: SpinLock<u32> = SpinLock::new(0, "my_data");

let mut guard = MY_DATA.lock();
*guard += 1;
// guard drop 时释放锁
```

### SpinLockIrq（中断安全）

```rust
use sync::SpinLockIrq;
use sync::lock_level;

static TABLE: SpinLockIrq<Vec<u8>> =
    SpinLockIrq::new_with_level(Vec::new(), "table", lock_level::TASK_TABLE_LOCK);

let mut guard = TABLE.lock();  // 自动关中断
guard.push(42);
// guard drop 时恢复中断
```

### HeldInterrupts（proof token）

```rust
use sync::HeldInterrupts;

let held = HeldInterrupts::hold();  // 保存中断状态 + 关中断
do_critical_section(&held);          // 编译期证明中断已禁用
drop(held);                          // 恢复中断
```

## 注意事项

### 1. SpinLock 不可在中断 handler 中使用

如果中断 handler 获取与线程相同的 `SpinLock`，会在同核心递归加锁（panic 或死锁）。
此类场景必须使用 `SpinLockIrq`。

### 2. UNCLASSIFIED 级别跳过锁序检查

`SpinLockIrq::new()` 默认级别为 `UNCLASSIFIED`，不参与锁序检查。
对安全性敏感的锁应使用 `new_with_level()` 显式指定级别。

### 3. RawSpinLock 无公平性保证

当前 TTAS 算法不保证 FIFO。在低核心数（≤8）下可接受，
高争用场景可通过实现 `RawTicketLock` 等公平算法替换。

## TODO

### RwLockIrq

为读多写少的场景（页表、地址空间）提供读写锁，
提高多核读并发度。实现 `RawRwLock` 后 `IrqSafe<RawRwLock, T>` 即可复用现有中断管理层。

### Per-CPU 帧缓存集成

`HeldInterrupts` 的 proof token 设计已预留 `&HeldInterrupts` 参数模式，
供 per-CPU 帧缓存的快速路径使用（无锁、仅关中断）。

### Debug 模式锁序检查扩展

为 `SpinLock`（非 IRQ 版）也加入 `cfg(debug_assertions)` 下的锁序检查，
覆盖更多潜在死锁场景。
