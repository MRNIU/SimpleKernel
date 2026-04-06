# ADR-004: 消除内核源码中的 `#[cfg(bare_metal)]`

> **状态**: 提议
>
> **日期**: 2026-04-06（更新）
>
> **审计阶段**: R8 — 集成与收尾
>
> **涉及模块**: 全项目

## 背景

当前项目通过 `#[cfg(bare_metal)]` / `#[cfg(not(bare_metal))]` 在源码中同时容纳裸机实现和宿主机 mock，
以支持 `cargo test` 在宿主机上运行单元测试。

架构差异（riscv64 vs aarch64）已由 `crates/arch/` 统一处理，
剩余的 cfg 全部是**运行环境差异**（裸机 vs 宿主机），遍布整个代码库。

### 完整清单

#### `crates/` 下的 cfg

| crate | cfg 数量 | 保护什么 |
|-------|---------|---------|
| `per_cpu` | 19 | 链接器符号 `__percpu_start`/`__percpu_end`、`SyncUnsafeCell`（nightly）、`PerCpuArea`/`PERCPU_AREAS`/`PERCPU_BASES` 静态数据、`percpu_init()`/`percpu_init_smp()`、`CpuLocal::get()`/`get_mut()`/`get_on()`/`offset()` 双路径、`host_core_id()` |
| `paging` | 8 | `KernelNodeFrame`（真实帧分配器）vs `HeapNodeFrame`（`Box` mock）、`NodeFrame` type alias |
| `sync` | 6 | `mutex.rs`: 中断上下文断言（2）+ 锁栈 push/pop（2）; `irq_safe.rs`: 锁栈 push/pop（2） |
| `memory` | 5 | `extern crate alloc`、`frame_allocator` re-export、`heap` re-export、`init` 模块、`init()`/`init_smp()` |
| `macros` | 2 | `.percpu` ELF section 放置 vs 普通 static |
| `arch` | 7 | 模块选择（riscv64/aarch64/host）、type alias、`set_irq_enabled_for_test()` |
| `page_table_entry` | 3 | PTE 类型选择（aarch64/riscv64/host 占位） |

#### `src/` 下的 cfg

| 文件 | cfg 数量 | 保护什么 |
|------|---------|---------|
| `src/lib.rs` | 7 | 模块开关：`arch`、`boot`、`device`、`fdt`、`fs`、`init`、`timer` 仅裸机编译 |
| `src/task/tcb.rs` | 11 | 上下文指针、内核栈、信号掩码、`FdTable` 字段、`new()`/`fork()` 方法 |
| `src/task/mod.rs` | 7 | `TaskManager`、调度器模块、`spawn()`/`schedule()`/`exit()` |
| `src/task/mutex.rs` | 2 | `TaskMutex` 类型、`sleep_on_lock()` |
| `src/syscall/mod.rs` | 3 | 三个 syscall 子模块 |
| `src/logging.rs` | 2 | `put_str()` 的控制台输出 vs no-op |
| `src/panic.rs` | 2 | `handle_panic()` 裸机行为 |
| `src/util/halt.rs` | 1 | `halt()` 函数 |

**总计：~85 个 cfg 散布在 15+ 个文件中。**

### 核心问题

内核代码不应该为宿主机测试做出妥协。
Linux 不会在源码里写 `#ifdef __KERNEL__`——它的代码就是内核代码。

## 参考内核做法

### Theseus（基于 [Theseus 源码](https://github.com/theseus-os/Theseus) 调研）

**策略：不让硬件相关 crate 在宿主机编译。**

- 整个内核代码库只有 **3 个** `#[cfg(test)]`，全在纯逻辑 crate（`memory_structs` 地址运算、`frame_allocator` 分配算法）
- **零** `#[cfg(not(target_os = "none"))]`——没有任何 host mock
- Per-CPU (`local_storage_initializer`)：纯裸机，无 host 回退
- 内存 crate：无 mock 分配器
- 内核功能测试通过 QEMU 裸机应用执行（`applications/test_tls` 等），由 `theseus_tests` feature 控制

### rCore / Tock

类似策略——内核 crate 只为裸机编译，测试在 QEMU 或硬件上运行。

### 共同点

成熟内核不试图让硬件相关代码在宿主机上编译。测试分两层：
1. 纯逻辑（地址运算、算法）→ `cargo test` 宿主机
2. 涉及硬件（per-CPU、中断、锁、页表、任务、文件系统）→ QEMU 裸机

## 备选方案

### 方案 A: Theseus 式——内核代码只为裸机编译

将内核代码库分为两类：

| 类别 | 编译目标 | 测试方式 | cfg 需求 |
|------|---------|---------|---------|
| **纯逻辑 crate** | 宿主机 + 裸机 | `cargo test` | 无 |
| **内核 crate** | 仅裸机 | `cargo xtask test`（QEMU） | 无 |

纯逻辑 crate（保持 `cargo test`，无需改动）：
- `memory_types` — 地址运算、页帧类型
- `span` — 通用范围类型
- `page_table_entry` — PTE 编解码逻辑（两个架构模块始终编译，type alias 可保留 cfg 或改为泛型）
- `config` — 常量（已通过 `arch` crate 消除 cfg）

内核 crate（只为裸机编译，删除所有 host mock）：
- `per_cpu` — 删除 `host_core_id()`、`CpuLocal` host 分支、19 个 cfg
- `sync` — 删除 6 个 cfg，锁栈检查和中断断言变为无条件代码
- `paging` — 删除 `HeapNodeFrame` 及 8 个 cfg
- `memory` — 删除 5 个 cfg
- `macros` — `.percpu` section 放置变为无条件
- `interrupt_state` — 已无 cfg（已迁移到 `arch`）；宿主机测试迁移到系统测试
- `arch` — host.rs 仅保留给 `cargo check`（见下方说明）

`src/` 下的内核主体：
- `src/lib.rs` — 删除 7 个模块开关 cfg，所有模块无条件编译
- `src/task/` — 删除 20 个 cfg，TCB/调度器/mutex 变为纯裸机代码
- `src/logging.rs` — `put_str()` 不再需要 no-op 分支
- `src/syscall/` — 删除 3 个 cfg
- `src/panic.rs`、`src/util/halt.rs` — 删除 cfg

测试迁移：

| 现有宿主机测试 | 迁移到 |
|--------------|--------|
| `interrupt_state`: HeldInterrupts 保存-恢复 | `tests/system/` 新增 interrupt 测试组 |
| `per_cpu`: CpuLocal::get() | `tests/system/` 已有或新增 |
| `sync`: SpinLock 基本操作 | `tests/system/` 已有 sync 测试组 |

`arch` crate 特殊处理：
`arch` crate 保留 host.rs 和 `cfg(not(bare_metal))`。原因：`config` 依赖 `arch::PA_BITS`/`arch::PT_LEVELS`，而 `config` 被纯逻辑 crate（`memory_types`）依赖。如果 `arch` 不能在宿主机编译，整个依赖链断裂。host.rs 提供占位值使 `cargo test` 能编译纯逻辑 crate。这是唯一保留 cfg 的 crate，且 cfg 已经集中在一处。

**优点**:
- 消除 ~78 个 cfg（仅 `arch` 保留 ~7 个）
- 内核代码就是裸机代码，零宿主机妥协
- 删除全部 host mock 代码
- 不再需要 "host mock 是否忠实反映裸机行为" 的审计（R8 待办可关闭）
- 与 Theseus、rCore 等成熟项目一致

**缺点**:
- 开发反馈变慢：改了内核逻辑不能 `cargo test` 几秒验证，要跑 QEMU 系统测试（~30s）
- `cargo check` 在宿主机上不再检查内核 crate（交叉编译 `cargo check --target riscv64gc-unknown-none-elf` 仍可用）
- 一次性测试迁移工作量

### 方案 B: cfg 集中到每个 crate 的分发层

保持所有 crate 在宿主机可编译，但将 cfg 收缩到每个 crate 的一个分发文件。核心逻辑通过 trait 或类型别名引用。

**优点**:
- 保留 `cargo test` 快速反馈
- 核心逻辑文件较干净

**缺点**:
- `per_cpu` 的 host mock 与裸机实现差异过大，trait 抽象不自然（链接器符号、ELF section 模板复制无法用 trait 表达）
- `src/task/tcb.rs` 有 11 个 cfg 保护不同字段——结构体字段的存在与否无法用 trait 抽象
- `src/lib.rs` 的模块开关无法通过 trait 处理
- 仍需维护 host mock，仍需验证一致性
- cfg 从 ~85 降到 ~15，没有完全消除

### 方案 C: 保持现状

cfg 散布在源码中，语义通过 `bare_metal` 别名已较清晰。

**优点**:
- 零工作量
- `cargo test` 快速反馈保留

**缺点**:
- 85 个 cfg 散布，内核逻辑被测试需求污染
- host mock 维护负担持续
- 偏离成熟内核的做法

## 决策

**采用方案 A：Theseus 式——内核代码只为裸机编译。**

理由：
1. 消除 ~78 个 cfg，内核代码零宿主机妥协
2. 删除全部 host mock，关闭 "host 模拟一致性" 这个永远审不完的问题
3. 需迁移的 13 个宿主机测试（`per_cpu`: 2, `interrupt_state`: 11）在宿主机上的可信度本就有限——它们测的是 mock 实现而非真实行为
4. 与 Theseus/rCore/Tock 等成熟项目一致
5. `arch` crate 保留 host.rs 为纯逻辑 crate 提供 `cargo test` 编译支持，是合理的局部妥协

## 关键权衡

| 维度 | 方案 A（只编译裸机） | 方案 B（cfg 集中） | 方案 C（现状） |
|------|-------------------|-------------------|--------------|
| 内核代码干净度 | 最优（~78 个 cfg 消除） | 较好（降到 ~15） | 差（~85） |
| 开发反馈速度 | 慢（QEMU ~30s） | 快（cargo test ~2s） | 快 |
| 维护负担 | 最低（无 host mock） | 中 | 高 |
| 与业界一致性 | Theseus/rCore 一致 | 无直接先例 | — |
| 一次性迁移量 | 大（15+ 文件） | 中 | 无 |
| 对纯逻辑 crate 的影响 | 无 | 无 | 无 |

## 影响

- **代码变更**:
  - `crates/`: 删除 `per_cpu`、`sync`、`paging`、`memory`、`macros` 中所有 `cfg(not(bare_metal))` 分支和 host mock
  - `src/`: 删除 `lib.rs`、`task/`、`logging.rs`、`syscall/`、`panic.rs`、`halt.rs` 中所有 cfg
  - `crates/arch/`: 保留 host.rs（为纯逻辑 crate 的 `cargo test` 提供占位值）
- **Cargo.toml 变更**: 内核 crate 从 `#![cfg_attr(not(test), no_std)]` 改为 `#![no_std]`
- **测试变更**: `interrupt_state`、`per_cpu`、`sync` 的宿主机测试迁移到 `tests/system/`
- **CI 变更**: `cargo test` 范围缩小到纯逻辑 crate；`cargo xtask test` 覆盖内核功能
- **R8 审计影响**: "host 模拟实现一致性评估" 和 "`CpuLocal` host 行为 ADR" 两个待办可直接关闭
- **开发工作流**: 需文档化新的测试策略（哪些 `cargo test`、哪些 `cargo xtask test`）

## TODO: 裸机调试能力增强

去掉宿主机测试后，复杂 bug（调度死锁、多核竞态）只能在 QEMU 裸机环境中调试。
以下配套工作按优先级排列，确保调试体验不降级：

1. **GDB init 脚本**（零成本）
   - 创建 `debug.gdb`，自动加载内核符号、设置常用断点（`schedule`、`context_switch`、`handle_timer_irq`）
   - 定义 GDB 便捷命令（如 `current_task` 打印当前 TCB）
   - `cargo xtask debug` 自动传入 `-x debug.gdb`

2. **子系统级日志过滤**（小投入）
   - 在 `src/logging.rs` 中增加 module path 匹配，支持按模块设置日志级别
   - 调试调度时可只看 `task::` 前缀的日志，不被其他子系统淹没

3. **调度状态 dump**（中等投入）
   - Panic handler 中自动打印所有 TCB 状态（pid、state、优先级、持有的锁）
   - `SpinLock` acquire 时记录 (task_id, lock_addr) 到环形缓冲区，死锁时 dump
   - 环形缓冲区记录最近 N 次 schedule() 调用的 (timestamp, from_task, to_task, reason)

## 参考

- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — crate 级模块化，内核 crate 不为宿主机编译
- [Tock SOSP'17](https://www.cs.virginia.edu/~bjc8c/papers/levy17tock.pdf) — trait 抽象平台差异，测试在硬件上运行
- [rCore-Tutorial-v3](https://github.com/rcore-os/rCore-Tutorial-v3) — 内核测试在 QEMU 裸机运行
- [Theseus 源码](https://github.com/theseus-os/Theseus) 调研：零 host mock，3 个 `#[cfg(test)]` 均为纯逻辑
- `crates/arch/` 重构 commit `d2cac96f4` — 架构差异已集中处理
