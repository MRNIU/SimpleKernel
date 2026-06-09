<!-- Copyright The SimpleKernel Contributors -->

# Dev Container

本目录定义 SimpleKernel 的开发、CI 和 QEMU 系统测试容器环境。宿主机只负责 Git、Docker
或兼容容器运行时、编辑器 / AI agent，以及 Dev Container CLI；Rust nightly、交叉编译器、
QEMU、固件构建依赖、pre-commit、Mermaid 和 GitHub CLI 都在容器内使用。

## 命名规则

本仓库使用自己的容器和镜像命名，不继承其他项目名称。

| 用途 | 名称 |
|------|------|
| Dev Container 显示名 | `simplekernel-devcontainer` |
| 本地开发镜像 | `simplekernel-devcontainer:latest` |
| 常驻 Dev Container 容器名 | `simplekernel-devcontainer-{username}-{branch}` |
| CI / GHCR Dev Container 镜像 | `ghcr.io/simple-xx/simplekernel-devcontainer:{latest,sha}` |
| 项目运行时容器 | 无；内核只在 Dev Container 内启动的 QEMU 中运行 |

`username` 和 `branch` 必须归一化为 Docker 容器名允许的字符。`branch` 来自当前具名
Git 分支；detached HEAD 状态不得启动常驻 Dev Container。不要依赖 Docker 自动生成的
随机名称作为常规入口。

## 启动入口

使用编辑器 Dev Container 或 CLI 前，先在宿主机 shell 中设置本仓库命名规则要求的
`DEVCONTAINER_NAME`：

```bash
DEVCONTAINER_USER="$(id -un | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
DEVCONTAINER_BRANCH="$(git branch --show-current | sed -E 's/[^[:alnum:]_.-]+/-/g; s/^-+//; s/-+$//')"
if [ -z "$DEVCONTAINER_BRANCH" ]; then
  echo "detached HEAD is not allowed for the devcontainer name" >&2
  exit 1
fi
export DEVCONTAINER_NAME="simplekernel-devcontainer-${DEVCONTAINER_USER}-${DEVCONTAINER_BRANCH}"
```

标准入口：

```bash
devcontainer up --workspace-folder .
docker exec -w /workspace "$DEVCONTAINER_NAME" cargo xtask build --arch riscv64
```

`devcontainer exec --workspace-folder . <command>` 也可用于临时交互，但文档化验证和 PR
证据优先写成 `docker exec -w /workspace "$DEVCONTAINER_NAME" <command>`，这样容器名、
工作目录和复用边界都明确。

## 手动 Docker fallback

没有 Dev Container CLI，或正在排查 Dev Container CLI 本身时，才手动创建常驻容器。
`docker run` 只用于创建后台容器；项目命令仍通过 `docker exec` 执行。

```bash
docker build --pull=false -f .devcontainer/Dockerfile -t simplekernel-devcontainer:latest .devcontainer
docker inspect "$DEVCONTAINER_NAME" >/dev/null 2>&1 || docker run -d \
  --name "$DEVCONTAINER_NAME" \
  --mount "type=bind,src=$(pwd),dst=/workspace" \
  -w /workspace \
  simplekernel-devcontainer:latest sleep infinity
if [ "$(docker inspect -f '{{.State.Running}}' "$DEVCONTAINER_NAME")" != "true" ]; then
  docker start "$DEVCONTAINER_NAME" >/dev/null
fi
docker exec -w /workspace "$DEVCONTAINER_NAME" git config --global --add safe.directory /workspace
```

重建同名容器前，先确认旧容器没有需要保留的状态。需要保留的内核、固件、文档或测试产物
必须写回当前仓库的 `target/`、`docs-out/` 或文档声明的产物目录。

## 产物路径

| 产物 | 宿主机路径 | 容器内路径 | 说明 |
|------|------------|------------|------|
| Cargo 产物 | `target/` | `/workspace/target/` | 内核 ELF、调试文件、启动目录和固件 |
| 固件产物 | `target/firmware/<arch>/` | `/workspace/target/firmware/<arch>/` | OpenSBI、U-Boot、OP-TEE、ATF 输出 |
| 启动产物 | `target/<target-triple>/<profile>/boot/` | `/workspace/target/<target-triple>/<profile>/boot/` | `boot.fit`、`boot.scr.uimg`、`rootfs.img` |
| 文档发布产物 | `docs-out/` | `/workspace/docs-out/` | `docs.yml` 生成并上传的 Pages artifact |
| 临时文件 | `.tmp/` | `/workspace/.tmp/` | 可清理暂存 |

不要把唯一产物留在容器临时文件系统、匿名 volume、`/tmp`、`/home/dev`、上级目录或项目外缓存目录。
