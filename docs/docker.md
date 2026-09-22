<!-- Copyright The SimpleKernel Contributors -->

# 可选 Dev Container / Docker 环境

开发者可使用本地环境或容器，见 [贡献指南](../CONTRIBUTING.md#环境与命令)。
本目录的镜像只提供 Git、C/C++ 基础编译工具、rustup 和项目 Rust 工具链，不预装
QEMU、固件交叉工具链、GDB、pre-commit、cargo-deny、Node 或文档渲染工具。
Rust 版本与组件由 [rust-toolchain.toml](../rust-toolchain.toml) 指定；
[Dockerfile](../.devcontainer/Dockerfile) 的预装版本随之同步。

## 进入环境

- VS Code：安装 Dev Containers 扩展，打开仓库并选择 **Reopen in Container**。
- Codespaces：在仓库页面选择 **Code → Codespaces**，确认所需分支。
- 已有 Dev Container CLI：在仓库根执行：

```bash
devcontainer up --workspace-folder .
docker exec -w /workspace simplekernel-devcontainer cargo xtask --help
```

默认容器名为 `simplekernel-devcontainer`，当前 checkout 挂载到 `/workspace`。
创建后只准备产物目录并确认 Rust 工具链，不初始化固件 submodule、不下载全部 Cargo 依赖、
不安装 Git hook。容器终端中直接运行项目命令，无需嵌套 Docker。
可以自定义容器名和挂载路径；复用前确认挂载，不覆盖其他任务的容器。

不使用 Dev Container CLI 时：

```bash
docker build --pull -f .devcontainer/Dockerfile -t simplekernel-devcontainer:latest .devcontainer
# 创建前确认同名容器不存在；若已存在，先核对挂载再复用。
docker run -d --name simplekernel-devcontainer \
  --mount "type=bind,src=$(pwd),dst=/workspace" -w /workspace \
  simplekernel-devcontainer:latest sleep infinity
docker exec -w /workspace simplekernel-devcontainer git config --global --add safe.directory /workspace
docker exec -w /workspace simplekernel-devcontainer cargo xtask --help
```

手动 Docker 默认以 root 运行；Dev Container 使用普通用户 `dev`。
重建前保留需要的容器状态和验证记录。

## 按需补齐工具

- 纯逻辑 host 测试与内核检查/构建可直接使用基础镜像。
- 运行或测试 QEMU：按 [CI setup 的架构包清单](../.github/actions/setup/action.yml)
  安装所需架构的模拟器、交叉编译器和固件工具（普通用户使用 `sudo apt-get`），
  再执行 `git submodule update --init --recursive --depth 1`。首次运行由 xtask 构建固件。
  运行用户须可写 `/srv/tftp`：例如在所选开发环境执行
  `sudo mkdir -p /srv/tftp && sudo chown "$(id -u):$(id -g)" /srv/tftp`。
- GDB 调试另装 `gdb-multiarch`；依赖审计另装 cargo-deny，版本及命令见
  [CI workflow](../.github/workflows/workflow.yml) 的 Dependency audit 步骤。
- pre-commit 只在执行 Git 提交的环境按 [贡献指南](../CONTRIBUTING.md#可选容器与提交检查)
  安装；文档渲染工具只在需要相应格式时安装。

CI 共用基础镜像：检查 job 单独安装 cargo-deny，系统测试 job 按矩阵架构安装固件/QEMU
工具，rustdoc job 无需固件工具。不要把这些任务依赖重新塞回基础镜像。
同一环境不要并行运行多个共用 `/srv/tftp` 的 run/debug/test 作业。

## 核验与维护

```bash
docker inspect --format '{{range .Mounts}}{{println .Source "->" .Destination}}{{end}}' simplekernel-devcontainer
docker exec -w /workspace simplekernel-devcontainer rustup show
docker exec -w /workspace simplekernel-devcontainer cargo xtask --help
```

只检查本次涉及的工具；镜像构建成功不等于内核系统测试通过。
命令参数、QEMU 超时与清理见 [xtask 手册](../xtask/README.md)。

## 产物路径

| 产物 | 宿主机路径 | 容器内路径 | 说明 |
|------|------------|------------|------|
| Cargo 产物 | `target/` | `/workspace/target/` | 内核 ELF、调试文件、启动目录和固件 |
| 固件产物 | `target/firmware/<arch>/` | `/workspace/target/firmware/<arch>/` | OpenSBI、U-Boot、OP-TEE、ATF 输出 |
| 启动产物 | `target/<target-triple>/<profile>/boot/` | `/workspace/target/<target-triple>/<profile>/boot/` | `boot.fit`、`boot.scr.uimg`、`rootfs.img` |
| 文档发布产物 | `docs-out/` | `/workspace/docs-out/` | `docs.yml` 生成并上传的 Pages artifact |
| 临时文件 | `.tmp/` | `/workspace/.tmp/` | 可清理暂存 |

需要保留的唯一交付物不要只留在容器临时文件系统或匿名 volume。
