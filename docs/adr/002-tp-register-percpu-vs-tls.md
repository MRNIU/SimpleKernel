<!-- Copyright The SimpleKernel Contributors -->

# ADR-002: RISC-V tp 寄存器——per-CPU 与 TLS 的冲突及 App std 支持路线

> **状态**: 提议
>
> **日期**: 2026-04-03
>
> **审计阶段**: R2 — 同步与 Per-CPU
>
> **涉及模块**: `crates/per_cpu`, `crates/macros`, 未来的 App 运行时

## 背景

SimpleKernel 的 `#[cpu_local]` 机制使用硬件寄存器定位当前核心的 per-CPU 数据区域：

| 架构 | per-CPU 使用的寄存器 | 用户态 TLS 使用的寄存器 |
|------|---------------------|----------------------|
| RISC-V | `tp`（thread pointer） | `tp`（同一个） |
| AArch64 | `TPIDR_EL1`（EL1 专用） | `TPIDR_EL0`（EL0/EL1 都可访问） |

**AArch64 不存在冲突**——两个独立寄存器，per-CPU 和 TLS 各用各的。

**RISC-V 存在冲突**——`tp` 是唯一的线程指针寄存器。传统 Linux 通过 trap 入口/出口保存/恢复 `tp`（用户态存 TLS 指针，内核态存 per-CPU 基地址）。SimpleKernel 的 SAS 架构没有 trap 切换——`tp` 全程被 per-CPU 占用，App 无法使用 TLS。

### 长期目标：App 支持 `std`

项目目标是让 OS 层提供 `std` 能力，使 App 能去掉 `#![no_std]`。这需要：

1. 自定义 Rust target（如 `riscv64-simplekernel`）
2. 移植 `std` 的 `sys` 模块（参考 Redox OS）
3. **TLS 支持**——`std` 内部依赖 `thread_local!`（panic 计数器、stdout 缓冲区、RNG 种子等）

TLS 是 `std` 的硬依赖。不解决 RISC-V 上的 `tp` 冲突，`std` 支持不可能实现。

### 当前状态

- App 在 `no_std` + `#![forbid(unsafe_code)]` 下运行
- App 没有 TLS 需求
- per-CPU 系统通过 `tp` 正常工作
- **冲突是潜在的**——当前不触发，但限制了未来演进

## 备选方案

### 方案 A: 维持现状——禁止 App 使用 TLS

per-CPU 继续占用 `tp`。App 不使用 `#[thread_local]` 和 `thread_local!`。

**优点**:
- 零改动，零开销
- 当前所有代码正常工作

**缺点**:
- App 无法使用依赖 TLS 的库
- 阻断 `std` 支持路线
- 隐式约束，容易被遗忘——未来有人在 RISC-V App 中引入 TLS 会静默产生 UB

### 方案 B: 任务切换时交换 `tp`

`tp` 的语义从"per-CPU 基地址"改为"当前任务的 TLS 指针"。per-CPU 访问改用间接查找：

```rust
// 当前：tp 直接指向 per-CPU 区域
fn get_percpu<T>(offset: usize) -> &T {
    &*((read_tp() + offset) as *const T)
}

// 改后：通过 hart_id 间接查找
fn get_percpu<T>(offset: usize) -> &T {
    let hart_id = read_hart_id();  // 读 CSR（如 sscratch 或 mhartid）
    let base = PERCPU_BASES[hart_id];
    &*((base + offset) as *const T)
}
```

`switch_to` 时保存/恢复 `tp`（存入 TCB）。

**优点**:
- App 获得完整 TLS 支持
- 与 Rust `std` 兼容
- 符合 RISC-V ABI 对 `tp` 的标准用法

**缺点**:
- per-CPU 访问变慢（多一次内存间接 + 一次 CSR 读取）
- per-CPU 是高频路径（中断处理、锁操作），性能影响需量化
- `switch_to` 多两条指令（save/restore `tp`）
- 需要一种可靠的方式获取 hart_id（`sscratch` 在 trap 中有其他用途）

### 方案 C: 利用 `sscratch` 存 per-CPU 基地址

`tp` 留给 TLS。per-CPU 基地址存入 `sscratch` CSR。`#[cpu_local]` 读 `sscratch` 而非 `tp`。

```rust
fn get_percpu<T>(offset: usize) -> &T {
    let base: usize;
    unsafe { asm!("csrr {}, sscratch", out(reg) base) };
    &*((base + offset) as *const T)
}
```

**优点**:
- `tp` 可用于标准 TLS，与 Rust `std` 兼容
- per-CPU 访问仍通过寄存器（`sscratch` 是 CSR，一条指令读取）

**缺点**:
- `csrr` 比 `mv reg, tp` 慢（CSR 读取 vs 通用寄存器移动）
- `sscratch` 在 trap entry 中被 Linux 等内核用作临时寄存器（保存 `sp`），SimpleKernel 也可能有类似需求
- 需要保证 trap 入口不破坏 `sscratch` 中的 per-CPU 基地址

### 方案 D: 合并 per-CPU 和 TLS 到同一个 `tp` 区域

`tp` 指向一个合并的区域：per-CPU 数据在负偏移方向，TLS 在正偏移方向（或反过来）：

```
tp 指向此处
    │
    ├── [-N..0)  per-CPU 数据
    │
    └── [0..+M)  任务 TLS 数据
```

`switch_to` 时：TLS 区域切换（修改 `tp` 指向新任务的合并区域），per-CPU 区域不变（因为在同一核心上）。

**优点**:
- 一个寄存器服务两种需求
- per-CPU 访问速度不变（`tp` + 负偏移）
- TLS 访问速度不变（`tp` + 正偏移）

**缺点**:
- 链接器脚本复杂（需要自定义 `.percpu` 和 `.tdata`/`.tbss` 布局）
- 需要自定义 TLS 运行时（标准 Rust TLS 假设 `tp` 指向 TLS 块起始）
- 每个任务需要分配"per-CPU 副本 + TLS"的合并区域
- 任务迁移时需要更新 per-CPU 部分的指针

### 方案 E: 仅 AArch64 支持 `std`，RISC-V 保持 `no_std`

AArch64 没有 `tp` 冲突，可以直接支持 `std`。RISC-V 暂时保持 `no_std`。

**优点**:
- AArch64 上的 `std` 支持最简单
- 不需要修改 per-CPU 机制
- 可以先在 AArch64 上验证 `std` 移植方案，再回头解决 RISC-V

**缺点**:
- 两个架构的 App 能力不对等
- RISC-V 是项目主要开发架构，功能受限

## 补充分析：为什么 RISC-V S-mode 必须借助寄存器

RISC-V S-mode 无法读取 `mhartid`（M-mode CSR），唯一获取 hart ID 的来源是 boot
时 SBI 通过 `a0` 传入。之后必须存在一个随时可读的位置。`tp` 是 Linux 的选择，
`sscratch` 是另一个可行选项（见方案 C）。AArch64 不存在此问题（`MPIDR_EL1` 在
EL1 可读）。

### `sscratch` 方案的 trap entry 设计

`sscratch` 永久存放 per-CPU 基地址，trap entry 改用 per-CPU scratch 槽保存 `sp`：

```asm
// sscratch 永久存放 per-CPU 基地址
csrr t0, sscratch             // t0 = per-CPU 基地址（只读，不交换）
sd sp, SCRATCH_SP_OFFSET(t0)  // 保存 sp 到 per-CPU scratch 槽
ld sp, KERN_SP_OFFSET(t0)     // 加载内核栈指针
// ... 保存其余寄存器
```

SAS 优势：所有代码运行在内核栈上，trap entry 不需要切换栈，`sscratch` 方案更简单。

### 性能对比

| 操作 | `tp` 方案 | `sscratch` 方案 |
|------|-----------|-----------------|
| 读 per-CPU 基地址 | `mv reg, tp`（1 周期） | `csrr reg, sscratch`（1-3 周期） |
| trap entry 额外开销 | 无（`tp` 始终有效） | 无（`sscratch` 始终有效） |
| per-CPU 访问总成本 | 1 条指令 | 1 条指令（CSR 读取） |

差异在实际工作负载中可忽略。

## 决策

待讨论。

当前采用 **方案 A**（维持现状）作为过渡。

长期目标是支持 App `std`（去掉 `no_std`），需要在以下时间点重新评估：

1. 开始实现自定义 Rust target（`riscv64-simplekernel`）时
2. 开始移植 `std` 的 `sys` 模块时（参考 Redox OS）
3. 引入 App 运行时时

**倾向方案 C**（`sscratch` 做 per-CPU）——一条 CSR 指令、不侵入 `tp`、SAS 下 trap
entry 设计简单。可在 R4（架构层审计）或 `std` 支持实现时执行。

## 影响

- **当前代码**：无变更
- **约束记录**：RISC-V App 禁止使用 `#[thread_local]`——需在 App crate 模板中注明
- **未来变更**：选择 B/C/D 方案时需修改 `crates/per_cpu`、`crates/macros`、`src/arch/riscv64/switch.S`
- **文档**：`crates/per_cpu/README.md` 应注明 RISC-V `tp` 寄存器约束

## 参考

- [Linux: `arch/riscv/kernel/entry.S`] — trap 入口使用 `sscratch` 保存 `sp`，然后加载内核 `tp`（per-CPU）
- [Linux: `arch/riscv/include/asm/asm-offsets.h`] — per-CPU 通过 `tp` + 偏移访问
- [Redox OS: `library/std/src/sys/pal/redox/`] — 自定义 Rust target 的 `std` 移植参考
- [RISC-V ELF psABI](https://github.com/riscv-non-isa/riscv-elf-psabi-doc) — `tp` 寄存器用于 TLS 的 ABI 规范
- [Arm ARM §D17.2.135](https://developer.arm.com/documentation/ddi0487/) — `TPIDR_EL0`/`TPIDR_EL1` 独立寄存器
- [Theseus] — 不使用 `tp` 做 per-CPU（集中式 `CpuLocalData` 结构），不存在此冲突
