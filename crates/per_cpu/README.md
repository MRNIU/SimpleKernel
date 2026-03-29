# per_cpu

Per-CPU 数据基础设施——通过 `#[cpu_local]` 分散声明，每核心独立副本。

## 概览

`per_cpu` 提供 per-CPU 变量的声明、初始化和访问机制。
用户通过 `#[cpu_local]` 属性宏声明变量，运行时通过 TP 寄存器（riscv64）
或 TPIDR_EL1（aarch64）定位当前 CPU 的副本，实现无锁、无原子操作的核心本地数据访问。

## 工作原理

```
编译期                              运行时
┌─────────────────────┐            ┌─────────────────────────────────────────┐
│ .percpu section     │            │ BSS: PERCPU_AREAS                       │
│ (ELF 模板)          │   复制 N 份 │ ┌───────────┬───────────┬─────────────┐ │
│ ┌─────────────────┐ │  ────────→ │ │  CPU 0    │  CPU 1    │  CPU 2 ...  │ │
│ │ CORE_ID: 0      │ │            │ │ CORE_ID:0 │ CORE_ID:1 │ CORE_ID:2   │ │
│ │ LOCK_STACK: ... │ │            │ │ LOCK_ST.. │ LOCK_ST.. │ LOCK_ST..   │ │
│ │ HARDIRQ_CNT: 0  │ │            │ │ HARDIRQ:0 │ HARDIRQ:0 │ HARDIRQ:0   │ │
│ └─────────────────┘ │            │ └───────────┴───────────┴─────────────┘ │
└─────────────────────┘            └─────────────────────────────────────────┘
                                          ↑
                                    TP / TPIDR_EL1 指向当前 CPU 的区域
```

1. `#[cpu_local]` 将变量放入 `.percpu` ELF section（模板）
2. `percpu_init()` 将模板复制 N 份（每 CPU 一份）到 BSS 预留区
3. TP（riscv64）/ TPIDR_EL1（aarch64）指向当前 CPU 的副本
4. 访问：`TP + (模板地址 - __percpu_start)` = 当前 CPU 的变量地址

## 核心类型

### `CpuLocal<T: Sync>`

Per-CPU 变量的包装器。不直接持有数据——数据在每个 CPU 的区域中各有一份副本。

| 方法 | 说明 |
|------|------|
| `get() -> &T` | 当前 CPU 的不可变引用 |
| `get_mut() -> &mut T` | 当前 CPU 的可变引用（**unsafe**，需关中断） |
| `get_on(core_id) -> &T` | 指定 CPU 的引用（**unsafe**，需原子类型或 IPI 保护） |

### `LockStack`

Per-CPU 锁顺序栈——每个核心记录当前持有的 SpinLock 及其级别，
获取新锁时检查级别是否严格递增，违反时 panic，防止死锁。

## 内置 per-CPU 变量

| 变量 | 类型 | 说明 |
|------|------|------|
| `CORE_ID` | `usize` | 当前核心 ID（`percpu_init` 时写入） |
| `LOCK_STACK` | `LockStack` | 锁顺序栈 |
| `HARDIRQ_COUNT` | `u32` | 硬中断嵌套计数 |
| `SOFTIRQ_COUNT` | `u32` | 软中断嵌套计数 |
| `PREEMPT_DISABLE_COUNT` | `u32` | 抢占关闭计数 |
| `NEED_RESCHED` | `AtomicBool` | 是否需要调度（可跨核设置） |

## 公开函数

| 函数 | 说明 |
|------|------|
| `percpu_init()` | 主核初始化（复制模板、设置 TP） |
| `percpu_init_smp()` | 从核初始化（设置 TP） |
| `current_core_id()` | 读取当前核心 ID |
| `in_interrupt()` | 是否在中断上下文 |
| `preemptible()` | 是否可抢占 |
| `enter_hardirq()` / `exit_hardirq()` | 进入/离开硬中断上下文 |
| `check_and_clear_need_resched()` | 检查并清除调度标志 |
| `set_need_resched_on(core)` | 跨核设置调度标志 |

## 模块结构

```
src/
├── lib.rs           CpuLocal<T>、percpu_init、core_id、中断/抢占计数
└── lock_stack.rs    LockStack + LockStackEntry（锁顺序强制）
```

## 使用示例

### 声明 per-CPU 变量

```rust
use per_cpu::cpu_local;

/// 当前核心的待处理工作计数
#[cpu_local]
pub static PENDING_WORK: u32 = 0;
```

### 读取

```rust
let count = *PENDING_WORK.get();
```

### 修改（需关中断）

```rust
// SAFETY: 中断已关闭，无同核心并发访问
let count = unsafe { PENDING_WORK.get_mut() };
*count += 1;
```

### 跨核访问（需原子类型）

```rust
use per_cpu::NEED_RESCHED;
use core::sync::atomic::Ordering;

// 安全：AtomicBool 本身保证原子性
unsafe { NEED_RESCHED.get_on(target_core) }.store(true, Ordering::Release);
```

## 初始化顺序

```
_start
  └→ percpu_init()          ← 主核：复制模板、设置所有 CPU 基地址、设置 TP
       └→ percpu_init_smp()  ← 各从核：设置自己的 TP
```

`percpu_init()` 必须在 `logging::init()` 之后、任何 `#[cpu_local]` 访问之前调用。

## 裸机 vs 宿主机

| 行为 | 裸机 (`target_os = "none"`) | 宿主机 (`cargo test`) |
|------|---------------------------|---------------------|
| 变量存储 | `.percpu` section → BSS 复制 | 普通 static |
| 访问路径 | TP + offset | 直接解引用模板指针 |
| `current_core_id()` | per-CPU `CORE_ID` 或 raw 寄存器 | 线程局部唯一 ID |
| `get_mut()` 安全性 | 需关中断 | 单线程测试中安全 |

## 注意事项

### 1. `get_mut()` 需要关中断

Per-CPU 变量的"无锁"前提是同一核心上不会有并发访问。
如果中断 handler 也访问同一变量，必须在修改前关中断。
`get()` 对原子类型（如 `AtomicBool`）安全，无需关中断。

### 2. 变量类型必须实现 `Sync`

`#[cpu_local]` 宏在展开时插入编译期检查 `T: Sync`。
这是因为 `CpuLocal<T>` 本身是 `static`（可被多线程引用），
即使实际访问是 per-CPU 隔离的。

### 3. `get_on()` 的安全要求

跨核访问其他 CPU 的 per-CPU 变量存在数据竞争风险。
安全选项：使用原子类型（`AtomicBool`、`AtomicUsize`）或确保目标 CPU 已停止。
