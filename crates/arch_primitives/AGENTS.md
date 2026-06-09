<!-- Copyright The SimpleKernel Contributors -->

# arch_primitives

架构原语层——所有因处理器架构而异的底层操作和常量的统一接口。

## 概览

`arch_primitives` crate 封装 CPU 寄存器访问、中断控制、TLB 维护等硬件原语，
对外暴露架构无关的模块级函数和常量，上层 crate 无需关心具体架构差异。

内部通过 `ArchImpl` trait 定义各架构必须实现的方法集，
再用 `cfg` 选择具体实现，对外只暴露模块级函数——调用方无需感知 trait 的存在。

## 支持的架构

| 架构 | cfg 标志 | 实现文件 | 说明 |
|------|----------|----------|------|
| RISC-V 64 | `bare_riscv64` | `riscv64.rs` | S 模式，TP 寄存器，sfence.vma |
| AArch64 | `bare_aarch64` | `aarch64.rs` | EL1，TPIDR_EL1，TLBI 指令 |

cfg 标志在 `.cargo/config.toml` 中按 target 设置，不需要 build.rs。

## 公开常量

| 常量 | 类型 | RISC-V 64 | AArch64 | 说明 |
|------|------|-----------|---------|------|
| `PA_BITS` | `usize` | 56 | 44 | 物理地址有效位宽 |
| `PT_LEVELS` | `usize` | 3 (Sv39) | 4 (4KB granule) | 页表层级数 |
| `VA_BITS` | `usize` | 39 | 48 | 虚拟地址有效位宽（自动推导） |
| `PTE_SIZE_SHIFT` | `usize` | 3 | 3 | `log2(sizeof(u64))` |
| `ENTRIES_PER_TABLE` | `usize` | 512 | 512 | 每张页表条目数 |
| `INDEX_BITS` | `usize` | 9 | 9 | 单级索引位宽 |
| `INDEX_MASK` | `usize` | 0x1FF | 0x1FF | 单级索引掩码 |
| `LEVEL_SHIFTS` | `[usize; PT_LEVELS]` | [12,21,30] | [12,21,30,39] | 各级 VPN 起始位位置 |
| `FDT_INTERRUPT_CONTROLLER_COMPATIBLES` | `&[&str]` | `riscv,plic0` / `sifive,plic-1.0.0` | `arm,gic-v3` | 当前架构首选中断控制器的 FDT binding |

## 公开函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `percpu_base()` | `-> usize` | 读取 per-CPU 基地址寄存器 |
| `set_percpu_base(val)` | `unsafe` | 写入 per-CPU 基地址寄存器 |
| `core_id()` | `-> usize` | 从硬件寄存器读取核心 ID |
| `is_irq_enabled()` | `-> bool` | 查询中断是否启用 |
| `disable_irq()` | | 禁用中断 |
| `enable_irq()` | `unsafe` | 启用中断 |
| `flush_tlb_all()` | | 刷新整个 TLB |
| `flush_tlb_page(vaddr)` | | 刷新单条 TLB 表项 |
| `page_size_at_level(level)` | `-> usize` | 返回指定层级的页大小 |

## 架构对比

| 操作 | RISC-V 64 | AArch64 |
|------|-----------|---------|
| per-CPU 基地址 | TP 寄存器 | TPIDR_EL1 |
| 核心 ID | TP（boot.S 写入 hart_id） | MPIDR_EL1.Aff0 |
| 中断使能位 | sstatus.SIE | DAIF.I（反逻辑：0=启用） |
| TLB 刷新全部 | `sfence.vma` | `TLBI VMALLE1` + DSB + ISB |
| TLB 刷新单页 | `sfence.vma(0, vaddr)` | `TLBI VAE1(vaddr)` + DSB + ISB |

## 依赖

| crate | 用途 |
|-------|------|
| `config` | `PAGE_SIZE`、`PAGE_SIZE_BITS` 常量（用于页表几何计算） |
| `riscv` | RISC-V CSR 读写（仅 `bare_riscv64`） |
| `aarch64-cpu` + `tock-registers` | AArch64 系统寄存器读写（仅 `bare_aarch64`） |

## 被依赖方

| crate | 用途 |
|-------|------|
| `per_cpu` | `percpu_base()`、`set_percpu_base()`、`core_id()` |
| `interrupt_state` | `is_irq_enabled()`、`disable_irq()`、`enable_irq()` |
| `memory_types` | `PA_BITS`、`VA_BITS` 用于地址有效性检查 |
| `paging` | 页表常量（`PT_LEVELS`、`LEVEL_SHIFTS` 等）、TLB 刷新 |
| `tlb` | `flush_tlb_all()`、`flush_tlb_page()` |

## 模块结构

```
src/
├── lib.rs           ArchImpl trait 定义、cfg 分发、模块级函数和常量
├── riscv64.rs       RISC-V 64 实现（S 模式）
└── aarch64.rs       AArch64 实现（EL1）
```

本 crate 为纯裸机 crate，不在宿主机上编译。
需要 `PA_BITS`/`VA_BITS` 的宿主机测试（如 `memory_types`）自行定义测试用常量。

## 设计要点

### ArchImpl trait 的内部性

`ArchImpl` 是 `pub(crate)` trait，不对外暴露。
对外通过模块级函数 `arch_primitives::disable_irq()` 等调用，
调用方不需要 `use ArchImpl`，也不需要知道 `Riscv64`/`Aarch64` 类型的存在。

## 使用示例

```rust
// 关中断 + 操作 + 开中断
arch_primitives::disable_irq();
// ... 临界区操作 ...
// SAFETY: 中断向量表已初始化，不在嵌套关中断临界区内
unsafe { arch_primitives::enable_irq() };

// 读取 per-CPU 基地址
let base = arch_primitives::percpu_base();

// 刷新单页 TLB
arch_primitives::flush_tlb_page(vaddr);

// 页表几何常量
let shift = arch_primitives::LEVEL_SHIFTS[level];
let entries = arch_primitives::ENTRIES_PER_TABLE;

// FDT 中断控制器 binding
let compatibles = arch_primitives::FDT_INTERRUPT_CONTROLLER_COMPATIBLES;
```
