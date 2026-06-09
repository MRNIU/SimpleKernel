<!-- Copyright The SimpleKernel Contributors -->

# xtask — 内核构建工具

替代 CMake 的宿主机构建脚本，通过 `cargo xtask <subcommand>` 调用。

## 子命令

```bash
cargo xtask build    [--arch riscv64|aarch64] [--release]   # 编译内核
cargo xtask check    [--arch riscv64|aarch64]               # 检查编译
cargo xtask run      [--arch riscv64|aarch64] [--release] [--timeout 30]  # 编译并在 QEMU 中运行
cargo xtask debug    [--arch riscv64|aarch64] [--release]   # QEMU 调试模式（GDB localhost:1234）
cargo xtask firmware [--arch riscv64|aarch64]               # 编译第三方固件
cargo xtask test     [选项]                                  # 运行 QEMU 系统测试
```

## 测试命令

```bash
cargo xtask test --arch riscv64 --all              # 全部独立测试
cargo xtask test --arch riscv64 --name <name>      # 指定测试（交互式，串口直接输出）
cargo xtask test --arch riscv64 --timeout 120      # 自定义超时（默认 300 秒）
cargo xtask test --list                            # 列出可用测试
```

`--all` 模式下，xtask 顺序执行所有 `tests/` 下的测试二进制，每个启动独立 QEMU 实例。输出被捕获，超时后自动终止。执行完毕后打印汇总报告。

`--name` 模式下，指定测试以交互模式运行（串口输出直接显示到终端），适合调试。

## 固件与运行

`run`、`debug` 和 `test` 会在启动前检查目标架构所需固件。固件缺失时，xtask 会自动执行对应的 `firmware` 构建流程，不需要手动先跑 `cargo xtask firmware`。

`run` 默认给 QEMU 设置 30 秒超时，避免启动流程卡死后阻塞终端。需要更长时间时可显式传入：

```bash
cargo xtask run --arch riscv64 --timeout 120
```

## 源码结构

| 文件 | 职责 |
|------|------|
| `src/main.rs` | CLI 入口（clap 参数解析 + 子命令分发） |
| `src/arch.rs` | 架构枚举（riscv64/aarch64）+ QEMU 路径/固件路径 |
| `src/build.rs` | 内核交叉编译 + 调试文件生成 + boot 目录准备 |
| `src/firmware.rs` | 第三方固件编译（OpenSBI、U-Boot、ATF、OP-TEE） |
| `src/qemu.rs` | QEMU 启动（DTB 导出、FIT 镜像、boot script、TFTP、交互/捕获两种模式） |
| `src/test.rs` | 测试编排（发现测试包、构建、执行、结果汇总） |
| `boot.its.template` | U-Boot FIT 镜像描述文件模板 |
| `*_boot_scr.txt` | 架构相关的 U-Boot 启动脚本 |
