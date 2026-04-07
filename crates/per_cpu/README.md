# per_cpu

Per-CPU 数据基础设施——通过 `#[cpu_local]` 分散声明，每核心独立副本。

## 概览

`per_cpu` 提供 per-CPU 变量的声明、初始化和访问机制。
用户通过 `#[cpu_local]` 属性宏声明变量，运行时通过 per-CPU 基地址寄存器
定位当前 CPU 的副本，实现无锁、无原子操作的核心本地数据访问。

本 crate 只提供 per-CPU **机制**（声明、初始化、访问），
不包含业务变量——各子系统在自己的 crate 中用 `#[cpu_local]` 声明。

## 工作原理

```
编译期                              运行时
+---------------------+            +-----------------------------------------+
| .percpu section     |            | BSS: PERCPU_AREAS                       |
| (ELF 模板)          |   复制 N 份 | +----------+----------+--------------+  |
| +-----------------+ |  ---------> | |  CPU 0   |  CPU 1   |  CPU 2 ...   |  |
| | CORE_ID: 0      | |            | | CORE_ID:0| CORE_ID:1| CORE_ID:2    |  |
| +-----------------+ |            | +----------+----------+--------------+  |
+---------------------+            +-----------------------------------------+
                                          |
                                    per-CPU 基地址寄存器指向当前 CPU 的区域
```

1. `#[cpu_local]` 将变量放入 `.percpu` ELF section（模板）
2. `percpu_init()` 将模板复制 N 份（每 CPU 一份）到 BSS 预留区
3. per-CPU 基地址寄存器指向当前 CPU 的副本
4. 访问：`base + (模板地址 - __percpu_start)` = 当前 CPU 的变量地址

## 核心类型

### `CpuLocal<T: Sync>`

Per-CPU 变量的包装器。不直接持有数据——数据在每个 CPU 的区域中各有一份副本。

| 方法 | 说明 |
|------|------|
| `get() -> &T` | 当前 CPU 的不可变引用 |
| `get_mut() -> &mut T` | 当前 CPU 的可变引用（**unsafe**，需关中断） |
| `get_on(core_id) -> &T` | 指定 CPU 的引用（**unsafe**，需原子类型或 IPI 保护） |

## 内置 per-CPU 变量

| 变量 | 类型 | 说明 |
|------|------|------|
| `CORE_ID` | `usize` | 当前核心 ID（`percpu_init` 时写入） |

其他 per-CPU 变量由各自 crate 使用 `#[cpu_local]` 声明，例如：
- `sync` crate 声明 `LOCK_STACK: LockStack`（锁顺序栈）
- `interrupt_state` crate 声明 `HARDIRQ_COUNT`、`SOFTIRQ_COUNT`、`PREEMPT_DISABLE_COUNT`、`NEED_RESCHED` 等
- `local_tick` crate 声明 `LOCAL_TICK_COUNT: AtomicU64`（每核心定时器计数）

## 公开函数

| 函数 | 说明 |
|------|------|
| `percpu_init()` | 主核初始化（复制模板、设置基地址寄存器） |
| `percpu_init_smp()` | 从核初始化（设置基地址寄存器） |
| `current_core_id()` | 读取当前核心 ID |

## 依赖

| crate | 用途 |
|-------|------|
| `macros` | 提供 `#[cpu_local]` 过程宏 |
| `config` | `MAX_CORE_COUNT`、`PERCPU_AREA_MAX` 常量 |
| `arch` | 架构相关操作（`percpu_base()`、`set_percpu_base()`、`core_id()`） |

## 模块结构

```
src/
└── lib.rs           CpuLocal<T> 定义、percpu_init/percpu_init_smp、current_core_id
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
use core::sync::atomic::Ordering;

// 安全：AtomicBool 本身保证原子性
unsafe { NEED_RESCHED.get_on(target_core) }.store(true, Ordering::Release);
```

## 初始化顺序

```
_start
  +-> percpu_init()          <- 主核：复制模板、设置所有 CPU 基地址寄存器
       +-> percpu_init_smp()  <- 各从核：设置自己的基地址寄存器
```

`percpu_init()` 必须在 `logging::init()` 之后、任何 `#[cpu_local]` 访问之前调用。

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
