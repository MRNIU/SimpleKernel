<!-- Copyright The SimpleKernel Contributors -->

# xtask — 内核构建工具

SimpleKernel 的仓库内构建、运行、调试、固件和系统测试入口，在所选开发环境的仓库根
通过 `cargo xtask <subcommand>` 调用。

## 职责

`xtask` 负责把内核构建、QEMU 运行、调试文件、固件构建和系统测试编排成稳定 CLI 入口。

## 边界

- 本地或容器均可使用；依赖准备与当前宿主平台限制见 [贡献指南](../CONTRIBUTING.md#环境与命令)。
- 本工具不修改内核运行时语义，只负责构建产物、启动参数、固件和测试执行编排。

## 子命令

以下为参数语法，方括号不是字面命令；实际参数以 `cargo xtask <subcommand> --help` 为准。
命令均从仓库根执行。`check` 不链接产物，`build` 生成 ELF 和调试文件。

```bash
cargo xtask build    [--arch riscv64|aarch64] [--release]
cargo xtask check    [--arch riscv64|aarch64]
cargo xtask run      [--arch riscv64|aarch64] [--release] [--timeout 30]
cargo xtask debug    [--arch riscv64|aarch64] [--release]
cargo xtask firmware [--arch riscv64|aarch64]
cargo xtask test     [选项]
```

## 测试命令

```bash
cargo xtask test --arch riscv64 --all --timeout 30
cargo xtask test --arch riscv64 --name <name> --timeout 30
cargo xtask test --arch riscv64 --timeout 120
cargo xtask test --list
```

不传 `--name` 时默认运行全部测试，`--all` 是显式写法。测试顺序执行，每个二进制独立启动
QEMU。单测和全量模式都捕获串口输出；失败时打印捕获内容，全量模式另打印汇总。
`--name` 必须匹配 `--list` 的显示名，不支持按包名前缀筛选；例如 `memory-test` 必须选择
`memory-test/fdt-multi-memory` 等具体二进制。测试通过条件见 [tests/README.md](../tests/README.md)。

QEMU 系统测试默认按 30 秒超时执行。低性能宿主机或特殊测试可显式放宽，例如 `--timeout 120`，但应在任务说明或验证记录中写清楚原因。

## 固件与运行

`run`、`debug` 和 `test` 会在启动前检查目标架构所需固件。固件缺失时，xtask 会自动执行对应的 `firmware` 构建流程，不需要手动先执行
`cargo xtask firmware`。

`run` 默认给 QEMU 设置 30 秒超时，避免启动流程卡死后阻塞终端。需要更长时间时可显式传入：

```bash
cargo xtask run --arch riscv64 --timeout 120
```

## 超时、失败与清理

- `run` 的 `--timeout` 限制 QEMU 运行；`test` 限制每个测试的 QEMU 执行。默认 30 秒，
  放宽时记录原因。编译、固件准备不包含在该时间内，agent 的外层命令等待也必须有界；
  冷构建所需时间应在外层预算中考虑，不能靠调大 QEMU timeout 解决。
- `debug` 为交互调试等待 GDB，CLI 没有 `--timeout`。自动化调用需在命令前加外层
  `timeout <seconds>`；人工会话结束时终止 QEMU 并确认进程退出。
- 失败先区分编译、固件、QEMU 启动、超时和 sentinel 失败。保留基线、架构、测试名、完整命令、
  退出状态与串口错误；检查 `target/<triple>/<profile>/boot/qemu.log`。日志会被后续运行覆盖，
  重跑前保留需要的证据；不要靠无限延长超时或重复运行掩盖失败。
- 测试 runner 超时会终止其子进程；外层取消或异常后仍须检查残留。先在执行 QEMU 的环境查询
  `pgrep -af qemu-system`，确认进程属于本次运行后按 PID 终止。仅在确认没有其他 QEMU
  作业时，使用 `pkill -f qemu-system`
  清理，再次检查进程。无匹配时查询/清理命令可能返回非零。
- 同一环境的 `/srv/tftp` 与 boot 目录会被复用，不并发启动多个 run/debug/test 作业。

## GDB 与编辑器

在所选环境启动 `cargo xtask debug --arch riscv64`，同一环境的另一个终端连接：

```bash
gdb-multiarch target/riscv64gc-unknown-none-elf/debug/simplekernel -x debug.gdb
```

AArch64 使用 `--arch aarch64` 和 `target/aarch64-unknown-none/debug/simplekernel`。
`localhost:1234` 属于运行 QEMU 的环境；使用容器时默认没有向宿主机发布端口。`debug.gdb` 提供初始连接与断点；
其中调用目标函数的命令只能在其注明的初始化条件满足后使用。

`.vscode/tasks.json` / `launch.json` 是 VS Code 的薄适配层，只调用现有 xtask。
`cppdbg` 需要所选环境中的 C/C++ 调试扩展；项目配置没有自动安装该扩展，可用上述 GDB CLI。
`launch.json` 的 GDB 路径默认按 Linux 环境配置，本地环境需按实际安装路径调整。
后台 debug task 没有就绪 matcher，F5 可能等待任务就绪；这不是内核启动成功或失败的证据。

## 验证入口

- 文档-only 变更：`git diff --check`。
- CLI 参数、构建或测试编排变更：`cargo clippy -p xtask -- -D warnings`。
- 测试发现逻辑变化：`cargo xtask test --list`。
- QEMU run/test 行为变化：按影响面运行对应 `cargo xtask run` 或 `cargo xtask test`，命令必须带 `--timeout 30` 或说明放宽原因。

## 不要假设

- 示例从仓库根执行；本地和容器共用 xtask，不另造脚本包装。
- 不要把自动固件构建写成手动前置步骤；`run`、`debug` 和 `test` 会按需检查并构建固件。
- 自动化 QEMU 调用保持有界；交互 debug 的特殊处理见上节。

## 源码结构

| 文件 | 职责 |
|------|------|
| `src/main.rs` | CLI 入口（clap 参数解析 + 子命令分发） |
| `src/arch.rs` | 架构枚举（riscv64/aarch64）+ QEMU 路径/固件路径 |
| `src/build.rs` | 内核交叉编译 + 调试文件生成 + boot 目录准备 |
| `src/firmware.rs` | 第三方固件编译（OpenSBI、U-Boot、ATF、OP-TEE） |
| `src/qemu.rs` | QEMU 启动（DTB 导出、FIT 镜像、boot script、TFTP、交互/捕获两种模式） |
| `src/test.rs` | 测试编排（发现测试包、构建、执行、结果汇总） |
| `src/boot.its.template` | U-Boot FIT 镜像描述文件模板 |
| `src/*_boot_scr.txt` | 架构相关的 U-Boot 启动脚本 |
