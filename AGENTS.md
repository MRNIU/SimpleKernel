<!-- Copyright The SimpleKernel Contributors -->

# AGENTS.md — SimpleKernel

## 项目与任务边界

SimpleKernel 是接口驱动的 Rust `no_std` / `no_main` 学习内核，使用 nightly、edition 2024，
支持 riscv64 与 aarch64。trait 文档是契约，先理解契约再修改实现。

- 采用单地址空间 SAS：所有代码在同一特权级和地址空间，syscall 是直接函数调用。
  Rust 类型系统与 crate 可见性是隔离手段；APP 只能经 syscall 访问内核是目标，尚未全面实现。
  当前边界见 [SAS 设计](docs/design/SAS-架构设计.md) 和 `src/lib.rs` 的实际公开面。
- 先确认任务范围、branch、HEAD、index、工作区及目标目录的局部 `AGENTS.md`，保留无关改动。
  只读审查先交付发现；实现、提交、push 分别遵循用户授权，不从历史记录继承一次性授权。
- 代码是实现真值；当前设计解释边界，ADR 解释决策。历史阶段文档、旧测试通过和文件存在
  都不能证明当前验证通过、ADR 已接受或审计阶段关闭。
- 用中文说明结果。遇到影响理解的 Rust 特有概念，简要用 C/C++ 类比，并说明所有权、
  生命周期或静态约束的差异，不假设作者已熟悉 typestate、GAT、`PhantomData` 等。

## 规则与操作入口

同一规则只保留一个详细维护位置；本文件保留项目级边界和路由，局部不变量留在所属目录。

| 需要了解 | 详细入口 |
|----------|----------|
| 项目定位、快速开始、能力边界 | [README](README.md)；[英文入口](README_ENG.md) |
| 开发、按范围验证、提交和 PR | [CONTRIBUTING](CONTRIBUTING.md) |
| 可选容器配置、挂载与产物路径 | [docs/docker.md](docs/docker.md) |
| Rust、错误、unsafe、文件组织、注释等工程规则 | [docs/conventions.md](docs/conventions.md) |
| commit 格式、正文、footer、DCO | [.gitmessage](.gitmessage)；每条提交必须 `git commit --signoff` |
| 构建、运行、调试、参数、QEMU 超时与失败清理 | [xtask/README.md](xtask/README.md) |
| 独立测试、sentinel、测试清单及新增方式 | [tests/README.md](tests/README.md) |
| crate 职责、依赖与局部验证 | [crates/AGENTS.md](crates/AGENTS.md)，再读目标 crate 的 `AGENTS.md` |
| 文档类型、当前设计、ADR/RFC/Spec/Plan 模板路由 | [docs/README.md](docs/README.md) |
| 按任务组织开发操作 | 唯一项目 skill：[simplekernel-dev](.agents/skills/simplekernel-dev/SKILL.md) |

开发环境由开发者选择，本地与容器使用相同工具链和 xtask 入口。环境准备见
[贡献指南](CONTRIBUTING.md#环境与命令)；Docker / Dev Container 均为可选项。
根 README 面向普通读者；目录职责、操作手册和索引放在目录 README。
修改 docs、tests 或 xtask 时，按上表读取对应 README 中的约束；Markdown 链接不会自动加载。
局部 AGENTS 仅用于需要自动发现的目录特有约束，不为每个目录新建。
README 与适用 AGENTS 冲突时以 AGENTS 为准，并在同一变更中修正冲突。

## 关键不变量

- 保持资源唯一所有权和 RAII 释放顺序。`AllocatedFrames` 的 Drop 归还 buddy；永久持有
  必须明确表达。帧生命周期、页表权限、MMIO、DMA 各归所属 crate，不互相替代。
- 内核互斥使用项目 `sync` crate。锁选择、中断与抢占 guard、锁序及跨核约束见
  [sync 局部规则](crates/sync/AGENTS.md)；不能以 host stub 验证代替裸机并发证据。
- 中断上下文禁止堆分配/释放；涉及 `Send`/`Sync`、原子顺序、per-CPU 访问、unsafe 或
  TLB 时必须检查真实调用上下文，不能以“编译通过”代替安全证明。
- 内核不变量违反必须 fail-fast；预期资源/设备失败向上返回所属子系统错误。
  完整错误与输入校验规则见工程约定，不能通过静默 fallback 掩盖非法平台输入。
- 当前 DMA 后端只承诺 QEMU VirtIO identity mapping，不宣称真机 non-coherent DMA 保证。

## 代码导航

| 任务 | 入口 |
|------|------|
| 启动与 SMP | `src/main.rs` → `src/boot.rs`；架构入口在 `src/arch/{riscv64,aarch64}/boot.rs` |
| 架构契约、interrupt、timer、context | `src/arch/mod.rs` 的 `ArchOps` 与各架构目录 |
| 内存策略 / 帧 / 页表 / MMIO / DMA | `crates/memory/` 及 [crate 职责表](crates/AGENTS.md) |
| 调度与任务 | `src/task/`；`src/task/scheduler/mod.rs` 的 `Scheduler` |
| 驱动 probe 与 capability | `crates/device_core/src/descriptor.rs` → `src/device/platform_bus.rs` → `src/device/virtio/` |
| VFS 与文件描述符 | `src/fs/vfs.rs`、`src/fs/fd_table.rs`；边界转换在 `src/syscall/` |
| 日志与 panic | `src/logging.rs`、`src/panic.rs`、`src/elf.rs` |

## 专项审计入口

仅在用户要求项目深度审计或继续审计时，读取
[Roadmap](docs/audit/review-roadmap.md) 和 [当前进度](docs/audit/audit-progress.md)，
按 Roadmap 的协作流程、标准排查流程执行；输出结构见
[审计 prompt](docs/audit/review-session-prompt.md)。普通局部修复、审查和文档修改不自动扩展为全阶段审计。

未指定审计目标时从进度中的下一步开始。审计报告与实施分开，设计讨论点客观列出备选方案，
不代替作者决策；ADR 状态权限见 [ADR 规则](docs/adr/README.md)。参考资料按需从
[references](docs/design/references.md) 进入，不要求每个任务全量阅读。
