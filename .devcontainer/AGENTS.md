<!-- Copyright The SimpleKernel Contributors -->

# 可选 Dev Container 环境

本目录提供预装 Rust、交叉编译器、QEMU 和固件工具的环境，也用于当前 CI 镜像。
开发者可以选择本地环境；项目不要求 Docker、Dev Container 或固定容器名。
本地依赖与工作流见 [贡献指南](../CONTRIBUTING.md#环境与命令)，容器搭建见
[docs/docker.md](../docs/docker.md)。

## 配置维护

- Rust 版本以根 `rust-toolchain.toml` 为准，Dockerfile 的预装版本随之更新；
  CI 在使用缓存镜像时仍按仓库文件补齐所需工具链。
- `devcontainer.json` 默认容器名为 `simplekernel-devcontainer`，挂载当前仓库到 `/workspace`。
  自定义容器名、多个 checkout 或其他容器运行时均可，命令中的名称与工作目录相应调整。
  复用前核对 bind mount；不删除或覆盖不属于当前任务的容器。
- `postCreateCommand` 获取依赖，不自动安装 Git hook；hook 在实际提交的环境按贡献指南启用。
- Dockerfile 应为 `dev` 用户准备可写的 `/srv/tftp`，该目录由当前 xtask 的 QEMU 入口复用。
  同一环境不要并行运行多个 run/debug/test 作业。
- 更改环境时核验相应工具版本和相关命令；构建镜像成功不等于内核系统测试通过。

## 手动 Docker 使用

不使用 Dev Container CLI 时，也可以直接构建和启动 Docker 容器：

```bash
docker build --pull -f .devcontainer/Dockerfile -t simplekernel-devcontainer:latest .devcontainer
# 创建前确认同名容器不存在；若已存在，先核对挂载再复用。
docker run -d --name simplekernel-devcontainer \
  --mount "type=bind,src=$(pwd),dst=/workspace" -w /workspace \
  simplekernel-devcontainer:latest sleep infinity
docker exec -w /workspace simplekernel-devcontainer git config --global --add safe.directory /workspace
docker exec -w /workspace simplekernel-devcontainer git submodule update --init --recursive --depth 1
docker exec -w /workspace simplekernel-devcontainer cargo fetch --locked
docker exec -w /workspace simplekernel-devcontainer cargo xtask --help
```

该方式不执行 `postCreateCommand`。重建前保留需要的容器状态；待 review 的文件和验证证据
写回挂载目录，临时缓存可以留在容器内。

## 产物路径

| 产物 | 宿主机路径 | 容器内路径 | 说明 |
|------|------------|------------|------|
| Cargo 产物 | `target/` | `/workspace/target/` | 内核 ELF、调试文件、启动目录和固件 |
| 固件产物 | `target/firmware/<arch>/` | `/workspace/target/firmware/<arch>/` | OpenSBI、U-Boot、OP-TEE、ATF 输出 |
| 启动产物 | `target/<target-triple>/<profile>/boot/` | `/workspace/target/<target-triple>/<profile>/boot/` | `boot.fit`、`boot.scr.uimg`、`rootfs.img` |
| 文档发布产物 | `docs-out/` | `/workspace/docs-out/` | `docs.yml` 生成并上传的 Pages artifact |
| 临时文件 | `.tmp/` | `/workspace/.tmp/` | 可清理暂存 |

需要保留的唯一交付物不要只留在容器临时文件系统或匿名 volume。
