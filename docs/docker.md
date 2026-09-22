<!-- Copyright The SimpleKernel Contributors -->

# 可选 Dev Container / Docker 环境

本项目可选用 [Dev Container](https://containers.dev/) 提供 Rust nightly、交叉编译器、QEMU、
固件工具和 pre-commit。版本与依赖以 [.devcontainer/Dockerfile](../.devcontainer/Dockerfile)
和 [rust-toolchain.toml](../rust-toolchain.toml) 为准。
也可按 [贡献指南](../CONTRIBUTING.md#本地开发) 使用本地工具链，无需 Docker。
容器默认命名、挂载检查、手动 Docker 用法和产物路径统一在
[.devcontainer/AGENTS.md](../.devcontainer/AGENTS.md) 维护。

## 进入环境

- VS Code：安装 Dev Containers 扩展，打开仓库并选择 **Reopen in Container**。
- Codespaces：在仓库页面选择 **Code → Codespaces**，确认所需分支。
- 已有 Dev Container CLI：在仓库根执行下列命令。

```bash
devcontainer up --workspace-folder .
docker exec -w /workspace simplekernel-devcontainer cargo xtask --help
```

首次创建会安装镜像依赖、初始化固件 submodule、获取 Cargo 依赖（不自动安装 Git hook）。
若这些步骤失败，保留具体阶段与错误，按局部容器规则排查。
也可直接使用局部规则中的手动 Docker 命令。

以上 `docker exec` 是宿主机入口；已在 VS Code Dev Container 或 Codespaces 的 `/workspace`
终端中时，直接执行 `cargo xtask --help` 等项目命令，不再嵌套 Docker。

## 环境只读核验

```bash
docker inspect --format '{{range .Mounts}}{{println .Source "->" .Destination}}{{end}}' simplekernel-devcontainer
docker exec -w /workspace simplekernel-devcontainer rustup show
docker exec -w /workspace simplekernel-devcontainer cargo --version
docker exec -w /workspace simplekernel-devcontainer qemu-system-riscv64 --version
docker exec -w /workspace simplekernel-devcontainer qemu-system-aarch64 --version
docker exec -w /workspace simplekernel-devcontainer mkimage -V
docker exec -w /workspace simplekernel-devcontainer pre-commit --version
```

只在对应工具有问题时扩展诊断；无需每次小改检查所有依赖。
构建、运行、调试和超时清理见 [xtask 手册](../xtask/AGENTS.md)，
验证与贡献流程见 [CONTRIBUTING](../CONTRIBUTING.md)。
