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

## 项目与能力边界

SimpleKernel 使用 Rust（`no_std`、`no_main`、nightly），以 trait 契约组织内核实现，
通过纯逻辑 host 测试和独立 QEMU 测试验证行为。支持 riscv64 和 aarch64 的 QEMU 平台。

项目采用单地址空间 SAS，所有代码运行在同一特权级和地址空间，syscall 是直接函数调用。
基于类型系统和 crate 可见性的隔离仍有未完成部分，不能将 APP 唯一公共网关视为已经实现。
设备层已有 descriptor probe、typed capability 和 VirtIO Block 集成；QEMU DMA 支持不等于
真机 non-coherent DMA 支持。实现与未决边界见 [当前设计入口](docs/AGENTS.md)。

## 快速开始

可选择本地开发或容器开发。先按 [贡献指南](CONTRIBUTING.md#环境与命令) 准备本地依赖，
或使用可选的 [Dev Container / Docker](docs/docker.md)。以下命令在所选环境的仓库根执行。

```bash
git clone https://github.com/simple-xx/SimpleKernel.git
cd SimpleKernel

# 编译并运行 RISC-V 内核
cargo xtask build --arch riscv64
cargo xtask run --arch riscv64 --timeout 30

# 查看测试名，再运行一个独立测试
cargo xtask test --list
cargo xtask test --arch riscv64 --name frame-test/alloc --timeout 30
```

切换 AArch64 时将 `--arch riscv64` 换成 `--arch aarch64`。`run`、`debug` 和执行测试会
按需构建缺失固件，首次运行可能较久。`--timeout` 约束 QEMU 执行，不包含前面的编译时间。

- VS Code、Codespaces、CLI 搭建与环境验证：[Dev Container 文档](docs/docker.md)。
- 构建、检查、调试、固件、超时与故障诊断：[xtask 手册](xtask/AGENTS.md)。
- 选择 host 或 QEMU 验证及 PR 流程：[贡献指南](CONTRIBUTING.md)。

## 项目结构

| 目录 | 职责 |
|------|------|
| `src/` | 内核入口、架构后端、任务、设备、文件系统和 syscall |
| `crates/` | 内存、同步、per-CPU、平台描述等子系统；[职责表](crates/AGENTS.md) |
| `tests/` | 独立裸机 QEMU 测试；[测试清单与编写方式](tests/AGENTS.md) |
| `xtask/` | 构建、检查、运行、调试、固件与系统测试编排 |
| `docs/` | 当前设计、ADR、历史记录与模板；[文档索引](docs/AGENTS.md) |
| `.devcontainer/`、`.github/` | 开发环境与 CI |
| `3rd/` | 固件 submodule；来源与固定版本由 `.gitmodules` 和 Git gitlink 记录 |
| `.agents/skills/simplekernel-dev/` | 唯一项目开发 skill |

Rust 依赖以 `Cargo.toml` / `Cargo.lock` 为准；固件包括 OpenSBI、U-Boot、OP-TEE、ATF。
设备树编译器由所选环境的包管理器安装，不是本仓库 submodule。

## 开发与贡献

从 [CONTRIBUTING.md](CONTRIBUTING.md) 开始；AI agent 同时遵循
[根 AGENTS](AGENTS.md) 和所修改目录的局部规则。项目 skill 的使用方式见
[贡献指南中的 AI 协作入口](CONTRIBUTING.md#ai-协作入口)。

- 当前架构与接口：[docs/AGENTS.md](docs/AGENTS.md)。
- 工程约定：[docs/conventions.md](docs/conventions.md)。
- 提交格式和 DCO：[.gitmessage](.gitmessage)。
- 当前状态、未决事项与下一步：[审计进度](docs/audit/audit-progress.md)。
- 社区行为规范：[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

历史设计和旧验证记录不能作为当前代码已经通过验证的证据。

## 许可证

本项目采用多重许可证：

- **代码许可** — [MIT License](LICENSE)
- **反 996 许可** — [Anti 996 License](https://github.com/996icu/996.ICU/blob/master/LICENSE)
