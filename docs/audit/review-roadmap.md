<!-- Copyright The SimpleKernel Contributors -->

# SimpleKernel 全项目深度审计 Roadmap

> **目标**：自底向上排查每个组件，将 C++ 遗留范式转换为地道 Rust，同时完善文档、测试、CI 与工具链。
>
> **参考内核**：Linux（子系统接口设计）、Theseus（typestate / crate 隔离）、Redox（scheme VFS / error handling）、Zephyr（嵌入式设备模型）、µFork（POSIX 兼容策略）
>
> **工作分支**：基线见[审计进度](audit-progress.md)；远端/主线差距以实际 Git 查询为准，不在本文维护动态 commit 数

---

## 总体原则

| 原则 | 说明 |
|------|------|
| **自底向上** | 从无依赖的叶子 crate 开始，逐层向上，每层稳定后再动上层 |
| **每步可验证** | 每个 Phase 结束时必须：编译通过 + 现有测试全绿 + 新增测试覆盖变更 |
| **全量回归** | 每个 Phase 结束后，运行全量系统测试 + 上游 Phase 的单元测试，防止底层变更静默破坏上层 |
| **文档即产出** | 排查过程中同步输出模块 AGENTS、时序图、生命周期图、依赖图 |
| **决策即记录** | 重要设计决策写入 ADR（`docs/adr/`），模板见 `docs/templates/adr-template.md` |
| **参考即标注** | 借鉴外部内核的设计必须在代码/文档中标注出处（`[Linux: fs/namei.c]`、`[Theseus: MappedPages]`） |
| **不做无测试的重构** | 任何架构变更必须先有测试兜底，或先补测试再改 |

## 协作流程

本节只适用于用户发起的专项深度审计；普通局部修改和片段 review 按根 AGENTS 与贡献指南。
启动时读取本 Roadmap、当前进度和目标所有源文件；未指定目标时读取进度中的下一步。

- 输出结构见 [审计 prompt](review-session-prompt.md)，完成报告后等待作者反馈，不直接实施。
- 设计讨论点客观列出备选方案及优缺点，不推荐方案；crate 替代仅评估，不自行替换。
  ADR 状态与接受权限以 [ADR 规则](../adr/README.md) 为准。
- 非只读任务结束时更新 [当前进度](audit-progress.md) 的状态、简短交接、下一步和证据入口；
  旧摘要/验证移入已有历史记录并标明日期与基线。只读要求优先，交接写在答复中。
- 交付物勾选仅在本 Roadmap 维护且附证据；阶段关闭另需完整覆盖、遗留处理和回归证明。

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
R1  原语层 ────── memory_types（含 Span）, config；原 build_common 已删除
│
R2  同步与 CPU ── sync, interrupt_state, per_cpu, macros
│
R3  内存子系统 ── frame_allocator → page_table_entry
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

## 交付物对账口径（2026-09-22）

本次按 `e45053ab` 代码、Git 历史、现存文档与历史验证记录核对；没有重跑构建或 QEMU。
`[x]` 表示该条交付内容有对应证据（测试项的适用日期另注），不是整个 Phase 关闭；
`[ ]` 后明确区分部分交付、待决和未找到证据。历史报告的交付不表示其中全部结论仍适用。

阶段关闭另需：审查范围完整覆盖、遗留项有确认的处理结论、相应基线的编译/全量回归和
上游测试记录，以及明确关闭记录。已有代码、文件或一轮全绿均不能单独替代这些条件。
本轮不补造阶段验收，也不把旧摘要中的“已完成目标”直接提升为阶段完成。

| 阶段 | 已有成果与对账状态 | 尚缺 / 待确认 |
|------|------------------|---------------|
| R0 | 部分完成：CI、基线报告、模板和格式/lint 入口已交付 | 项目依赖图已漂移；合并/tag、阶段关闭缺证据 |
| R1 | 部分完成：原语修复、当前 crate 手册、Span 合并已落地 | 类型图、测试补全验收及阶段关闭缺证据 |
| R2 | 部分完成：同步分层、局部手册、QEMU 测试和启动时序已有 | 锁/中断生命周期图、trybuild 适用性、完整审查/关闭缺证据 |
| R3 | 部分完成：多轮修复、当前内存设计、RAII/权限/MMIO 图及历史回归 | unsafe 覆盖闭环、运行期并发边界、多 bank / 真机范围与阶段关闭未确认 |
| R4 | 主要修复、契约、启动/中断图和 SMP/TLB 历史验证已交付 | 全阶段回归绑定与关闭记录未确认 |
| R5 | 实现及冒烟场景存在，未找到完整审计交付证据 | TCB/生命周期/切换/算法文档、专项测试与关闭待核对 |
| R6 | 部分完成：D0.5/D1/D2、指定兼容入口清理及枚举修复已落地 | VFS/FD/FAT/POSIX 交付与覆盖未闭环；D3 仅候选范围待确认 |
| R7 | syscall 实现存在，未找到完整审计交付证据 | syscall/POSIX 清单、可见性报告、边界测试及关闭缺证据 |
| R8 | 部分完成：文档治理、style follow-up、部分 CI/测试可信度修复 | 全量测试基础设施、host 一致性、产物守护、合并/里程碑关闭缺证据 |

当前工作与唯一下一步只在[审计进度](audit-progress.md)维护。
历史验证与完成切片见[归档](2026-09-22-audit-history.md)，下列测试勾选不代表本轮 HEAD 通过。

---

## R0 — 基线建立

**目标**：搭建排查所需的基础设施，建立度量基线。

### 审查范围

| 子任务 | 内容 |
|--------|------|
| CI 审计与重写 | `workflow.yml` 结构评估、系统测试稳定性、`cargo-deny`、unsafe 统计自动化、代码覆盖率、Clippy/rustfmt 配置、Docker 镜像策略 |
| 文档基础设施 | 模块 AGENTS 模板、Mermaid 图表工具链、项目级依赖图、Rustdoc 发布 |
| Unsafe 审计基线 | 全量 unsafe 扫描、分类（必要 vs 可消除）、输出基线报告 |
| 依赖审计 | Git 依赖上游化评估、版本锁定、许可证检查、第三方 unsafe 使用量 |
| 分支策略 | `feat/rust-SAS` 与 `main` 的合并方案（`merge --no-ff` + `pre-audit-baseline` tag） |

### R0 交付物

- [x] `deny.toml` + CI 集成：`.github/workflows/workflow.yml` 包含 `cargo deny check`；本轮未查询 CI 运行结果。
- [x] 格式化/lint 规则收口：无 `rustfmt.toml`，根 AGENTS 与 CI 使用默认 rustfmt、双裸机 target Clippy；风格切片见 `c0b629c3` / `2bcb49c8` / `629fab44`。
- [x] [Unsafe 基线](unsafe-audit-baseline.md)：已交付 2026-04-03 / `aff99bdd8` 历史统计，不是当前 unsafe 证明。
- [x] [依赖审计](dependency-audit.md)：历史报告已交付，版本/上游状态未经本轮重查。
- [ ] 项目级 [crate 依赖图](../diagrams/crate-dependency-graph.md)：历史产物存在，但仍列 `span`、`build_common`、`system-test`，与当前 workspace 不一致，不能按当前图完成勾选。
- [x] [模块 AGENTS 模板](../templates/local-AGENTS.md)：职责、边界、验证和不要假设等模板内容已具备。
- [x] [ADR 模板](../templates/adr-template.md) + `docs/adr/`：模板与现有 ADR 记录已具备，本轮未改接受状态。
- [ ] 分支合并（含 `pre-audit-baseline` tag）：本地 tag 列表没有该 tag，未找到本审计收尾合并证据；远端状态未查询。

---

## R1 — 原语层

**目标**：排查依赖树底部的叶子 crate，确保类型设计、API 边界、文档和测试完备。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| `memory_types` | 地址和页帧的 newtype 封装（`PhysAddr` / `VirtAddr` / `Frame` / `Page` / `Span`） |
| `config` | 内核常量与编译期不变量 |
| `Span`（原 `span` crate） | 已并入 `crates/memory_types/src/span.rs`，提交 `ff85b3f5` |
| 原 `build_common` | crate 已在 `404ad4d7` 删除，当前构建编排在 `xtask/src/build.rs`，架构汇编在 `global_asm!` 调用点；不再要求为已删除 crate 新增手册 |

这是原始审查分组，不再是当前依赖拓扑；当前成员和依赖以 `Cargo.toml` / crate manifest 为准。

### R1 交付物

- [x] 各当前 crate AGENTS：`crates/memory_types/AGENTS.md`、`crates/config/AGENTS.md` 已描述当前 API 和验证；原 span/build_common 的归属见上表。
- [ ] 单元测试补全：已有 `tests/memory-types-test/` 等回归及历史修复记录，未找到逐项覆盖验收和 R1 全范围关闭证据。
- [ ] `memory_types` 类型关系图（Mermaid class diagram）：局部 AGENTS 目前是文本关系图，尚未满足该项图表交付。
- [x] 已发生的 API 变更：Span 合并 `ff85b3f5`、地址/帧校验修复 `a2a5cf56`、当前 `crates/memory_types/src/` 可追溯；不据此关闭测试补全项。

---

## R2 — 同步与 Per-CPU

**目标**：审查同步原语的正确性和 Rust 范式合理性，这是上层所有并发代码的基石。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| `sync` | `SpinLock` 禁用抢占、`SpinLockIrq` 关中断，配合 Guard 与锁级别机制 |
| `interrupt_state` | 中断状态 proof token（`HeldInterrupts`） |
| `per_cpu` | Per-CPU 数据（`#[cpu_local]` 宏、SMP 初始化） |
| `macros` | 过程宏 |

### R2 交付物

- [ ] 锁层级关系图（Mermaid）：`crates/sync/AGENTS.md` 有层级表和文本分层，未找到对应 Mermaid 图。
- [ ] 中断状态生命周期图：`crates/interrupt_state/AGENTS.md` 有 token / 嵌套语义说明，未找到该项生命周期图。
- [x] Per-CPU 初始化时序图：[R4 主核/从核时序](../design/R4-arch-boot-sequence.md) 已覆盖 `percpu_init` / `percpu_init_smp` 顺序，与 `src/boot.rs` 对应。
- [ ] `trybuild` 测试（如适用）：未找到 trybuild 套件；适用性或免做结论待确认，不默认认定无需测试。
- [x] 各 crate AGENTS：`sync`、`interrupt_state`、`per_cpu`、`macros` 均有职责、边界和验证入口；实现有同步分层及 `tests/sync-test/`，但不替代完整 R2 审查报告。

---

## R3 — 内存子系统

**目标**：核对内存所有权、页表与映射不变量，查漏补缺并文档化；旧 typestate 方案不作为当前实现。

### 审查范围

按依赖关系排查：

```
memory_types ← frame_allocator ← page_table_entry ← paging ← memory
                                ← heap
                                ← tlb
```

| 模块 | 功能定位 |
|------|----------|
| `frame_allocator` | 物理帧分配器（`AllocatedFrames` RAII 所有权、buddy allocator） |
| `page_table_entry` | 页表项抽象（PTE flags、W^X 安全） |
| `paging` | 页表管理（identity mapping、`update_range_flags` 权限覆盖、TLB 刷新回调） |
| `memory` | 内存初始化、内存布局校验与 MMIO 类型化入口 |
| `heap` | 内核堆分配器 |
| `tlb` | TLB 管理与 shootdown |

### R3 交付物

- [x] 内存子系统全景依赖图：[当前内存设计](../design/memory-subsystem-v2.md)“分层架构”，不同于 R0 已漂移的全项目依赖图。
- [x] 帧生命周期状态机图：同文“物理帧生命周期”与 `AllocatedFrames` 分配、Drop、永久持有路径对应，已移除旧 typestate 前提。
- [x] 页表映射/权限覆盖时序图：同文“启动时序”“权限覆盖时序”，对应 `memory::init()`、`PageTable::update_range_flags()` 和 TLB guard。
- [x] MMIO 映射与 RAM 重叠校验时序图：同文“MMIO 模型”，对应 `crates/memory/src/mmio.rs`。
- [x] 各 crate AGENTS：`frame_allocator`、`page_table_entry`、`tlb`、`paging`、`heap`、`memory` 均已具备当前职责和验证入口。
- [ ] Unsafe 审计：已有 [R3 发现与修复记录](2026-05-07-r3-memory-review-findings.md)，仍需确认剩余边界与审计覆盖闭环，不能用文件头/SAFETY 扫描替代。
- [ ] 单元测试 + 系统测试补全：已有 `tests/frame-test/`、`paging-test/`、`memory-test/` 等及 2026-05/06 历史回归；运行期并发写者、多 bank RAM 和真机范围仍有限制，完整覆盖/关闭缺证据。

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

- [x] 架构抽象 trait 接口文档：`src/arch/mod.rs::ArchOps` 契约及 [移植指南](../design/R4-architecture-porting-guide.md) 的方法约束已具备。
- [x] 启动时序图：[启动与 SMP 上线时序](../design/R4-arch-boot-sequence.md)，对应 `src/boot.rs` 的 task-before-IRQ 和 online barrier。
- [x] 中断处理流程图：[中断、Timer 与 TLB 流程](../design/R4-interrupt-timer-flow.md)，包含 IRQ-exit 和 shootdown 顺序。
- [x] 架构扩展指南：[新增架构指南](../design/R4-architecture-porting-guide.md) 已覆盖契约、启动、IRQ、TLB 与最小验证；本轮将旧 `boot.S` 路径纠正为当前 `boot.rs`。
- [x] 系统测试：SMP 启动验证已有 `tests/arch-test/src/main.rs` 的 online 断言，历史 2026-05-09 定点及 2026-06-10 双架构全量记录见[归档](2026-09-22-audit-history.md)；不是当前 HEAD 的新运行结果。

---

## R5 — 任务管理

**目标**：审查 TCB 设计、调度器接口和上下文切换；是否存在范式或并发问题应以实际审查为准。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| TCB | `TaskControlBlock` 结构设计、字段原子性、所有权模型 |
| 调度器 | `Scheduler` trait、调度策略分发（CFS / FIFO / RR）、Per-CPU 调度器状态 |
| 上下文切换 | `switch_to()` 及其 lock handoff 机制 |
| 信号与等待 | SAS 下的信号模型、WaitQueue 实现 |

### R5 交付物

- [ ] TCB 字段语义文档：`src/task/tcb.rs` 有代码注释，未找到完整字段所有权/并发审计交付。
- [ ] Task 生命周期状态机图：`src/task/state.rs` 实现存在，未找到经当前代码核对的审计图。
- [ ] 上下文切换时序图（含 SMP lock handoff）：R4 覆盖上下文/IRQ 边界，未找到该项完整交付。
- [ ] 调度算法对比文档：`src/task/scheduler/` 有 CFS/FIFO/RR 实现，未找到本阶段比较与审查结果。
- [ ] 系统测试 task spawn/exit/wait/signal：`src/smoke_test.rs` 有相关场景；独立 QEMU 测试不执行 `main::bootstrap()` 的 `spawn_all()`，不能把全量独立测试通过当作这些场景全通过。
- [ ] 独立调度公平性测试（可选）：未找到；是否纳入阶段验收待确认。

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

- [x] 设备注册/发现流程图：[设备当前设计](../design/device-subsystem-current.md) 已与 D2 descriptor / FDT / capability 实现对齐。
- [ ] VFS 操作时序图（open → read → write → close）：[P7 设计](../design/P7-文件系统.md) 是早期方案，未找到该链路按当前实现验收的时序图。
- [ ] FD 表设计文档：`src/fs/fd_table.rs` 与 P7 背景存在，当前 dup/close、生命周期和审计结论缺完整交付证据。
- [ ] POSIX 兼容性矩阵（支持 / 部分支持 / 不支持）：未找到与当前实现逐项核对的矩阵，不由“POSIX 兼容编号”推断语义兼容。
- [ ] 系统测试补全：device-test 已硬断言默认 Block 与 sector 0 读取；fs-test 仅断言 RamFS/VFS，Full 初始化中的 FAT 挂载可失败返回 `false`。FAT、FD 语义等验收缺口未关闭，详见设备当前设计的覆盖表。

---

## R7 — 系统调用与 API 网关

**目标**：SAS 架构下 syscall 是直接函数调用，审查其作为唯一公共 API 网关的目标与当前多子系统 `pub` 暴露面的差距。

### 审查范围

| 模块 | 功能定位 |
|------|----------|
| Syscall 层 | API 完备性、类型安全（newtype vs 裸 usize）、错误码统一 |
| 可见性审计 | `pub` vs `pub(crate)` 全局扫描、crate 边界隔离验证 |

### R7 交付物

- [ ] Syscall 清单（编号 / 签名 / 实现状态）：`src/syscall/` 有实现及编号，未找到完整对账清单。
- [ ] POSIX 兼容性路线图：未找到已确认的审计路线；历史 P 阶段计划不能替代。
- [ ] 错误处理统一方案：`docs/conventions.md` 已规定子系统错误及边界转换，但未找到 syscall 全接口映射与适用性审查闭环。
- [ ] 可见性审计报告（`pub` 清单 + 整改建议）：`src/lib.rs` 仍公开 device/fs/task 等；R4 仅收窄 arch，唯一 syscall 网关目标未完成。
- [ ] syscall 集成测试：`src/smoke_test.rs` 使用 syscall，不等于全部公共边界已覆盖；未找到独立覆盖矩阵与关闭记录。

---

## R8 — 集成与收尾

**目标**：全局性工作，包括文档重写、CI 重写、项目重组、测试基础设施审计。

### 审查范围

| 子任务 | 内容 |
|--------|------|
| **测试基础设施审计** | 全量排查测试相关的 cfg 门控、host 模拟实现、宏开关（见下方详述） |
| 文档重写 | `README.md`、`00-概述.md`、`AGENTS.md`、模块 AGENTS、Rustdoc、架构图集 |
| CI 重写 | Matrix 构建、测试分层并行、质量门全链路、自动发布 |
| 项目重组 | crate 合并/拆分评估、`src/` 目录结构、测试目录、`3rd/` 子模块清理 |
| 分支合并 | 审计分支合入 `main`、历史分支清理、分支保护规则 |
| 审计产物 CI 守护 | 见下方 TODO |

#### 函数命名规范检查

全项目扫描所有 `pub` 函数和方法的命名，确保符合 Rust 命名惯例：

| 规则 | 正确示例 | 错误示例 |
|------|----------|----------|
| 返回 `bool` 的查询用 `is_`/`has_`/`can_` 前缀 | `is_empty()`, `is_irq_enabled()` | `check_empty()`, `irq_enabled()` |
| Getter 用名词，不加 `get_`/`read_` 前缀 | `len()`, `percpu_base()` | `get_len()`, `read_percpu_base()` |
| Setter 用 `set_` 前缀 | `set_len()`, `set_percpu_base()` | `write_percpu_base()` |
| 动作用动宾结构（动词在前） | `flush_tlb()`, `disable_irq()` | `tlb_flush()`, `irq_disable()` |
| 不用 C 风格"模块名前缀" | `arch_primitives::disable_irq()` | `arch_primitives::arch_irq_disable()` |

排查范围：`src/`、`crates/`、`tests/` 下所有 `.rs` 文件的 `pub fn` / `pub unsafe fn`。

交付物：
- 命名不规范清单（函数名 + 文件位置 + 建议修正）
- 批量重命名 PR（可分模块提交）

#### 测试基础设施审计

代码中大量为测试环境单独添加了实现和宏开关，需全量排查，确保：
1. **测试代码不污染内核代码**——内核源码应以内核设计为优先，不应为测试环境打补丁
2. **host 模拟实现与裸机行为一致**——测试有意义的前提是 host 行为能反映裸机行为

排查维度：

| 维度 | 排查内容 | 典型问题 |
|------|----------|----------|
| `#[cfg(test)]` 代码块 | 所有 crate 中的 `cfg(test)` 门控代码 | 是否引入了与裸机语义不同的行为？测试是否在验证真实逻辑？ |
| host 与裸机编译边界 | `memory_types` 的条件依赖、`arch_primitives` 的裸机 `Impl`、`ttas.rs` 的 `caller_id()` | 原 `interrupt_state/arch/host.rs` 已不存在；核验每个 host 入口是否可编译及其证据边界 |
| `#[cfg(target_os = "none")]` 门控 | 锁栈、中断检查等仅裸机生效的逻辑 | 门控是否合理？能否让测试也覆盖这些路径？ |
| `#[cfg_attr(..., allow/expect)]` | 为测试/host 环境压制的 lint | 是否掩盖了真实问题？ |
| Feature flags | `spin-timeout` 等 feature 对测试的影响 | feature 组合是否都在 CI 中测试？ |
| `CpuLocal` 在 host 上的行为 | 多线程测试中所有线程共享同一个 static | 是否导致测试结果不可靠？是否需要 thread-local 模拟？ |

交付物：
- 测试基础设施审计报告（问题清单 + 改进建议）
- host 模拟实现一致性评估
- `CpuLocal` host 行为的 ADR（是否需要 thread-local 模拟）

#### 审计产物 CI 守护

> **TODO**: 审计过程中产出的基线和规范（unsafe 审计基线、锁层级图、依赖图等）需要 CI 自动守护，
> 防止后续开发导致规范漂移。每次 CI 运行时自动检查：
>
> - unsafe 数量是否超过基线（`unsafe-audit-baseline.md` 中的记录）
> - 文档中引用的函数/类型是否仍然存在（文档与代码一致性）
> - ADR 中标记为"已接受"的决策，相关代码是否仍符合决策内容
> - 依赖图（Mermaid）是否与实际 `cargo metadata` 输出一致
>
> 具体实现方式尚待确认；已有 CI 不等于这些守护已完成。

### R8 交付物

- [ ] 函数命名检查报告 + 批量重命名：style follow-up 已完成约定/风格切片，但未找到全体 public 函数命名逐项报告。
- [ ] 测试基础设施审计报告：R4 已修复 sentinel/timeout 判定，未覆盖上文所列全部 cfg / feature / host 审计维度。
- [ ] host 模拟实现一致性评估：存在局部删除 mock 的提交 `28475136`，未找到全量一致性报告。
- [ ] `CpuLocal` host 行为 ADR：当前 ADR 索引未找到此专项决策，必要性仍待确认。
- [ ] 完整文档集：README、CONTRIBUTING、CODE_OF_CONDUCT、局部 AGENTS 等已交付；[style follow-up](2026-06-10-style-organization-followup.md) 已完成并归档，R0–R7 的上述缺项仍在。
- [ ] 新版 CI pipeline（含审计产物守护）：已有双架构构建、Clippy、deny 和重复系统测试；未见 unsafe 基线、文档符号、ADR 一致性、依赖图四项完整守护。
- [ ] 清理后的分支结构：未找到审计完成后的合并/清理证据；本轮未修改分支或查询远端。
- [ ] 本审计的 v1.0 里程碑：本地已有历史 `v1.0.0` 等 tag，不构成本轮审计关闭与发布验收，是否沿用该里程碑名称待确认。

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

- [ ] 模块 AGENTS（按模板）
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

## 原始时间与优先级估算（非当前执行队列）

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

> 此表保留原计划的估算与依赖，不代表目前必须按顺序重做已完成切片。当前唯一下一步见审计进度。

---

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-09-22 | 按 `e45053ab` 对账交付物；区分历史验证、当前实现与阶段关闭，未运行内核验证 |
| 2026-04-02 | 初版——基于全项目代码审阅、Git 历史分析和设计文档评审 |
| 2026-04-02 | 重构——移除模块级详细排查维度表，改为按功能分层；新增协作流程、并发安全 checklist、依赖与版本检查、参考内核对比（含 Zephyr）；ADR 机制集成；审计产物 CI 守护 TODO |
