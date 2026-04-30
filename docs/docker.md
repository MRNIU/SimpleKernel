
# Dev Container 开发环境

本项目使用 [Dev Container](https://containers.dev/) 提供一致的开发环境。镜像基于 Ubuntu 26.04 LTS，包含交叉编译工具链、QEMU、固件构建依赖、Rust nightly 工具链、`pre-commit` 和 `cargo xtask` 所需工具。

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
# 安装 devcontainer CLI
npm install -g @devcontainers/cli

# 构建并启动
devcontainer up --workspace-folder .

# 在容器内执行命令
devcontainer exec --workspace-folder . cargo xtask build --arch riscv64
```

## 验证环境

```shell
gcc --version
aarch64-linux-gnu-gcc --version    # aarch64 交叉编译器
riscv64-linux-gnu-gcc --version    # riscv64 交叉编译器
rustup show
cargo --version
pre-commit --version
shellcheck --version
qemu-system-riscv64 --version
mkimage -V
```

## 构建与运行

```shell
# 构建内核
cargo xtask build --arch riscv64
cargo xtask build --arch aarch64

# 构建固件
cargo xtask firmware --arch riscv64
cargo xtask firmware --arch aarch64
# run/debug/test 会在固件缺失时自动构建，这里通常只需显式预热固件时使用

# 运行
cargo xtask run --arch riscv64 --timeout 30
cargo xtask run --arch aarch64 --timeout 30

# 调试
cargo xtask debug --arch riscv64    # GDB 连接 localhost:1234

# QEMU 系统测试
cargo xtask test --arch riscv64
```
