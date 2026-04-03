# SimpleKernel 全项目深度审计 Roadmap

> **目标**：自底向上排查每个组件，将 C++ 遗留范式转换为地道 Rust，同时完善文档、测试、CI 与工具链。
>
> **参考内核**：Linux（子系统接口设计）、Theseus（typestate / crate 隔离）、Redox（scheme VFS / error handling）、Zephyr（嵌入式设备模型）、µFork（POSIX 兼容策略）
>
> **工作分支**：`feat/rust-SAS`（当前活跃，领先 `main` 234 commits）

---

## 总体原则

| 原则 | 说明 |
|------|------|
| **自底向上** | 从无依赖的叶子 crate 开始，逐层向上，每层稳定后再动上层 |
| **每步可验证** | 每个 Phase 结束时必须：编译通过 + 现有测试全绿 + 新增测试覆盖变更 |
| **全量回归** | 每个 Phase 结束后，运行全量系统测试 + 上游 Phase 的单元测试，防止底层变更静默破坏上层 |
| **文档即产出** | 排查过程中同步输出模块文档（README、时序图、生命周期图、依赖图） |
| **决策即记录** | 重要设计决策写入 ADR（`docs/decisions/`），模板见 `docs/templates/adr-template.md` |
| **参考即标注** | 借鉴外部内核的设计必须在代码/文档中标注出处（`[Linux: fs/namei.c]`、`[Theseus: MappedPages]`） |
| **不做无测试的重构** | 任何架构变更必须先有测试兜底，或先补测试再改 |

## 协作流程

审计采用人机协作模式，人工把控每一个关键决策点：

1. **AI 初步审阅** — AI 按排查 checklist 阅读代码，输出审查报告（问题清单 + 改进建议）
2. **提交人工查看** — 审查报告提交给项目作者 review
3. **作者同步阅读代码** — 作者独立阅读同一段代码，形成自己的理解
4. **共同讨论设计** — 作者向 AI 提问，双方讨论设计方案，重要决策写入 ADR
5. **实施变更** — 达成共识后，AI 执行代码修改，作者 review 并合入

---

## Phase 概览

```
R0  基线建立 ──── CI / 文档基础设施 / unsafe 审计基线 / 依赖审计
│
R1  原语层 ────── memory_types, config, span, build_common
│
R2  同步与 CPU ── sync, interrupt_state, per_cpu, macros
│
R3  内存子系统 ── frame_allocator → page_allocator → page_table_entry
│                 → tlb → paging → heap → memory
│
R4  架构层 ────── arch 抽象 trait, boot flow, console, interrupt, timer
│
R5  任务管理 ──── TCB, scheduler, context switch, signal, preemption
│
R6  设备与文件 ── device framework, HAL, VFS, FD table, ramfs, fatfs
│
R7  系统调用 ──── syscall 层, POSIX 兼容, 可见性审计
│
R8  集成与收尾 ── 文档重写, CI 重写, 项目重组, 分支合并
```

> **注意**：每个 Phase 只规定审查范围和交付物，不预设具体检查细节。
> 细节问题应在排查过程中根据代码实际状态发现，而非提前规定——
> 因为前面 Phase 的变更可能改变后续模块的状态和接口。

---

## R0 — 基线建立

**目标**：搭建排查所需的基础设施，建立度量基线。

### 审查范围

| 子任务 | 内容 |
|--------|------|
| CI 审计与重写 | `workflow.yml` 结构评估、系统测试稳定性、`cargo-deny`、unsafe 统计自动化、代码覆盖率、Clippy/rustfmt 配置、Docker 镜像策略 |
| 文档基础设施 | 模块 README 模板、Mermaid 图表工具链、项目级依赖图、Rustdoc 发布 |
| Unsafe 审计基线 | 全量 unsafe 扫描、分类（必要 vs 可消除）、输出基线报告 |
| 依赖审计 | Git 依赖上游化评估、版本锁定、许可证检查、第三方 unsafe 使用量 |
| 分支策略 | `feat/rust-SAS` 与 `main` 的合并方案（`merge --no-ff` + `pre-audit-baseline` tag） |

### R0 交付物

- [ ] `deny.toml` + CI 集成
- [ ] `rustfmt.toml` + `clippy.toml`
- [ ] `docs/audit/unsafe-audit-baseline.md`
- [ ] `docs/audit/dependency-audit.md`
- [ ] `docs/diagrams/crate-dependency-graph.md`（Mermaid）
- [ ] 模块 README 模板 `docs/templates/module-readme-template.md`
- [ ] ADR 模板 `docs/templates/adr-template.md` + `docs/decisions/` 目录
- [ ] 分支合并完成（含 `pre-audit-baseline` tag）

---

## R1 — 原语层

**目标**：排查依赖树底部的叶子 crate，确保类型设计、API 边界、文档和测试完备。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| `memory_types` | 地址和页帧的 newtype 封装（`PhysAddr` / `VirtAddr` / `Frame` / `Page` / `Span`） |
| `config` | 内核常量与编译期不变量 |
| `span` | 通用范围类型 |
| `build_common` | 构建脚本共享逻辑 |

这些是无依赖的叶子 crate，变更不会破坏同层其他模块，但会影响所有上层。

### R1 交付物

- [ ] 各 crate README（按模板）
- [ ] 单元测试补全
- [ ] `memory_types` 类型关系图（Mermaid class diagram）
- [ ] API 变更（如有）

---

## R2 — 同步与 Per-CPU

**目标**：审查同步原语的正确性和 Rust 范式合理性，这是上层所有并发代码的基石。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| `sync` | 中断安全的自旋锁原语（SpinLock、Guard、锁级别机制） |
| `interrupt_state` | 中断状态 proof token（`HeldInterrupts`） |
| `per_cpu` | Per-CPU 数据（`#[cpu_local]` 宏、SMP 初始化） |
| `macros` | 过程宏 |

### R2 交付物

- [ ] 锁层级关系图（Mermaid）
- [ ] 中断状态生命周期图
- [ ] Per-CPU 初始化时序图
- [ ] `trybuild` 测试（如适用）
- [ ] 各 crate README 更新

---

## R3 — 内存子系统

**目标**：这是项目中 Rust 范式最成熟的部分（typestate、所有权模型），但也是最复杂的。重点是查漏补缺和文档化。

### 审查范围

按依赖关系排查：

```
memory_types ← frame_allocator ← page_allocator
                                ← page_table_entry ← paging ← memory
                                ← heap
                                ← tlb
```

| 模块 | 功能定位 |
|------|----------|
| `frame_allocator` | 物理帧分配器（typestate 状态机、buddy allocator） |
| `page_table_entry` | 页表项抽象（PTE flags、W^X 安全） |
| `paging` | 页表管理（MappedPages 所有权、MMIO 映射） |
| `memory` | 地址空间与 VMA 管理 |
| `heap` | 内核堆分配器 |
| `tlb` | TLB 管理与 shootdown |
| `page_allocator` | 虚拟页分配器 |

### R3 交付物

- [ ] 内存子系统全景依赖图
- [ ] 帧生命周期状态机图（Mermaid state diagram，从代码验证）
- [ ] 页表映射/解映射时序图
- [ ] VMA 操作时序图
- [ ] 各 crate README
- [ ] Unsafe 审计（此层 unsafe 最密集）
- [ ] 单元测试 + 系统测试补全

---

## R4 — 架构层

**目标**：审查架构抽象的完备性，确保新增架构的扩展成本最小。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| 架构抽象 Trait | `ArchOps` 及其关联类型，零成本分发机制 |
| Boot Flow | `_start` → `kernel_init()` 分阶段初始化、SMP 启动 |
| Console | 早期控制台（SBI putchar / PL011 MMIO） |
| Interrupt | 中断控制器配置（PLIC / GIC）、trap 分发 |
| Timer | 定时器初始化与 tick 处理 |

> R4.1（架构抽象 Trait）和 R4.3（Console/Interrupt/Timer）可与 R3 并行。
> R4.2（Boot Flow）依赖 R3 的内存初始化变更，需在 R3 稳定后进行。

### R4 交付物

- [ ] 架构抽象 trait 接口文档
- [ ] 启动时序图（Mermaid sequence diagram）
- [ ] 中断处理流程图
- [ ] 架构扩展指南（"如何添加新架构"）
- [ ] 系统测试：SMP 启动验证

---

## R5 — 任务管理

**目标**：这是最可能存在 C++ 范式残留的区域。TCB 设计、调度器接口、上下文切换都需要深度审查。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| TCB | `TaskControlBlock` 结构设计、字段原子性、所有权模型 |
| 调度器 | `Scheduler` trait、调度策略分发（CFS / FIFO / RR）、Per-CPU 调度器状态 |
| 上下文切换 | `switch_to()` 及其 lock handoff 机制 |
| 信号与等待 | SAS 下的信号模型、WaitQueue 实现 |

### R5 交付物

- [ ] TCB 字段语义文档
- [ ] Task 生命周期状态机图
- [ ] 上下文切换时序图（含 SMP lock handoff）
- [ ] 调度算法对比文档
- [ ] 系统测试：task spawn/exit/wait/signal
- [ ] 独立测试：调度器公平性验证（可选）

---

## R6 — 设备与文件系统

**目标**：审查运行时多态（`dyn Device` / `dyn FileSystem`）的合理性，审查 FD 表设计。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| Device Framework | 设备注册/发现框架、设备生命周期 |
| HAL | `SimpleKernelHal` — VirtIO 驱动的硬件抽象 |
| VFS | `FileSystem` trait、路径解析、mount 管理 |
| RamFS / FatFS | 具体文件系统实现 |
| FD Table | 文件描述符表、dup/close 语义 |

### R6 交付物

- [ ] 设备注册/发现流程图
- [ ] VFS 操作时序图（open → read → write → close）
- [ ] FD 表设计文档
- [ ] POSIX 兼容性矩阵（支持 / 部分支持 / 不支持）
- [ ] 系统测试补全

---

## R7 — 系统调用与 API 网关

**目标**：SAS 架构下 syscall 是直接函数调用，审查其作为唯一公共 API 网关的设计。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| Syscall 层 | API 完备性、类型安全（newtype vs 裸 usize）、错误码统一 |
| 可见性审计 | `pub` vs `pub(crate)` 全局扫描、crate 边界隔离验证 |

### R7 交付物

- [ ] Syscall 清单（编号 / 签名 / 实现状态）
- [ ] POSIX 兼容性路线图
- [ ] 错误处理统一方案文档
- [ ] 可见性审计报告（`pub` 清单 + 整改建议）
- [ ] 系统测试：syscall 集成测试

---

## R8 — 集成与收尾

**目标**：全局性工作，包括文档重写、CI 重写、项目重组。

### 审查范围

| 子任务 | 内容 |
|--------|------|
| 文档重写 | `README.md`、`00-概述.md`、`CLAUDE.md`、模块 README、Rustdoc、架构图集 |
| CI 重写 | Matrix 构建、测试分层并行、质量门全链路、自动发布 |
| 项目重组 | crate 合并/拆分评估、`src/` 目录结构、测试目录、`3rd/` 子模块清理 |
| 分支合并 | 审计分支合入 `main`、历史分支清理、分支保护规则 |
| 审计产物 CI 守护 | 见下方 TODO |

#### 审计产物 CI 守护

> **TODO**: 审计过程中产出的基线和规范（unsafe 审计基线、锁层级图、依赖图等）需要 CI 自动守护，
> 防止后续开发导致规范漂移。每次 CI 运行时自动检查：
>
> - unsafe 数量是否超过基线（`unsafe-audit-baseline.md` 中的记录）
> - 文档中引用的函数/类型是否仍然存在（文档与代码一致性）
> - ADR 中标记为"已接受"的决策，相关代码是否仍符合决策内容
> - 依赖图（Mermaid）是否与实际 `cargo metadata` 输出一致
>
> 具体实现方式在 R0 CI 审计时一并设计。

### R8 交付物

- [ ] 完整文档集
- [ ] 新版 CI pipeline（含审计产物守护）
- [ ] 清理后的分支结构
- [ ] 项目 v1.0 里程碑标记

---

## 每个 Phase 的标准排查流程

每个模块/crate 排查时，按以下 checklist 执行。
具体深入哪些维度、发现哪些问题，由排查过程中根据代码实际状态决定。

### 代码审查

- [ ] 读 trait 定义——契约是否清晰？doc comment 是否完整？
- [ ] 读 impl 块——是否充分利用了 Rust 类型系统？
- [ ] 读 unsafe 块——SAFETY 注释是否充分？不变量是否成立？能否消除？
- [ ] 读 error handling——Result / Option 使用是否一致？
- [ ] 读全局状态——`static` 是否都有锁保护或 Once 初始化？
- [ ] 读 `pub` 接口——是否最小化暴露？

### Rust 范式检查

- [ ] 所有权：资源是否有明确的唯一所有者？
- [ ] 生命周期：是否有不必要的 `'static`？
- [ ] Typestate：状态机是否可以编码到类型中？
- [ ] RAII：资源获取/释放是否通过 Drop 自动管理？
- [ ] 零成本抽象：泛型 vs trait object 的选择是否合理？
- [ ] 错误处理：`?` 传播 vs `expect` vs `match` 的使用是否恰当？

### 并发安全检查

- [ ] `Send`/`Sync`：类型的 `Send`/`Sync` 约束是否正确？手动 `unsafe impl` 是否有充分理由？
- [ ] 多核竞态：共享可变状态是否有锁/原子操作保护？跨核访问路径是否遗漏？
- [ ] 中断重入：中断处理路径是否可能与被中断代码竞争同一资源？锁是否 interrupt-aware？
- [ ] 锁序：多锁场景下获取顺序是否一致？是否可能死锁？
- [ ] Atomic ordering：`Ordering` 选择是否正确（`Relaxed` / `Acquire` / `Release` / `SeqCst`）？是否过度使用 `SeqCst`？
- [ ] 初始化竞态：`spin::Once` / `static` 初始化在 SMP 启动时是否有竞态窗口？

### 依赖与版本检查

- [ ] 第三方 crate 版本：是否使用了最新稳定版？是否有已知 CVE 或 deprecated API？
- [ ] Rust nightly 特性：是否用到了已稳定的 feature flag（可移除 `#![feature(...)]`）？是否有更新的语言特性可以简化代码？
- [ ] crate 替代评估：当前手写的功能是否有成熟的 `no_std` crate 可替代？列出候选 crate 及其优缺点，标记为 ADR 待决（不要自行替换）

### 参考内核对比

- [ ] 与参考内核（Linux / Theseus / Redox / Zephyr / µFork）中对应模块的实现思路对比
- [ ] 记录设计差异及其原因（SimpleKernel 的 SAS 架构 / 教学目标可能导致合理的差异）
- [ ] 如果参考内核有明显更优的设计，列入设计讨论点（不要自行采纳）

### 文档输出

- [ ] 模块 README（按模板）
- [ ] 关键类型的生命周期图（Mermaid）
- [ ] 关键操作的时序图（Mermaid）
- [ ] 依赖关系图（如有变更）

### 测试

- [ ] 现有测试是否覆盖核心路径？
- [ ] 边界条件、错误路径是否测试？
- [ ] 是否需要新增系统测试 / 独立测试？

---

## 参考内核的具体使用指南

完整参考文献（含论文链接）见 `docs/design/references.md`。

| 参考内核 | 何时参考 | 参考什么 | 不参考什么 |
|----------|----------|----------|------------|
| **Linux** | 审查子系统接口设计时 | VFS ops 接口、`sched_class` 设计、`mm_struct`/`vm_area_struct`、信号处理框架、lockdep | 具体 C 实现、CONFIG 宏体系、模块加载 |
| **Theseus** | 审查 Rust 类型系统利用和 SAS 隔离时 | `MappedPages` RAII、typestate、crate 隔离、`#![forbid(unsafe_code)]` APP 隔离 | Theseus 特有的 live evolution 机制 |
| **Redox** | 审查 API 设计和 error handling 时 | `syscall` crate 设计、scheme VFS、`Error` 统一处理 | 微内核的用户态驱动模型 |
| **Tock** | 审查访问控制和嵌入式模式时 | `unsafe trait` capability 模式、Grant 内存模型、capsule 隔离 | MPU 硬件隔离（SimpleKernel 无 MPU） |
| **Asterinas** | 审查内核内特权分离时 | Framekernel 架构（unsafe framework + safe services）、OSTD 安全抽象层 | Linux ABI 兼容层细节 |
| **Zephyr** | 审查设备模型和嵌入式设计时 | device model、devicetree 绑定、轻量级线程模型、电源管理 | Zephyr 特有的 Kconfig 体系、行业安全认证流程 |
| **rCore** | 审查 RISC-V 实现时 | RISC-V 启动流程、页表实现、教学内核结构 | 传统用户态/内核态分离模型 |
| **µFork** | 审查 POSIX 兼容策略时 | 哪些 POSIX 语义原样保留、哪些重新诠释、capability 与 FD 的映射 | Actor model 本身（与 SAS 架构不兼容） |

---

## 时间与优先级

| Phase | 预估工作量 | 优先级 | 前置依赖 |
|-------|-----------|--------|----------|
| R0 | 3-5 天 | P0 — 必须先做 | 无 |
| R1 | 2-3 天 | P0 | R0 |
| R2 | 5-7 天 | P0 | R1 |
| R3 | 7-10 天 | P1 | R2 |
| R4 | 5-7 天 | P1 | R2（Boot Flow 依赖 R3 内存初始化，架构抽象/Console/Timer 可与 R3 并行） |
| R5 | 7-10 天 | P1 | R3, R4 |
| R6 | 5-7 天 | P2 | R5 |
| R7 | 3-5 天 | P2 | R6 |
| R8 | 5-7 天 | P3 | 全部 |

> 注：工作量为单人全职估算，AI 辅助可显著压缩。R3/R4 可部分并行。

---

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-04-02 | 初版——基于全项目代码审阅、Git 历史分析和设计文档评审 |
| 2026-04-02 | 重构——移除模块级详细排查维度表，改为按功能分层；新增协作流程、并发安全 checklist、依赖与版本检查、参考内核对比（含 Zephyr）；ADR 机制集成；审计产物 CI 守护 TODO |
