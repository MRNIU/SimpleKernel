<!-- Copyright The SimpleKernel Contributors -->

# interrupt_state

中断状态与抢占状态管理——proof token、RAII 守卫和架构中断原语。

## 概览

`interrupt_state` 将"中断已关闭"和"抢占已禁用"编码为 Rust 类型，
利用类型系统在编译期保证中断安全和抢占安全。

核心原则：**关中断只能通过 guard，开中断由 guard 的 Drop 自动完成**。
裸 `irq_disable()` 为 `pub(crate)`，外部无法直接调用——
这在编译期消除了"关了忘记开"的错误类别。

唯一的例外是 `bootstrap_enable()`——新任务首次获得 CPU 时
无条件开启中断，不与 disable 配对，因此标记为 `unsafe`。

## 公开 API

```rust
/// 关中断的唯一方式——guard 析构时自动恢复
pub struct HeldInterrupts { /* !Copy, !Clone, !Send */ }
impl HeldInterrupts {
    pub fn hold() -> Self;          // 保存状态 + 关中断
    pub fn was_enabled(&self) -> bool;
}
impl Drop for HeldInterrupts { ... } // 恢复之前的中断状态

/// 抢占禁用的唯一方式——guard 析构时自动恢复
pub struct PreemptGuard { /* !Copy, !Clone, !Send */ }
impl PreemptGuard {
    pub fn disable() -> Self;       // 递增嵌套计数
}
impl Drop for PreemptGuard { ... }  // 递减嵌套计数

/// 硬中断上下文 RAII 守卫
pub struct HardIrqGuard { /* !Copy, !Clone, !Send */ }
impl HardIrqGuard {
    pub fn enter() -> Self;         // 递增硬中断嵌套计数
}
impl Drop for HardIrqGuard { ... }  // 递减硬中断嵌套计数

/// 查询（纯读，不改状态）
pub fn is_enabled() -> bool;
pub fn is_in_interrupt() -> bool;
pub fn preemptible() -> bool;

/// 调度标志
pub fn check_and_clear_need_resched() -> bool;
pub unsafe fn set_need_resched_on(target_core: usize);

/// 仅供 bootstrap（非成对操作）
pub unsafe fn bootstrap_enable();
```

### PreemptGuard 使用示例

```rust
use interrupt_state::PreemptGuard;

let guard = PreemptGuard::disable();
// 抢占已禁用，可安全操作 per-CPU 数据
do_something();
drop(guard); // 恢复抢占
```

支持嵌套调用——每次调用递增计数，全部守卫析构后才恢复可抢占状态。
类似 C/C++ 中手动调用 `preempt_disable()` / `preempt_enable()` 对，
但利用 Rust RAII 保证一定成对出现，不可能遗漏 `preempt_enable()`。

### 抢占与调度查询

| 函数 | 说明 |
|------|------|
| `preemptible()` | 抢占禁用计数为 0 且不在中断上下文时返回 `true` |
| `check_and_clear_need_resched()` | 原子检查并清除当前核心的重调度标志 |
| `set_need_resched_on(core)` | 跨核设置目标核心的重调度标志（`unsafe`，需保证 core 有效） |

## `HeldInterrupts` 的类型约束

| 约束 | 机制 | 目的 |
|------|------|------|
| `!Copy` | 不 derive | 防止复制后双重恢复 |
| `!Clone` | 不 derive | 同上 |
| `!Send` | `PhantomData<*const ()>` | 中断状态是 per-CPU 的，不可跨核传递 |
| RAII Drop | `impl Drop` | 自动恢复，杜绝遗漏 |
| `#[must_use]` | `hold()` 返回值 | 防止 `HeldInterrupts::hold();` 立即 drop |

`PreemptGuard` 和 `HardIrqGuard` 具有相同的 `!Send` 约束（`PhantomData<*const ()>`），
且均为零大小类型（ZST）。

## 嵌套 hold

支持嵌套调用——内层 `hold()` 发现中断已禁用（`was_enabled = false`），
drop 时不会重新启用，只有最外层的 guard 才恢复中断：

```rust
let outer = HeldInterrupts::hold();  // 关中断，was_enabled = true
let inner = HeldInterrupts::hold();  // 中断已关，was_enabled = false
drop(inner);                          // was_enabled = false -> 不恢复
drop(outer);                          // was_enabled = true  -> 恢复中断
```

## 架构支持

| 架构 | 查询 | 关中断 | 开中断 |
|------|------|--------|--------|
| RISC-V | `sstatus.SIE` | `csrc sstatus, SIE` | `csrs sstatus, SIE` |
| AArch64 | `DAIF.I == 0` | `msr daifset, #2` | `msr daifclr, #2` |
| 宿主机（测试） | 返回 `false` | no-op | no-op |

AArch64 通过 `aarch64-cpu` crate 的 `DAIFSet`/`DAIFClr` 封装使用
`daifset`/`daifclr` 指令，而非 `DAIF.write()`，
避免意外取消屏蔽 Debug/SError/FIQ 异常。

## 模块结构

```
src/
├── lib.rs               crate 入口，HeldInterrupts、PreemptGuard、HardIrqGuard、
│                        is_enabled、is_in_interrupt、preemptible、
│                        check_and_clear_need_resched、set_need_resched_on、
│                        bootstrap_enable
└── arch/
    ├── mod.rs           InterruptArch trait + cfg 分发
    ├── riscv64.rs       RISC-V 中断原语（pub(crate)）
    ├── aarch64.rs       AArch64 中断原语（pub(crate)）
    └── host.rs          宿主机 stub（cargo test 用）
```

## Per-CPU 变量

本 crate 通过 `#[cpu_local]` 声明以下 per-CPU 变量：

| 变量 | 类型 | 说明 |
|------|------|------|
| `HARDIRQ_COUNT` | `AtomicU32` | 硬中断嵌套计数 |
| `SOFTIRQ_COUNT` | `AtomicU32` | 软中断嵌套计数（当前始终为 0，预留） |
| `PREEMPT_DISABLE_COUNT` | `AtomicU32` | 抢占禁用嵌套计数 |
| `NEED_RESCHED` | `AtomicBool` | 重调度标志（可跨核设置） |

## 与 sync crate 的关系

`sync` crate 依赖 `interrupt_state` 并 re-export：

```text
interrupt_state            sync
+----------------+    +---------------------+
| HeldInterrupts |<---| IrqSafe<R, T>       |
| PreemptGuard   |<---| Mutex<R, T>         |
| is_in_interrupt |    |   MutexGuard        |
| is_enabled()   |    |     +- _preempt: PreemptGuard
| bootstrap_     |    |   IrqSafeGuard      |
|   enable()     |    |     +- _held: HeldInterrupts
+----------------+    +---------------------+
```

- `Mutex`（即 `SpinLock`）获取时通过 `PreemptGuard::disable()` 禁用抢占，
  guard 析构时由 `PreemptGuard` 的 Drop 自动恢复。
- `IrqSafe`（即 `SpinLockIrq`）获取时通过 `HeldInterrupts::hold()` 关中断，
  不需要额外的 `PreemptGuard`（中断禁用已隐含抢占禁用）。
- 只需关中断不需要锁的模块可直接依赖 `interrupt_state`。
- 只需禁用抢占的场景可直接使用 `PreemptGuard`。

## 注意事项

### 1. `bootstrap_enable()` 只在 bootstrap 路径调用

新任务通过 `switch_to` 首次获得 CPU 时，中断处于禁用状态。
`bootstrap_enable()` 无条件开启中断，调用方必须确保向量表已初始化、
调度锁已释放。非 bootstrap 场景应使用 `HeldInterrupts`。

### 2. 函数签名可以要求 proof token

```rust
fn access_per_cpu_data(held: &HeldInterrupts) {
    // 编译器保证调用方已关中断
}
```

这比 `unsafe` + 注释更安全——如果调用方没有 `HeldInterrupts` 值，代码无法编译。
