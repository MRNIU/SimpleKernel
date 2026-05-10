<!-- Copyright The SimpleKernel Contributors -->

# Dev Container 开发环境

本项目使用 [Dev Container](https://containers.dev/) 提供一致的开发环境。镜像基于 Ubuntu 26.04 LTS，包含交叉编译工具链、QEMU、固件构建依赖、Rust nightly 工具链、`pre-commit` 和 `cargo xtask` 所需工具。

## 宿主机与容器边界

默认优先使用 Dev Container 运行构建、检查、`pre-commit`、固件构建和 QEMU 测试。宿主机只保留 Docker 或兼容容器运行时、Git、编辑器/AI agent 和已有 Dev Container 入口工具；不要为了本项目在宿主机安装 Rust nightly、交叉编译器、QEMU、固件构建依赖或其他项目开发依赖。

在宿主机上执行项目命令时，使用 `devcontainer exec --workspace-folder . <command>` 进入容器环境；只有正在修复容器自身配置、文档/Git 等入口操作，或任务明确要求无需项目工具链的本地操作时，才在宿主机执行，并说明原因和验证边界。

## 快速开始

### VS Code

1. 安装 [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) 扩展
2. 打开项目目录
3. 点击左下角 `><` 图标，选择 **Reopen in Container**
4. 等待容器构建完成（首次约 5-10 分钟）

### GitHub Codespaces

点击仓库页面的 **Code → Codespaces → Create codespace on main**，环境自动就绪。

### CLI

```shell
# 使用已存在的 Dev Container CLI 构建并启动
devcontainer up --workspace-folder .

# 在容器内执行命令
devcontainer exec --workspace-folder . cargo xtask build --arch riscv64
```

## 验证环境

```shell
devcontainer exec --workspace-folder . gcc --version
devcontainer exec --workspace-folder . aarch64-linux-gnu-gcc --version
devcontainer exec --workspace-folder . riscv64-linux-gnu-gcc --version
devcontainer exec --workspace-folder . rustup show
devcontainer exec --workspace-folder . cargo --version
devcontainer exec --workspace-folder . pre-commit --version
devcontainer exec --workspace-folder . shellcheck --version
devcontainer exec --workspace-folder . qemu-system-riscv64 --version
devcontainer exec --workspace-folder . mkimage -V
```

## 构建与运行

```shell
# 构建内核
devcontainer exec --workspace-folder . cargo xtask build --arch riscv64
devcontainer exec --workspace-folder . cargo xtask build --arch aarch64

# 构建固件
devcontainer exec --workspace-folder . cargo xtask firmware --arch riscv64
devcontainer exec --workspace-folder . cargo xtask firmware --arch aarch64
# run/debug/test 会在固件缺失时自动构建，这里通常只需显式预热固件时使用

# 运行
devcontainer exec --workspace-folder . cargo xtask run --arch riscv64 --timeout 30
devcontainer exec --workspace-folder . cargo xtask run --arch aarch64 --timeout 30

# 调试
devcontainer exec --workspace-folder . cargo xtask debug --arch riscv64

# QEMU 系统测试
devcontainer exec --workspace-folder . cargo xtask test --arch riscv64 --timeout 30
```
