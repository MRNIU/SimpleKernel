# ADR-004: 消除内核源码中的 `#[cfg(bare_metal)]`

> **状态**: 提议
>
> **日期**: 2026-04-04
>
> **涉及模块**: 全项目（`src/`、`crates/per_cpu`、`crates/sync`、`crates/paging`、`crates/memory`、`crates/config`、`crates/interrupt_state`、`crates/tlb`、`crates/macros`）

## 背景

当前项目通过 `#[cfg(bare_metal)]` / `#[cfg(not(bare_metal))]` 在内核源码中同时容纳裸机实现和宿主机 mock，
以支持 `cargo test` 在宿主机上运行单元测试。这些 cfg 分三类：

| 类别 | 数量 | 典型例子 |
|------|------|---------|
| **模块开关** | ~15 | `src/lib.rs`: `#[cfg(bare_metal)] pub mod arch` |
| **host mock 二选一** | ~25 | `per_cpu`: `#[cfg(bare_metal)] fn read_tp()` vs `#[cfg(not(bare_metal))] fn host_core_id()` |
| **裸机专属安全检查** | ~8 | `sync/mutex.rs`: `#[cfg(bare_metal)] assert!(!is_in_interrupt())` |

**问题**：内核源码应当默认就是裸机代码——正如 Linux 不会在内核源码里写 `#ifdef __KERNEL__`。
当前的 cfg 散布是一种为宿主机测试做出的妥协，污染了内核逻辑。

## 备选方案

### 方案 A: crate 架构拆分（纯逻辑 + 平台绑定）

将每个需要平台分支的 crate 拆为两层：

```
crates/per_cpu/
  src/lib.rs          ← 纯逻辑，通过 trait 调用平台接口，无 cfg
  src/arch/mod.rs     ← cfg 分发（仅此一处）
  src/arch/riscv64.rs ← 裸机实现
  src/arch/aarch64.rs ← 裸机实现
  src/arch/host.rs    ← #[cfg(test)] mock，仅测试可见
```

内核代码零 cfg 门控。`#[cfg(test)]` mock 只存在于 `arch/host.rs`，
不在核心逻辑路径上。类似 Theseus 的 crate 级隔离。

**优点**:
- 内核源码完全无 cfg 污染
- mock 与真实实现物理隔离，不会意外交叉引用
- 每个 crate 的 `lib.rs` 只有纯逻辑，可读性好
- 符合 CLAUDE.md 的 interface-driven 设计哲学

**缺点**:
- 改动量大，需要逐 crate 重构
- 部分 crate（如 `per_cpu`）的 host mock 与真实实现差异极大，trait 抽象可能不自然
- 可能需要引入泛型参数或关联类型，增加类型签名复杂度

### 方案 B: 只在 arch 分发层保留 cfg，其余上推到 Cargo.toml

保持当前 crate 结构，但将 cfg 控制点从源码内部收缩到两处：

1. **`crates/*/src/arch/mod.rs`**：唯一允许 `cfg(bare_metal)` 的源码文件
2. **`Cargo.toml`**：通过 `[target.'cfg(...)'.dependencies]` 控制裸机依赖

核心逻辑文件（`lib.rs`、`mutex.rs` 等）通过 trait object 或类型别名引用 arch 层，自身无 cfg。

**优点**:
- 改动量适中——每个 crate 只需抽取 arch 层
- cfg 集中在少数 arch 分发文件中，易于审计
- 不需要大幅改变类型签名

**缺点**:
- `src/lib.rs` 的模块开关（`pub mod arch` / `pub mod fdt`）仍需 cfg
- 某些安全检查（如 `is_in_interrupt()` 断言）不好通过 trait 抽象
- 仍存在少量 cfg，只是数量从 ~50 降到 ~10

### 方案 C: 保持现状（cfg 过渡方案）

当前已完成的 `bare_metal` / `bare_riscv64` / `bare_aarch64` 别名方案。
cfg 仍散布在源码中，但语义比 `target_os = "none"` 清晰，且在所有宿主架构上行为一致。

**优点**:
- 已实现，所有命令已通过验证
- 零额外架构变更

**缺点**:
- cfg 散布未解决，内核逻辑被测试需求污染
- 新开发者需要理解 bare_metal/not(bare_metal) 的含义

## 决策

待定。

## 参考

- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — crate 级模块化，每个 crate 内部无条件编译
- [Tock SOSP'17](https://www.cs.virginia.edu/~bjc8c/papers/levy17tock.pdf) — trait 抽象平台差异，`unsafe trait` 作 capability
- [Embassy](https://github.com/embassy-rs/embassy) — `arch-*` feature 选择平台实现，核心逻辑无 cfg
- Linux `scripts/Kconfig` — 配置系统生成编译开关，C 源码用 `IS_ENABLED()` 而非 `#ifdef`
