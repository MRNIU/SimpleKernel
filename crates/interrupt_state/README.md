# interrupt_state

中断状态管理——`HeldInterrupts` proof token 和架构中断原语。

## 概览

`interrupt_state` 将「中断已关闭」编码为 Rust 类型，
借鉴 Theseus OS 的 intralingual 设计哲学。

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

/// 查询（纯读，不改状态）
pub fn is_enabled() -> bool;

/// 仅供 bootstrap（非成对操作）
pub unsafe fn bootstrap_enable();
```

## `HeldInterrupts` 的类型约束

| 约束 | 机制 | 目的 |
|------|------|------|
| `!Copy` | 不 derive | 防止复制后双重恢复 |
| `!Clone` | 不 derive | 同上 |
| `!Send` | `PhantomData<*const ()>` | 中断状态是 per-CPU 的，不可跨核传递 |
| RAII Drop | `impl Drop` | 自动恢复，杜绝遗漏 |
| `#[must_use]` | `hold()` 返回值 | 防止 `HeldInterrupts::hold();` 立即 drop |

## 嵌套 hold

支持嵌套调用——内层 `hold()` 发现中断已禁用（`was_enabled = false`），
drop 时不会重新启用，只有最外层的 guard 才恢复中断：

```rust
let outer = HeldInterrupts::hold();  // 关中断，was_enabled = true
let inner = HeldInterrupts::hold();  // 中断已关，was_enabled = false
drop(inner);                          // was_enabled = false → 不恢复
drop(outer);                          // was_enabled = true  → 恢复中断
```

## 架构支持

| 架构 | 查询 | 关中断 | 开中断 |
|------|------|--------|--------|
| RISC-V | `sstatus.SIE` | `csrc sstatus, SIE` | `csrs sstatus, SIE` |
| AArch64 | `DAIF.I == 0` | `msr daifset, #2` | `msr daifclr, #2` |
| 宿主机（测试） | 返回 `false` | no-op | no-op |

AArch64 使用 `daifset`/`daifclr` 而非 `DAIF.write()`，
避免意外取消屏蔽 Debug/SError/FIQ 异常。

## 模块结构

```
src/
├── lib.rs      crate 入口，pub API（is_enabled / bootstrap_enable）
├── arch.rs     架构中断原语（pub(crate)，外部不可直接调用）
└── held.rs     HeldInterrupts proof token
```

## 与 sync crate 的关系

`sync` crate 依赖 `interrupt_state` 并 re-export：

```text
interrupt_state          sync
┌──────────────┐    ┌──────────────────┐
│ HeldInterrupts│◄───│ IrqSafe<R, T>    │
│ is_enabled()  │    │ IrqSafeGuard     │
│ bootstrap_    │    │   └─ _held: HeldInterrupts
│   enable()    │    │ lock_stack       │
└──────────────┘    └──────────────────┘
```

`IrqSafeGuard` 内部持有 `HeldInterrupts`，锁释放后由 guard 的字段析构自动恢复中断。
只需关中断不需要锁的模块（如未来的 per-CPU 帧缓存）可直接依赖 `interrupt_state`。

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

## TODO

### AArch64 裸汇编替换

AArch64 的 `irq_disable`/`irq_enable` 使用裸汇编（`msr daifset, #2` / `msr daifclr, #2`），
因为 `aarch64-cpu` crate 的 `DAIF` 只提供整寄存器读写（`msr DAIF, Xn`），
会覆盖 Debug/SError/FIQ 的屏蔽位。`daifset`/`daifclr` 是原子位操作指令，
只修改指定位不影响其他异常掩码。

待上游 `aarch64-cpu` 提供 `daifset`/`daifclr` 封装后，替换裸汇编。
也可考虑向上游提交 PR 添加此功能。
