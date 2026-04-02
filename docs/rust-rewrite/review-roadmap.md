# SimpleKernel 全项目深度审计 Roadmap

> **目标**：自底向上排查每个组件，将 C++ 遗留范式转换为地道 Rust，同时完善文档、测试、CI 与工具链。
>
> **参考内核**：Linux（子系统接口设计）、Theseus（typestate / crate 隔离）、Redox（scheme VFS / error handling）、µFork（POSIX 兼容策略）
>
> **工作分支**：`feat/rust-SAS`（当前活跃，领先 `main` 234 commits）

---

## 总体原则

| 原则 | 说明 |
|------|------|
| **自底向上** | 从无依赖的叶子 crate 开始，逐层向上，每层稳定后再动上层 |
| **每步可验证** | 每个 Phase 结束时必须：编译通过 + 现有测试全绿 + 新增测试覆盖变更 |
| **文档即产出** | 排查过程中同步输出模块文档（README、时序图、生命周期图、依赖图） |
| **参考即标注** | 借鉴外部内核的设计必须在代码/文档中标注出处（`[Linux: fs/namei.c]`、`[Theseus: MappedPages]`） |
| **不做无测试的重构** | 任何架构变更必须先有测试兜底，或先补测试再改 |

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

---

## R0 — 基线建立

**目标**：搭建排查所需的基础设施，建立度量基线。

### R0.1 CI 审计与重写

| 检查项 | 当前状态 | 目标 |
|--------|----------|------|
| `workflow.yml` 结构 | 3 jobs（check/build-riscv64/build-aarch64） | 评估是否需要拆分、增加 matrix |
| 系统测试稳定性 | PR 3 次 / push 10 次 | 确认是否足够，考虑 flaky test 检测 |
| 依赖安全扫描 | ❌ 无 `cargo-deny` | 新增 `deny.toml` + CI job |
| Unsafe 统计 | ❌ 无自动化 | 新增 `cargo-geiger` 或自定义脚本，输出 unsafe 基线报告 |
| 代码覆盖率 | ❌ 无 | 评估 `cargo-llvm-cov`（unit test 部分） |
| Clippy 配置 | 仅 `-D warnings` | 新增 `clippy.toml`，启用额外 lint（`cognitive_complexity`, `unwrap_used` 等） |
| rustfmt 配置 | ❌ 无 `rustfmt.toml` | 新建，锁定 `max_width = 100` 等规则 |
| Docker 镜像 | 手动触发重建 | 评估自动化触发策略 |

### R0.2 文档基础设施

| 产出 | 说明 |
|------|------|
| 文档模板 | 制定模块 README 模板（概述 / 设计决策 / 依赖图 / 生命周期 / API / 测试） |
| 图表工具链 | 选定 Mermaid（可嵌入 Markdown），建立 `docs/diagrams/` 目录 |
| 项目级依赖图 | 用 `cargo-depgraph` 或手动 Mermaid 绘制 crate 间依赖关系 |
| Rustdoc 发布 | 配置 `cargo doc` → GitHub Pages（`gh-pages` 分支已存在） |

### R0.3 Unsafe 审计基线

- 全量扫描所有 `unsafe` 块，输出清单（文件、行号、SAFETY 注释有无）
- 分类：必要（FFI/汇编/裸指针操作） vs 可消除（可用安全抽象替代）
- 输出 `docs/rust-rewrite/unsafe-audit-baseline.md`

### R0.4 依赖审计

| 检查项 | 说明 |
|--------|------|
| Git 依赖 | `fatfs`（rafalh fork）、`aarch64-cpu`（MRNIU fork）—— 评估是否可上游化或锁定 rev |
| 版本锁定 | 确认 `Cargo.lock` 是否提交（裸机项目应提交） |
| 许可证 | `cargo-deny` advisories + licenses 检查 |
| Unsafe 使用 | 第三方 crate 中的 unsafe 量（`spin`, `buddy_system_allocator` 等） |

### R0.5 分支策略

- 当前 `feat/rust-SAS` 领先 `main` 234 commits，`main` 停留在 signal 开发
- **决策点**：是否在审计完成前先将 `feat/rust-SAS` 合并到 `main`？
- 建议：先合并（或 force-push），让 `main` 成为单一真相源，后续审计直接在 `main` 上进行

### R0 交付物

- [ ] `deny.toml` + CI 集成
- [ ] `rustfmt.toml` + `clippy.toml`
- [ ] `docs/rust-rewrite/unsafe-audit-baseline.md`
- [ ] `docs/rust-rewrite/dependency-audit.md`
- [ ] `docs/diagrams/crate-dependency-graph.md`（Mermaid）
- [ ] 模块 README 模板 `docs/templates/module-readme-template.md`
- [ ] 分支合并完成

---

## R1 — 原语层

**目标**：排查依赖树底部的叶子 crate，确保类型设计、API 边界、文档和测试完备。

### R1.1 `memory_types`

| 排查维度 | 检查内容 |
|----------|----------|
| 类型设计 | `PhysAddr` / `VirtAddr` / `Frame<P>` / `Page<P>` / `Span<T>` 的 newtype 封装是否完备 |
| 算术安全 | 地址运算是否有溢出检查？`checked_add` vs `wrapping_add` 策略是否一致？ |
| 泛型设计 | `PageSize` trait 是否足够灵活（4K / 2M / 1G）？是否需要 const generic 替代？ |
| 参考 | [Theseus: `memory_structs`] — 对比 `VirtualAddress` / `PhysicalAddress` 设计 |
| 测试 | 对齐、溢出、边界、Display/Debug 格式 |
| 文档 | README + 类型生命周期图 |

### R1.2 `config`

| 排查维度 | 检查内容 |
|----------|----------|
| 常量分类 | 是否有常量应该变为运行时从 FDT 读取？（如 `MAX_CORE_COUNT`） |
| const assert | 是否覆盖所有不变量？ |
| 架构差异 | `cfg` 分支是否清晰？ |

### R1.3 `span`

| 排查维度 | 检查内容 |
|----------|----------|
| 泛型设计 | 通用范围类型是否过度泛化或不足？ |
| 与标准库 | 与 `Range<T>` 的关系——是否应实现 `RangeBounds`？ |

### R1.4 `build_common`

| 排查维度 | 检查内容 |
|----------|----------|
| 职责边界 | 是否承载了不属于 build script 的逻辑？ |
| 架构扩展 | 新增架构时修改成本 |

### R1 交付物

- [ ] 各 crate README（按模板）
- [ ] 单元测试补全
- [ ] `memory_types` 类型关系图（Mermaid class diagram）
- [ ] API 变更（如有）

---

## R2 — 同步与 Per-CPU

**目标**：审查同步原语的正确性和 Rust 范式先进性，这是上层所有并发代码的基石。

### R2.1 `sync`

| 排查维度 | 检查内容 |
|----------|----------|
| 锁级别 | 当前运行时 panic 检测死锁 → 是否可用 `const generic LEVEL` 做编译期检查？ |
| RawLock trait | 参数化锁算法设计是否完备？是否需要 RwLock / Ticket Lock？ |
| Guard 安全 | `PhantomData<*mut ()>` 的 `!Send` 约束是否覆盖所有场景？ |
| 参考 | [Linux: `lockdep`] 锁依赖检测思路、[Theseus: `DeadlockPrevention` trait] |
| 文档 | 锁层级图（Mermaid）、获取/释放时序图 |

### R2.2 `interrupt_state`

| 排查维度 | 检查内容 |
|----------|----------|
| Proof token | `HeldInterrupts` 设计是否可扩展（嵌套中断场景）？ |
| 架构 trait | `ArchInterruptState` 分发是否干净？ |

### R2.3 `per_cpu`

| 排查维度 | 检查内容 |
|----------|----------|
| `#[cpu_local]` 宏 | 生成代码质量、编译错误信息是否友好？ |
| 初始化时序 | primary vs secondary core 初始化顺序是否有竞态？ |
| `SyncUnsafeCell` 使用 | 哪些可以替换为 `#[cpu_local]` 或 `AtomicXxx`？ |
| 参考 | [Linux: `DEFINE_PER_CPU`]、[Theseus: `CpuLocalData`] |
| 文档 | Per-CPU 内存布局图、初始化时序图 |

### R2.4 `macros`

| 排查维度 | 检查内容 |
|----------|----------|
| proc-macro 质量 | 错误报告是否精确（span 指向正确位置）？ |
| 可测试性 | 是否有 proc-macro 的编译测试（`trybuild`）？ |

### R2 交付物

- [ ] 锁层级关系图（Mermaid）
- [ ] 中断状态生命周期图
- [ ] Per-CPU 初始化时序图
- [ ] `trybuild` 测试（如适用）
- [ ] 各 crate README 更新
- [ ] 编译期锁级别 PoC（如可行）

---

## R3 — 内存子系统

**目标**：这是项目中 Rust 范式最成熟的部分（typestate、所有权模型），但也是最复杂的。重点是查漏补缺和文档化。

### 排查顺序（按依赖关系）

```
memory_types ← frame_allocator ← page_allocator
                                ← page_table_entry ← paging ← memory
                                ← heap
                                ← tlb
```

### R3.1 `frame_allocator`

| 排查维度 | 检查内容 |
|----------|----------|
| Typestate 完备性 | `Free → Allocated → Mapped → Unmapped → Free` 闭环是否无泄漏路径？ |
| Drop 语义 | 各状态下 Drop 行为是否正确（Mapped drop = panic / Allocated drop = 回收）？ |
| 性能 | buddy allocator 在碎片化场景下的表现？是否需要 benchmark？ |
| 参考 | [Theseus: `frame_allocator`] — 对比 typestate 设计差异 |
| 文档 | 状态机转换图（已有但需验证与代码一致性） |

### R3.2 `page_table_entry`

| 排查维度 | 检查内容 |
|----------|----------|
| PteFlagsOps trait | builder 方法是否符合 Rust builder pattern 惯例？ |
| W^X 安全 | 是否在 API 层面阻止同时设置 W+X？（已有测试，确认 API 层是否强制） |
| 规范引用 | RISC-V Privileged Spec / Arm ARM 链接是否完整？ |

### R3.3 `paging`

| 排查维度 | 检查内容 |
|----------|----------|
| MappedPages 所有权 | 帧是否随 MappedPages drop 正确归还？ |
| 无锁化 | Git 历史显示 CAS 方案曾 revert——当前方案是什么？是否需要重新评估？ |
| MmioRegion | `mem::forget` 泄漏 vs `ManuallyDrop` vs `'static` 引用——选定一种方案 |
| 参考 | [Theseus: `page_table_entry`, `MappedPages`] |

### R3.4 `memory`

| 排查维度 | 检查内容 |
|----------|----------|
| AddressSpace / VMA | VMA 管理是否需要红黑树（Linux `maple_tree`）？当前 Vec 是否有性能问题？ |
| 初始化流程 | `memory::init()` 是否可以用 typestate 表示（未初始化 / 堆可用 / 页表激活）？ |
| 参考 | [Linux: `mm_struct` / `vm_area_struct`]、[Theseus: `MappedPages` 作为 VMA] |

### R3.5 `heap` / `tlb` / `page_allocator`

- `heap`：堆分配器选择（`buddy_system_allocator`）是否最优？
- `tlb`：TLB shootdown 策略是否完整（IPI 驱动 / 阈值切换）？
- `page_allocator`：虚拟页分配器与 Linux `vmalloc` 区域管理的对比

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

### R4.1 架构抽象 Trait

| 排查维度 | 检查内容 |
|----------|----------|
| `ArchOps` trait | 所有 associated functions 是否都是必要的？是否有遗漏？ |
| 零成本保证 | type alias 分发是否在所有路径上避免了 `dyn` 开销？ |
| 扩展性 | 假设新增 x86_64——需要改多少文件？ |
| 参考 | [Theseus: `kernel/` 的 arch 抽象]、[Redox: `arch/` 模块组织] |

### R4.2 Boot Flow

| 排查维度 | 检查内容 |
|----------|----------|
| `InitLevel` 设计 | 枚举 vs typestate——是否值得用 typestate 表示初始化阶段？ |
| SMP 启动 | primary/secondary 分发是否有竞态？`PRIMARY_BOOTED` atomic 是否足够？ |
| 参考 | [Linux: `start_kernel()` 初始化序列]、[Zephyr: `z_cstart()`] |
| 文档 | 完整启动时序图（含 firmware → _start → kernel_init → idle） |

### R4.3 Console / Interrupt / Timer

| 排查维度 | 检查内容 |
|----------|----------|
| Console | SBI putchar vs PL011 MMIO——抽象是否统一？ |
| Interrupt | PLIC/GIC 配置是否 trait 化？中断注册/注销是否有 RAII guard？ |
| Timer | `HW_FREQ` 全局变量 vs 初始化时写入 config——设计选择 |
| 文档 | 中断处理流程图（trap → dispatch → handler → return） |

### R4 交付物

- [ ] 架构抽象 trait 接口文档
- [ ] 启动时序图（Mermaid sequence diagram）
- [ ] 中断处理流程图
- [ ] 架构扩展指南（"如何添加新架构"）
- [ ] 系统测试：SMP 启动验证

---

## R5 — 任务管理

**目标**：这是最可能存在 C++ 范式残留的区域。TCB 设计、调度器接口、上下文切换都需要深度审查。

### R5.1 TCB 设计

| 排查维度 | 检查内容 |
|----------|----------|
| `SyncUnsafeCell<CalleeSavedContext>` | 是否可替换为更安全的抽象？ |
| 字段原子性 | `AtomicTaskState` / `AtomicI32` 的 memory ordering 是否正确？ |
| Typestate | Task 状态转换能否用 typestate 编码（`Task<Running>` / `Task<Sleeping>` / ...）？ |
| 所有权模型 | `Arc<TaskControlBlock>` 的引用图——谁持有、何时释放？ |
| 参考 | [Linux: `task_struct`]、[Theseus: `Task`]、[Redox: `Context`] |
| 文档 | TCB 生命周期图、所有权传递图 |

### R5.2 调度器

| 排查维度 | 检查内容 |
|----------|----------|
| `Scheduler` trait | 接口是否完备？enqueue / dequeue / pick_next 之外是否需要更多方法？ |
| `SchedPolicy` enum | 静态分发 vs trait object——当前 enum dispatch 是最优解吗？ |
| Per-CPU 调度器状态 | `PER_CPU_SCHED: SyncUnsafeCell<[...]>` → 应迁移到 `#[cpu_local]` |
| CFS 实现 | vruntime 溢出处理、权重计算精度 |
| 参考 | [Linux: `sched_class` / `fair.c`]、[Redox: round-robin 实现] |
| 文档 | 调度决策时序图、CFS vruntime 更新流程 |

### R5.3 上下文切换

| 排查维度 | 检查内容 |
|----------|----------|
| Lock handoff | `lock_manual()` / `unlock_manual()` 绕过 RAII——是否有更安全的方案？ |
| 栈切换安全 | `switch_to()` 的 unsafe 合约是否文档化？ |
| 参考 | [Linux: `context_switch()` / `finish_task_switch()`] |
| 文档 | 上下文切换时序图（含锁交接） |

### R5.4 信号与等待

| 排查维度 | 检查内容 |
|----------|----------|
| 信号模型 | 当前实现 vs POSIX 信号语义——SAS 下信号的含义是什么？ |
| WaitQueue | 实现方式是否地道？是否需要 `Waker` pattern（类 Rust `Future`）？ |
| 参考 | [µFork: 对 POSIX 信号的兼容策略]、[Linux: `signal.c`] |

### R5 交付物

- [ ] TCB 字段语义文档
- [ ] Task 生命周期状态机图
- [ ] 上下文切换时序图（含 SMP lock handoff）
- [ ] 调度算法对比文档
- [ ] 系统测试：task spawn/exit/wait/signal
- [ ] 独立测试：调度器公平性验证（可选）

---

## R6 — 设备与文件系统

**目标**：审查运行时多态（`dyn Device` / `dyn FileSystem`）的必要性，优化 FD 表设计。

### R6.1 Device Framework

| 排查维度 | 检查内容 |
|----------|----------|
| `Vec<Box<dyn Device>>` | 是否需要类型索引查找（`TypeId` → device）？ |
| 设备生命周期 | 热插拔场景（虽然 QEMU 不需要，但接口应考虑） |
| HAL trait | `SimpleKernelHal` 的 unsafe 合约 |
| 参考 | [Linux: `struct device` / `bus_type`]、[Zephyr: device model] |

### R6.2 VFS

| 排查维度 | 检查内容 |
|----------|----------|
| `FileSystem` trait | 接口是否与 POSIX 语义对齐？缺失哪些操作（stat/chmod/link）？ |
| `dyn FileSystem` | 是否可改为 enum dispatch（已知 FS 类型有限）？ |
| 路径解析 | 线性扫描 mount table → 性能是否可接受？ |
| 参考 | [Linux: `file_operations` / `inode_operations`]、[Redox: scheme-based VFS]、[µFork: POSIX 兼容层] |

### R6.3 FD Table

| 排查维度 | 检查内容 |
|----------|----------|
| `Arc<SpinLock<File>>` | dup 场景是否真的需要 Arc？fork 语义下的 COW 考虑？ |
| FD 分配策略 | Vec<Option<...>> 的最低空闲 FD 查找——是否需要 bitmap？ |

### R6 交付物

- [ ] 设备注册/发现流程图
- [ ] VFS 操作时序图（open → read → write → close）
- [ ] FD 表设计文档
- [ ] POSIX 兼容性矩阵（支持 / 部分支持 / 不支持）
- [ ] 系统测试补全

---

## R7 — 系统调用与 API 网关

**目标**：SAS 架构下 syscall 是直接函数调用，审查其作为唯一公共 API 网关的设计。

### R7.1 Syscall 层

| 排查维度 | 检查内容 |
|----------|----------|
| API 完备性 | 当前实现了哪些 syscall？缺失哪些核心 POSIX 调用？ |
| 类型安全 | 参数/返回值是否使用了 newtype（`Fd`, `Pid` 等）而非裸 `usize`？ |
| 错误码 | `ErrorCode` 是否覆盖所有 POSIX errno？是否需要与 `TaskError` / `FsError` 统一？ |
| 参考 | [µFork: POSIX 兼容策略——哪些 syscall 原样保留、哪些重新诠释]、[Redox: `syscall` crate] |

### R7.2 可见性审计

| 排查维度 | 检查内容 |
|----------|----------|
| `pub` vs `pub(crate)` | 是否有模块暴露了不该暴露的接口？ |
| SAS 隔离 | crate 边界是否真正阻止了越权访问？ |
| 参考 | [Theseus: crate 隔离模型] |

### R7 交付物

- [ ] Syscall 清单（编号 / 签名 / 实现状态）
- [ ] POSIX 兼容性路线图
- [ ] 错误处理统一方案文档
- [ ] 可见性审计报告（`pub` 清单 + 整改建议）
- [ ] 系统测试：syscall 集成测试

---

## R8 — 集成与收尾

**目标**：全局性工作，包括文档重写、CI 重写、项目重组。

### R8.1 文档重写

| 产出 | 说明 |
|------|------|
| `README.md` | 重写项目首页，反映当前架构和功能 |
| `docs/rust-rewrite/00-概述.md` | 更新总纲，标记已完成/变更的设计决策 |
| `AGENTS.md` / `CLAUDE.md` | 根据审计结果更新 AI 辅助指令 |
| 模块 README | 确保所有 crate 和 `src/` 子模块都有 README |
| API 文档 | `cargo doc` 生成，发布到 GitHub Pages |
| 架构图集 | 汇总所有 Mermaid 图到 `docs/diagrams/` |

### R8.2 CI 重写

| 项目 | 说明 |
|------|------|
| Matrix 构建 | 评估 `{arch} × {feature}` 矩阵 |
| 测试层级 | unit → system → standalone，分 job 并行 |
| 质量门 | fmt + clippy + deny + geiger + doc + test 全链路 |
| 自动发布 | Release tag → 自动构建内核镜像 + 发布 |

### R8.3 项目重组（如需要）

| 可能的变更 | 评估标准 |
|------------|----------|
| crate 合并 / 拆分 | 依赖图中是否有过度拆分或耦合过紧的情况？ |
| `src/` 目录结构 | 模块组织是否反映了当前架构（vs 迁移历史遗留）？ |
| 测试目录 | `tests/` 组织是否清晰？ |
| `3rd/` 子模块 | 是否所有子模块都还需要？ |

### R8.4 分支合并

- 将审计后的 `feat/rust-SAS`（或工作分支）合并到 `main`
- 清理历史分支（`boot`, `interrupt`, `memory`, `snmalloc`, `thread` 等）
- 设置分支保护规则

### R8 交付物

- [ ] 完整文档集
- [ ] 新版 CI pipeline
- [ ] 清理后的分支结构
- [ ] 项目 v1.0 里程碑标记

---

## 每个 Phase 的标准排查流程

每个模块/crate 排查时，按以下 checklist 执行：

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

| 参考内核 | 何时参考 | 参考什么 | 不参考什么 |
|----------|----------|----------|------------|
| **Linux** | 审查子系统接口设计时 | VFS ops 接口、`sched_class` 设计、`mm_struct`/`vm_area_struct`、信号处理框架、lockdep | 具体 C 实现、CONFIG 宏体系、模块加载 |
| **Theseus** | 审查 Rust 类型系统利用时 | `MappedPages` RAII、typestate、crate 隔离模型、`DeadlockPrevention` | Theseus 特有的 live evolution 机制 |
| **Redox** | 审查 API 设计和 error handling 时 | `syscall` crate 设计、scheme VFS、`Error` 统一处理 | 微内核的用户态驱动模型 |
| **µFork** | 审查 POSIX 兼容策略时 | 哪些 POSIX 语义原样保留、哪些重新诠释、capability 与 FD 的映射 | Actor model 本身（与 SAS 架构不兼容） |

---

## 时间与优先级

| Phase | 预估工作量 | 优先级 | 前置依赖 |
|-------|-----------|--------|----------|
| R0 | 3-5 天 | P0 — 必须先做 | 无 |
| R1 | 2-3 天 | P0 | R0 |
| R2 | 5-7 天 | P0 | R1 |
| R3 | 7-10 天 | P1 | R2 |
| R4 | 5-7 天 | P1 | R2 |
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
