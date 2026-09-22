<!-- Copyright The SimpleKernel Contributors -->

# 贡献指南

## 开始任务

1. 确认目标、影响范围和 Git 基线：`git branch --show-current`、`git rev-parse HEAD`、
   `git status --short`、`git diff`、`git diff --cached`。保留无关改动；分支基于已确认的目标线，
   不因历史 prompt 自动切换到 main 或重置工作区。
2. 阅读 [根 AGENTS](AGENTS.md) 和目标目录的局部规则；工程细节见
   [conventions](docs/conventions.md)，按需从 [文档索引](docs/README.md) 找当前设计。
3. 先追踪 trait 契约、实现、调用方和测试。局部修复按既有契约实施；改变架构不变量、
   所有权、跨模块边界或引入长期替代方案时，按 [ADR 规则](docs/adr/README.md) 讨论决策。
   普通小改不要求新建 Plan、ADR 或扫描全仓。
4. 只读审查交付问题位置、可达触发、影响、证据和最小修复方向，完成后等待反馈。
   用户已授权实施的任务继续完成范围内修改；不把审查授权视为修改授权。

## 环境与命令

开发者可自行选择本地环境或容器环境；Docker、Dev Container 均为可选项。
下文命令在所选环境的仓库根执行，共用 [rust-toolchain.toml](rust-toolchain.toml)、
Cargo.lock 和 [xtask 手册](xtask/README.md)。

### 本地开发

- 安装 Git、rustup 和本机 C 链接器；`rustup show` 会按仓库配置选择 nightly、组件及两个裸机 target。
- 纯逻辑 host 测试、`xtask check` 和 `xtask build` 不需要 QEMU 或固件；
  构建的 LLVM 工具来自工具链 `llvm-tools` 组件。运行/测试创建 FAT 镜像时另需 `dd`、`mkfs.fat`。
- 运行/系统测试需要 `qemu-system-riscv64` / `qemu-system-aarch64`、`dtc`、`mkimage`；
  首次自动构建固件还需要 GNU make、目标交叉 GCC/binutils、Python 固件模块等。
  Linux 固件/QEMU 包清单按架构维护在 [CI setup](.github/actions/setup/action.yml)，
  在所选环境按任务安装；调试另装 `gdb-multiarch`。
- 涉及固件时执行 `git submodule update --init --recursive --depth 1`，保持 Git 固定版本。
  GDB 调试使用支持目标架构的 GDB。

当前 xtask 使用 Unix symlink、Linux 风格交叉编译器命名和固定 `/srv/tftp`。
本地 Linux 可直接准备这些依赖；macOS 需要自行适配工具路径/文件系统，Windows 可用 WSL。
这不是完整的原生跨宿主平台支持承诺。运行前需确保当前用户可写 `/srv/tftp`，且没有其他
SimpleKernel QEMU 作业共用它；纯逻辑测试不需要该目录。

### 为什么仍需 nightly

`no_std` 本身不要求 nightly，但当前实现使用尚未稳定的 `SyncUnsafeCell` 和
`#[alloc_error_handler]`；xtask 还通过 `-Z build-std` 重建 core/alloc/compiler_builtins。
因此仅修改工具链版本不能迁移到 stable。未来迁移需同时评估静态存储的并发契约、
分配失败处理、预编译标准库是否满足两个 target，并完成相应回归。

### 可选容器与提交检查

希望使用基础 Rust 容器时，见 [Dev Container / Docker 使用说明](docs/docker.md)。容器只提供环境，
不改变贡献规则。复用容器时确认挂载的是当前 checkout。

pre-commit 是可选本地辅助工具，配置见 [.pre-commit-config.yaml](.pre-commit-config.yaml)。
在实际执行 Git 提交的环境安装 pre-commit，再运行 `pre-commit install --install-hooks`；
不要在容器创建时自动写入宿主机可能无法运行的 hook。
可执行 `pre-commit run --files <files...>` 检查本次文件；文件卫生 hook 可能修复文件，
之后复查 diff。Rust fmt hook 只检查、不改写文件。两架构 Clippy 为手动 hook，例如
`pre-commit run cargo-clippy-riscv64 --hook-stage manual --all-files`，按改动选择架构。

## 按改动选择验证

先选能覆盖改动的最小集合；跨架构公共代码要检查两种 target，涉及实际平台行为再选对应
QEMU 测试。更大的集成变更或阶段关闭按其约定扩大回归；不要把 CI 全量任务变成每次文档修改的前置条件。

| 改动 | 必要验证与证据边界 |
|------|--------------------|
| Markdown、协作规则、skill | `git diff --check`；核对受影响路径、链接、命令和规则引用；skill 另查 frontmatter、发现位置与典型任务走查 |
| crate 纯逻辑 | 按局部 AGENTS 选择 `cargo test -p <package>` 和 package Clippy；host 只证明实际编译到的逻辑 |
| 裸机实现、共享内核接口 | 格式、受影响 target 的 Clippy / `xtask check`；需要链接产物时用 `xtask build` |
| 锁、中断、页表、启动、per-CPU、设备或平台行为 | 在编译检查之外，按 [测试清单](tests/README.md) 选择定点 QEMU；host stub 不证明硬件行为、SMP、时序或真机 DMA |
| xtask 参数或测试编排 | package Clippy / host 测试；参数 `--help`、测试发现 `test --list`；执行路径变化再运行对应 QEMU 场景 |
| 依赖与许可证 | `cargo deny check`，并按消费者影响补充验证 |
| Rust API 文档 | 对受影响 target 执行 `cargo doc --no-deps --target <triple>`；普通 Markdown 不要求 rustdoc |

以下是可复制的命令示例，按上表选择，不要求全部执行：

```bash
# Rust 修改的格式检查
cargo fmt --all -- --check

# 内核双 target 静态检查
cargo clippy --target riscv64gc-unknown-none-elf -- -D warnings
cargo clippy --target aarch64-unknown-none -- -D warnings

# 纯逻辑示例；更多测试按 crate-local AGENTS 选择
cargo test -p platform_fdt

# 参数与测试名核验，不启动 QEMU
cargo xtask --help
cargo xtask test --list
```

不要对根内核使用无 target 的 `cargo test` 或 Clippy 来代替上述入口；workspace 的默认成员
是裸机内核。CI 实际检查、重复次数与超时以 [workflow.yml](.github/workflows/workflow.yml)
为准。

记录完整命令、环境/target、测试名、结果、失败日志与未覆盖范围；未运行的检查说明原因，
不将历史通过、构建成功或 QEMU 正常退出写成系统测试通过。

## 文档同步

按实际受影响的描述修改，不机械同步所有入口：

| 变化 | 详细维护位置 |
|------|--------------|
| 工程约定、错误与代码风格 | [conventions](docs/conventions.md) |
| 环境选择与本地依赖 / 可选容器维护 | 本文件 / [docs/docker.md](docs/docker.md) |
| CLI、超时、测试编排 / 测试契约 | [xtask/README.md](xtask/README.md) / [tests/README.md](tests/README.md) |
| trait、错误、所有权、平台或模块边界 | 契约注释、当前设计和相关局部 AGENTS；架构决策另按 ADR 规则 |
| 提交格式、DCO、footer | [.gitmessage](.gitmessage) |
| 项目定位、快速开始、读者导航 | [README](README.md)，英文入口仅同步其实际受影响部分 |
| 当前工作、未决事项、下一步 | [audit-progress](docs/audit/audit-progress.md)；历史证据按其路由归档 |

文件移动或删除时检查引用；文档类型和模板按 [docs/README.md](docs/README.md) 路由。

## 提交与 PR

检查最终 diff 和无关改动；只暂存本任务文件。提交格式、正文、footer、DCO 以
[.gitmessage](.gitmessage) 为唯一详细来源，每条 commit 使用 `git commit --signoff`。
可选用 `git config commit.template .gitmessage` 启用模板。

AI agent 仅在用户授权后提交或 push。创建 PR 时按
[PR 模板](.github/PULL_REQUEST_TEMPLATE.md) 描述问题、改变后的行为、验证、未覆盖项和风险；
ADR 的接受状态由项目作者决定，不能把代码已实现视为已接受。

## AI 协作入口

仓库只维护一个开发 skill：[simplekernel-dev](.agents/skills/simplekernel-dev/SKILL.md)。
在此仓库内可用 `$simplekernel-dev` 显式调用，也可由匹配任务自动选择；覆盖实现、修复、
重构、审查、文档维护和验证。`.agents/skills/` 是 Codex 的
[项目级发现位置](https://learn.chatgpt.com/docs/build-skills)，不需安装到个人全局目录。
若当前会话尚未刷新技能列表，重新打开仓库会话。

其他工具只有在确认支持相应发现机制时才使用自动入口；否则显式提供根 AGENTS、相关局部规则
和这个 SKILL.md。不要假设 Markdown 链接会自动加载，也不复制多份工具专用规则。
