<!-- Copyright The SimpleKernel Contributors -->

[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-interrupt](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/main)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

**面向 AI 的操作系统学习项目 | Interface-Driven OS Kernel for AI-Assisted Learning**

> 设计理念：定义清晰的内核接口（Rust trait），由 AI 完成实现——学习操作系统的新范式

## 目录

- [项目简介](#项目简介)
- [面向 AI 的设计理念](#面向-ai-的设计理念)
- [支持架构](#支持架构)
- [快速开始](#快速开始)
- [项目结构](#项目结构)
- [测试体系](#测试体系)
- [第三方依赖](#第三方依赖)
- [开发指南](#开发指南)
- [文档入口](#文档入口)
- [贡献指南](#贡献指南)
- [许可证](#许可证)

## 项目简介

SimpleKernel 是一个**面向 AI 辅助学习的现代化操作系统内核项目**。采用 Rust（`no_std`、nightly）编写，支持 RISC-V 64 和 AArch64 两种架构。

> **迁移状态**：项目已从 C++ 迁移到 Rust。C++ 源码（`src_cpp/`）保留作为实现参考，但不再维护。所有新开发均在 Rust 中进行。

与传统 OS 教学项目不同，SimpleKernel 采用**接口驱动（Interface-Driven）** 的设计：

- **项目主体是 trait 定义**——trait 包含文档注释（`# Safety`、`# Errors`、`# Panics`）作为契约
- **实现由 AI 完成**——AI 根据 trait 文档生成 `impl` 块
- **测试验证正确性**——单元测试（host）和系统测试（QEMU）验证实现是否符合接口契约

### 核心亮点

| 特性 | 说明 |
|------|------|
| **AI-First 设计** | trait 文档即 prompt，AI 可直接根据 trait 定义生成实现 |
| **接口与实现分离** | trait 定义契约，`impl` 块是实现，互不耦合 |
| **双架构支持** | RISC-V 64、AArch64，同一套 trait 适配不同硬件 |
| **双层测试验证** | 单元测试（`cargo test`）+ 系统测试（`cargo xtask test`，QEMU 运行） |
| **Workspace 架构** | 内核拆分为 `lib + bin`，子系统独立 crate（memory、sync、paging 等） |
| **工程化基础设施** | `xtask` 构建工具、GitHub Actions CI/CD、`rustfmt` + `clippy` |

## 面向 AI 的设计理念

### 核心工作流

```
读 trait 定义 → 理解契约 → AI 生成 impl → 测试验证
```

#### 1. 阅读 trait，理解契约

每个模块的 trait 都包含完整的接口文档：

```rust
/// 调度器抽象 trait
///
/// 所有调度算法必须实现此接口。
///
/// # Safety
/// 实现者必须保证 `pick_next()` 在持有调度锁时调用。
pub trait Scheduler: Send + Sync {
    /// 从就绪队列中选择下一个要运行的任务
    fn pick_next(&mut self) -> Option<TaskRef>;

    /// 将任务加入就绪队列
    fn enqueue(&mut self, task: TaskRef);
}
```

#### 2. 让 AI 实现

将 trait 定义作为上下文提供给 AI（如 Claude Code、GitHub Copilot 等），要求其生成 `impl` 块。trait 的文档注释就是最好的 prompt。

#### 3. 测试验证

```bash
# 单元测试
devcontainer exec --workspace-folder . cargo test

# 系统测试（QEMU）
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --timeout 30
```

#### 4. 对照参考实现

如果测试不通过，可以参考项目提供的实现进行对照和学习。

## 支持架构

| 架构 | 引导链 | 串口 | 中断控制器 | 时钟 |
|:---:|:---:|:---:|:---:|:---:|
| **RISC-V 64** | U-Boot SPL → OpenSBI → U-Boot | SBI Call | PLIC | SBI Timer |
| **AArch64** | U-Boot → ATF → OP-TEE | PL011 | GICv3 | Generic Timer |

## 快速开始

### 系统要求

- **操作系统**: Linux（推荐 Ubuntu 26.04 LTS）或 macOS
- **容器引擎**: Docker 或兼容的容器运行时
- **工具链**: 已包含在 Dev Container 中（Rust nightly、GCC 交叉编译器、QEMU 等）

### 环境搭建

**方式一：使用 Dev Container（推荐）**

```bash
git clone https://github.com/simple-xx/SimpleKernel.git
cd SimpleKernel

# 使用 VS Code 打开并在容器中重新打开
# 安装 Dev Containers 扩展后，点击左下角 >< 图标
# 选择 "Reopen in Container"

# 或使用已存在的 Dev Container CLI
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

> 也支持 **GitHub Codespaces**：点击仓库页面的 Code → Codespaces → Create codespace on main
>
> 详细说明见 [Dev Container 文档](./docs/docker.md)

> 开发环境默认优先使用 Dev Container。宿主机只保留 Docker 或兼容容器运行时、
> Git、编辑器/AI agent 和已有 Dev Container 入口工具；不要为了本项目在宿主机安装开发依赖。

#### 容器与镜像命名

常规开发优先使用 `devcontainer up/exec` 或编辑器 Dev Container 入口。需要手动创建可复用后台容器时，容器名使用当前用户名和具名 Git 分支，分支名中的 `/`、空格和其他特殊字符替换为 `-`；detached HEAD 状态不要创建常驻容器。

| 用途 | 名称 / 路径 | 说明 |
|------|-------------|------|
| Dev Container 配置 | `.devcontainer/devcontainer.json` | 本地开发入口，按 `.devcontainer/Dockerfile` 构建 |
| CI Dev Container 镜像 | `ghcr.io/simple-xx/simplekernel-dev:latest` | `workflow.yml`、`docs.yml` 使用；`dev-image.yml` 额外发布 commit SHA tag |
| 手动常驻容器 | `simplekernel-devcontainer-{username}-{branch}` | 仅在不用 Dev Container CLI 时需要；后续命令通过 `docker exec` 进入 |
| 项目运行时容器 | 无 | 内核运行在 Dev Container 内启动的 QEMU 中，不维护根目录生产容器镜像 |

**方式二：修复容器或执行明确要求的本地任务**

默认不在宿主机安装 Rust nightly、交叉编译器、QEMU、固件构建依赖或其他项目开发依赖。只有正在修复容器自身配置、文档/Git 等入口操作，或任务明确要求无需项目工具链的本地操作时，才在宿主机执行，并在 PR 中说明原因和验证边界。

### 编译与运行

以下项目命令通过 Dev Container、Codespaces 或 CI 声明的隔离环境执行；宿主机只作为 Docker/Dev Container 编排入口。

```bash
# 编译内核
devcontainer exec --workspace-folder . cargo xtask build --arch riscv64

# 在 QEMU 模拟器中运行
devcontainer exec --workspace-folder . cargo xtask run --arch riscv64 --timeout 30

# 调试（GDB 连接 localhost:1234）
devcontainer exec --workspace-folder . cargo xtask debug --arch riscv64

# 单元测试
devcontainer exec --workspace-folder . cargo test

# 系统测试（QEMU 中运行）
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --all --timeout 30
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --name panic-test --timeout 30
devcontainer exec --workspace-folder . cargo xtask test --list
```

**支持的架构：**
- `riscv64` — RISC-V 64 位
- `aarch64` — ARM 64 位

### 运行模式与验证模式

| 模式 | 用途 | 运行位置 | 是否需要硬件 | 命令入口 |
|------|------|----------|--------------|----------|
| 容器内开发 | 编译、检查、文档、pre-commit | Dev Container / Codespaces | 否 | `devcontainer exec --workspace-folder . <command>` |
| CI | DCO、fmt、clippy、单元测试、依赖审计、双架构构建和系统测试 | GitHub Actions + `ghcr.io/simple-xx/simplekernel-dev:latest` | 否 | `.github/workflows/workflow.yml` |
| QEMU 运行 | 启动内核并观察串口日志 | Dev Container / CI | 否 | `cargo xtask run --arch <arch> --timeout 30` |
| QEMU 系统测试 | 独立裸机测试二进制回归 | Dev Container / CI | 否 | `cargo xtask test --arch <arch> --timeout 30` |
| 固件构建 | OpenSBI、U-Boot、OP-TEE、ATF 构建 | Dev Container / CI | 否 | `cargo xtask firmware --arch <arch>` |
| 文档发布 | 生成 rustdoc 并部署 GitHub Pages | GitHub Actions | 否 | `.github/workflows/docs.yml` |

交互式 Bash 中运行 QEMU 相关命令时必须设置 30 秒超时；超时后清理残留 `qemu-system` 进程。CI 为稳定性使用 workflow 中声明的更长外层超时和重复次数。

### 打包与发布

SimpleKernel 当前没有独立生产容器镜像。需要保留或发布的产物必须写回仓库工作区内的声明目录，不能只留在容器临时文件系统、匿名 volume 或项目外路径。

| 产物 | 生成命令 / workflow | 宿主机可见路径 | 容器内路径 | 校验 / 发布 |
|------|---------------------|----------------|------------|-------------|
| 内核 ELF 与调试文件 | `cargo xtask build --arch <arch>` | `target/<target-triple>/<profile>/` | 同 bind mount 路径 | `cargo xtask run/test` 或 `cargo xtask build` 成功 |
| 启动产物 | `cargo xtask run/test --arch <arch>` | `target/<target-triple>/<profile>/boot/` | 同 bind mount 路径 | 包含 `boot.fit`、`boot.scr.uimg`、`rootfs.img` 等 |
| 固件产物 | `cargo xtask firmware --arch <arch>` | `target/firmware/<arch>/` | 同 bind mount 路径 | `ensure_firmware_exists()` 检查必需文件 |
| rustdoc Pages artifact | `.github/workflows/docs.yml` | CI 工作区 `docs-out/` | `docs-out/` | `actions/upload-pages-artifact` 后部署 GitHub Pages |
| Dev Container 镜像 | `.github/workflows/dev-image.yml` | GHCR | `ghcr.io/simple-xx/simplekernel-dev:{latest,sha}` | workflow build-and-push 成功 |

## 项目结构

```
SimpleKernel/
├── src/                            # 内核源码
│   ├── lib.rs                      #   库入口，re-export 所有模块
│   ├── main.rs                     #   二进制入口（_start、bootstrap）
│   ├── boot.rs                     #   kernel_init() 分级初始化接口
│   ├── arch/                       #   架构相关代码
│   │   ├── riscv64/                #     RISC-V 64 实现
│   │   └── aarch64/                #     AArch64 实现
│   ├── task/                       #   任务管理（TCB、调度器、信号）
│   ├── logging.rs                  #   日志后端（ANSI 彩色输出）
│   ├── panic.rs                    #   Panic handler + backtrace
│   └── ...
├── crates/                         # Workspace 子 crate
│   ├── memory/                     #   虚拟/物理内存管理
│   ├── sync/                       #   SpinLock（中断感知）
│   ├── paging/                     #   多级页表
│   ├── frame_allocator/            #   物理帧分配器
│   ├── per_cpu/                    #   Per-CPU 数据
│   └── ...
├── tests/                          # QEMU 系统测试（按包组织，[[bin]] 自动发现）
│   ├── test_harness/               #   公共 harness（test_main! 宏）
│   ├── heap-test/                  #   堆分配测试
│   ├── sync-test/                  #   SpinLock / 锁栈测试
│   └── ...                         #   每个 [[bin]] 独立启动 QEMU
├── xtask/                          # 构建工具（cargo xtask）
│   └── src/
│       ├── main.rs                 #   子命令分发（build/run/debug/test/firmware）
│       ├── build.rs                #   内核和测试编译
│       ├── qemu.rs                 #   QEMU 启动和 FIT 镜像生成
│       └── test.rs                 #   系统测试编排
├── docs/                           # 文档入口、设计、ADR、审计和模板
│   ├── README.md                   #   文档类型和目录说明
│   ├── conventions.md              #   工程和文档约定
│   ├── git.md                      #   Git 与 commit 规范
│   ├── design/                     #   当前设计与历史阶段设计
│   ├── adr/                  #   ADR 架构决策记录
│   ├── audit/                      #   深度审计计划与进度
│   └── templates/                  #   可复制文档模板
├── 3rd/                            # 第三方固件源码（Git Submodule）
└── .github/workflows/              # CI/CD（GitHub Actions）
```

## 测试体系

SimpleKernel 采用两层测试 + 冒烟测试：

### 单元测试（容器内 host target）

纯逻辑 crate 的 `#[test]` 模块，在容器内以宿主架构运行：

```bash
devcontainer exec --workspace-folder . cargo test -p memory_types -p config -p page_table_entry -p arch
```

覆盖范围：地址运算、PTE 编解码、常量验证等。

### 系统测试（QEMU）

每个测试是独立的 `#![no_std]` 裸机二进制，启动独立 QEMU 实例，拥有干净的内核环境。

```bash
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --all --timeout 30
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --name <name> --timeout 30
devcontainer exec --workspace-folder . cargo xtask test --list
```

测试位于 `tests/` 目录，使用 `tests/test_harness/` 提供的 `test_main!` 宏消除样板代码。

### 添加新测试

1. 创建 `tests/my-test/`，包含 `Cargo.toml` 和 `src/main.rs`
2. `src/main.rs` 使用 `test_harness::test_main!` 宏
3. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径

新增独立测试的细节以 `tests/test_harness/`、现有 `tests/*/Cargo.toml` 和 `xtask/src/test.rs` 为准。

## 第三方依赖

### Rust Crate 依赖

| Crate | 用途 |
|-------|------|
| [`spin`](https://crates.io/crates/spin) | `Once<T>` 单例初始化 |
| [`bitflags`](https://crates.io/crates/bitflags) | 类型安全的位标志（寄存器字段、页表项） |
| [`bitfield-struct`](https://crates.io/crates/bitfield-struct) | 过程宏位域结构体（硬件寄存器字段定义） |
| [`log`](https://crates.io/crates/log) | 日志门面，后端在 `logging.rs` |
| [`buddy_system_allocator`](https://crates.io/crates/buddy_system_allocator) | Buddy system 堆分配器 + 物理帧分配器 |
| [`heapless`](https://crates.io/crates/heapless) | 固定容量 `Vec`/`String`/`Queue`（无需堆分配，中断安全） |
| [`hashbrown`](https://crates.io/crates/hashbrown) | no_std `HashMap`，O(1) 查找 |
| [`intrusive-collections`](https://crates.io/crates/intrusive-collections) | 侵入式链表/红黑树（调度器队列、等待队列，零额外分配） |
| [`elf`](https://crates.io/crates/elf) | 零拷贝 ELF 解析（no_std） |
| [`rustc-demangle`](https://crates.io/crates/rustc-demangle) | Rust 符号 demangling（栈回溯） |
| [`unwinding`](https://crates.io/crates/unwinding) | DWARF 栈回溯 |
| [`fdt`](https://crates.io/crates/fdt) | 纯 Rust 设备树（FDT）解析器 |
| [`qemu-exit`](https://crates.io/crates/qemu-exit) | 用指定退出码结束 QEMU（系统测试） |
| [`virtio-drivers`](https://crates.io/crates/virtio-drivers) | VirtIO 协议栈（blk/net/console/gpu） |
| [`sbi-rt`](https://crates.io/crates/sbi-rt) | RISC-V SBI 运行时接口 |
| [`riscv`](https://crates.io/crates/riscv) | RISC-V CSR 访问、S-mode 支持 |
| [`aarch64-cpu`](https://crates.io/crates/aarch64-cpu) | AArch64 系统寄存器访问 |
| [`tock-registers`](https://crates.io/crates/tock-registers) | 类型安全的 MMIO 寄存器抽象 |
| [`arm-gic`](https://crates.io/crates/arm-gic) | GICv3 中断控制器驱动 |
| [`arm-psci`](https://crates.io/crates/arm-psci) | PSCI 电源管理接口 |

### 固件 / 引导（Git Submodule）

`3rd/` 只保存需要随仓库固定版本的第三方固件源码和工具源码。Rust crate 依赖由 `Cargo.toml` / `Cargo.lock` 管理，不复制到 `3rd/`。更新 submodule 时，应在 PR 中说明来源、版本、许可证影响和验证命令。

| 依赖 | 用途 |
|------|------|
| [riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git) | RISC-V SBI 实现 |
| [u-boot/u-boot](https://github.com/u-boot/u-boot.git) | 通用引导程序 |
| [OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git) | OP-TEE 操作系统 |
| [ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git) | ARM 可信固件 |
| [dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git) | 设备树编译器 |

## 开发指南

### 代码风格

- **语言**: Rust nightly，`#![no_std]`，edition 2024
- **格式化**: `rustfmt.toml`（100 字符宽度），`cargo fmt` 强制执行
- **静态检查**: `cargo clippy -- -D warnings`
- **注释语言**: 所有注释和文档注释使用中文；`// SAFETY:` 前缀保留英文
- **完整约定**: Copyright、注释、文件规模、严格 JSON、第三方代码和运行时配置规则见 [docs/conventions.md](./docs/conventions.md)

### 命名约定

| 类型 | 风格 | 示例 |
|------|------|------|
| 函数/方法 | snake_case | `init_timer()` |
| 类型/Trait/Enum | PascalCase | `TaskManager`、`Scheduler` |
| 常量 | SCREAMING_SNAKE_CASE | `MAX_CORE_COUNT` |
| 模块 | snake_case | `paging` |

### Git Commit 规范

```
<type>(<scope>): <subject>

type: feat|fix|refactor|test|docs|chore|build|ci|perf|style|revert
scope: 可选，影响的模块 (arch, memory, task, xtask)
```

每条 commit 必须使用 `git commit --signoff`（DCO 签署）。
PR CI 会检查每个 commit 是否包含 `Signed-off-by` trailer。
可选提交模板：

```bash
git config commit.template .gitmessage
```

## 文档入口

- **文档索引**: [docs/README.md](./docs/README.md)
- **工程约定**: [docs/conventions.md](./docs/conventions.md)
- **Git 与 Commit**: [docs/git.md](./docs/git.md)
- **设计总览**: [docs/design/00-概述.md](./docs/design/00-概述.md)
- **SAS 架构**: [docs/design/SAS-架构设计.md](./docs/design/SAS-架构设计.md)
- **架构决策记录（ADR）**: [docs/adr/README.md](./docs/adr/README.md)
- **审计计划与进度**: [docs/audit/review-roadmap.md](./docs/audit/review-roadmap.md)、[docs/audit/audit-progress.md](./docs/audit/audit-progress.md)
- **Dev Container**: [docs/docker.md](./docs/docker.md)
- **可复制模板**: [docs/templates/README.md](./docs/templates/README.md)

文档类型边界：SAD/SDD 描述当前架构和当前设计；ADR/RFC 记录决策历史和方案讨论；Spec 记录设计输入；Plan 记录执行步骤。图表优先使用 Mermaid 或 PlantUML。

## 贡献指南

我们欢迎所有形式的贡献！

| 方式 | 说明 |
|------|------|
| **报告问题** | 通过 [GitHub Issues](https://github.com/Simple-XX/SimpleKernel/issues) 报告 Bug |
| **改进接口** | 提出更好的 trait 抽象和文档改进建议 |
| **补充测试** | 在 `tests/` 的对应测试包中添加 `[[bin]]` 测试用例 |
| **完善文档** | 改进文档注释、添加使用示例 |
| **提交实现** | 提交 trait 的实现或替代实现 |

### 代码贡献流程

1. Fork 本仓库
2. 创建功能分支: `git checkout -b feat/amazing-feature`
3. 遵循 `AGENTS.md`、`docs/conventions.md` 和 `docs/git.md` 进行开发
4. 确保相关测试通过，例如 `devcontainer exec --workspace-folder . cargo test` 和 `devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --all --timeout 30`
5. 提交变更: `git commit --signoff -m 'feat(scope): add amazing feature'`
6. 创建 Pull Request

## 许可证

本项目采用多重许可证：

- **代码许可** — [MIT License](./LICENSE)
- **反 996 许可** — [Anti 996 License](https://github.com/996icu/996.ICU/blob/master/LICENSE)

---

<div align="center">

**如果这个项目对您有帮助，请给我们一个 Star！**

[Star 项目](https://github.com/Simple-XX/SimpleKernel) | [报告问题](https://github.com/Simple-XX/SimpleKernel/issues) | [参与讨论](https://github.com/Simple-XX/SimpleKernel/discussions)

</div>
