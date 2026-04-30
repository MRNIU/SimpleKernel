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
| **Workspace 架构** | 内核拆分为 `lib + bin`，子系统独立 crate（memory、sync、page_table 等） |
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
# 单元测试（宿主机）
cargo test

# 系统测试（QEMU）
cargo xtask test --arch riscv64
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

# 或使用 CLI
npm install -g @devcontainers/cli
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

> 也支持 **GitHub Codespaces**：点击仓库页面的 Code → Codespaces → Create codespace on main
>
> 详细说明见 [Dev Container 文档](./docs/docker.md)

> 开发环境默认优先使用 Dev Container。除安装 Docker、Dev Container CLI/扩展、
> Git 等入口工具外，不需要在宿主机安装 Rust nightly、交叉编译器、QEMU 或固件构建依赖。

**方式二：本地环境**

参考 [工具链文档](./docs/0_工具链.md) 配置本地开发环境。

### 编译与运行

```bash
cd SimpleKernel

# 编译内核
cargo xtask build --arch riscv64

# 在 QEMU 模拟器中运行
cargo xtask run --arch riscv64

# 调试（GDB 连接 localhost:1234）
cargo xtask debug --arch riscv64

# 单元测试（宿主机 x86_64）
cargo test

# 系统测试（QEMU 中运行）
cargo xtask test --arch riscv64           # 统一测试内核
cargo xtask test --arch riscv64 --all     # 全部测试（统一 + 独立）
cargo xtask test --arch riscv64 --name panic-test  # 指定独立测试
cargo xtask test --list                   # 列出可用测试
```

**支持的架构：**
- `riscv64` — RISC-V 64 位
- `aarch64` — ARM 64 位

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
│   ├── page_table/                 #   多级页表
│   ├── frame_allocator/            #   物理帧分配器
│   ├── per_cpu/                    #   Per-CPU 数据
│   └── ...
├── tests/                          # QEMU 系统测试（每个子目录是独立二进制）
│   ├── test_harness/               #   公共 harness（test_main! 宏）
│   ├── heap-test/                  #   堆分配测试
│   ├── sync-spinlock-test/         #   SpinLock 测试
│   └── ...                         #   共 17 个测试二进制
├── xtask/                          # 构建工具（cargo xtask）
│   └── src/
│       ├── main.rs                 #   子命令分发（build/run/debug/test/firmware）
│       ├── build.rs                #   内核和测试编译
│       ├── qemu.rs                 #   QEMU 启动和 FIT 镜像生成
│       └── test.rs                 #   系统测试编排
├── docs/                           # 文档
│   └── rust-rewrite/               #   Rust 迁移设计文档（P0-P7）
├── 3rd/                            # 第三方固件（Git Submodule）
├── src_cpp/                        # C++ 遗留代码（只读参考）
└── .github/workflows/              # CI/CD（GitHub Actions）
```

## 测试体系

SimpleKernel 采用两层测试 + 冒烟测试：

### 单元测试（宿主机）

纯逻辑 crate 的 `#[test]` 模块，在宿主机上运行：

```bash
cargo test -p memory_types -p config -p page_table_entry -p arch
```

覆盖范围：地址运算、PTE 编解码、常量验证等。

### 系统测试（QEMU）

每个测试是独立的 `#![no_std]` 裸机二进制，启动独立 QEMU 实例，拥有干净的内核环境。

```bash
cargo xtask test --arch riscv64 --all          # 全部测试
cargo xtask test --arch riscv64 --name <name>  # 指定测试
cargo xtask test --list                        # 列出可用测试
```

测试位于 `tests/` 目录，使用 `tests/test_harness/` 提供的 `test_main!` 宏消除样板代码。

### 添加新测试

1. 创建 `tests/my-test/`，包含 `Cargo.toml` 和 `src/main.rs`
2. `src/main.rs` 使用 `test_harness::test_main!` 宏
3. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径

详见 `tests/README.md`。

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

### 命名约定

| 类型 | 风格 | 示例 |
|------|------|------|
| 函数/方法 | snake_case | `init_timer()` |
| 类型/Trait/Enum | PascalCase | `TaskManager`、`Scheduler` |
| 常量 | SCREAMING_SNAKE_CASE | `MAX_CORE_COUNT` |
| 模块 | snake_case | `page_table` |

### Git Commit 规范

```
<type>(<scope>): <subject>

type: feat|fix|docs|style|refactor|perf|test|build|revert
scope: 可选，影响的模块 (arch, memory, task, xtask)
```

每条 commit 必须使用 `git commit --signoff`（DCO 签署）。

### 文档

- **设计总览**: [docs/design/00-概述.md](./docs/design/00-概述.md)
- **阶段计划**: [docs/design/](./docs/design/)
- **工具链**: [docs/0_工具链.md](./docs/0_工具链.md)
- **系统启动**: [docs/1_系统启动.md](./docs/1_系统启动.md)
- **调试输出**: [docs/2_调试输出.md](./docs/2_调试输出.md)
- **中断**: [docs/3_中断.md](./docs/3_中断.md)
- **Dev Container**: [docs/docker.md](./docs/docker.md)

## 贡献指南

我们欢迎所有形式的贡献！

| 方式 | 说明 |
|------|------|
| **报告问题** | 通过 [GitHub Issues](https://github.com/Simple-XX/SimpleKernel/issues) 报告 Bug |
| **改进接口** | 提出更好的 trait 抽象和文档改进建议 |
| **补充测试** | 在 `tests/system/` 中添加新的测试用例 |
| **完善文档** | 改进文档注释、添加使用示例 |
| **提交实现** | 提交 trait 的实现或替代实现 |

### 代码贡献流程

1. Fork 本仓库
2. 创建功能分支: `git checkout -b feat/amazing-feature`
3. 遵循代码规范进行开发
4. 确保所有测试通过: `cargo test && cargo xtask test --arch riscv64 --all`
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
