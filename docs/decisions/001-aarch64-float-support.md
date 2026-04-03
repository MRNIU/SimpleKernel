# ADR-001: AArch64 启用硬件浮点支持

## 状态

**提议**

## 日期

2026-04-03（审计阶段 R0）

## 背景

SimpleKernel 采用 SAS（单地址空间）架构，应用程序与内核运行在同一特权级和地址空间中。

当前 AArch64 编译目标为 `aarch64-unknown-none-softfloat`，即禁用硬件浮点，所有浮点运算由软件模拟。
而 RISC-V 目标 `riscv64gc-unknown-none-elf` 中 `g` 已包含 `f`+`d` 扩展（单精度+双精度硬件浮点）。

在 SAS 架构下，应用程序与内核共享编译目标。如果应用需要浮点运算，内核也必须启用硬件浮点支持，否则：
1. 编译目标不支持浮点指令，应用无法使用浮点
2. 上下文切换时不保存 FP/SIMD 寄存器（V0-V31），多任务浮点状态会互相覆盖

## 备选方案

### 方案 A：切换到 `aarch64-unknown-none`（默认启用 FP/NEON）

- 优点：与 RISC-V 对称，应用可直接使用浮点；Rust 标准库的 `f32`/`f64` 运算生成硬件指令
- 缺点：上下文切换需保存 32×128bit FP/SIMD 寄存器（额外 512 字节/任务），需修改 `TrapContext` 和汇编

### 方案 B：保持 `softfloat`，应用通过软件模拟

- 优点：无需修改上下文切换，实现简单
- 缺点：性能差；两个架构不对称；限制应用能力

### 方案 C：lazy FP save/restore

- 优点：只在任务实际使用浮点时才保存/恢复，减少上下文切换开销
- 缺点：实现复杂（需 trap on FP access + 状态追踪）；SAS 架构下 trap 成本更高

## 决策

（待讨论后确定）

初步倾向方案 A。实施计划：
1. R0 阶段：统一 CI 中 Clippy target 为 `aarch64-unknown-none-softfloat`（修复当前不一致）
2. R4 阶段（架构审查）：切换到 `aarch64-unknown-none`，同步修改 `TrapContext` + 汇编

## 理由

（待方案确定后填写）

## 影响

- **`xtask/src/arch.rs`**：`target_triple()` 返回值变更
- **`.cargo/config.toml`**：target section 更新
- **`src/arch/aarch64/context.rs`**：`TrapContext` 增加 V0-V31 字段
- **`src/arch/aarch64/*.S`**：上下文保存/恢复汇编增加 FP 寄存器
- **CI workflow**：Clippy target 更新
- **二进制体积**：每个任务栈额外 512 字节
