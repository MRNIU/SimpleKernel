<!-- Copyright The SimpleKernel Contributors -->

# xtask — 内核构建工具

替代 CMake 的宿主机构建脚本，通过 `cargo xtask <subcommand>` 调用。

## 子命令

```bash
docker exec -w /workspace simplekernel-devcontainer cargo xtask build    [--arch riscv64|aarch64] [--release]
docker exec -w /workspace simplekernel-devcontainer cargo xtask check    [--arch riscv64|aarch64]
docker exec -w /workspace simplekernel-devcontainer cargo xtask run      [--arch riscv64|aarch64] [--release] [--timeout 30]
docker exec -w /workspace simplekernel-devcontainer cargo xtask debug    [--arch riscv64|aarch64] [--release]
docker exec -w /workspace simplekernel-devcontainer cargo xtask firmware [--arch riscv64|aarch64]
docker exec -w /workspace simplekernel-devcontainer cargo xtask test     [选项]
```

## 测试命令

```bash
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --all --timeout 30
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --name <name> --timeout 30
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --timeout 120
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --list
```

`--all` 模式下，xtask 顺序执行所有 `tests/` 下的测试二进制，每个启动独立 QEMU 实例。输出被捕获，超时后自动终止。执行完毕后打印汇总报告。

`--name` 模式下，指定测试以交互模式运行（串口输出直接显示到终端），适合调试。

QEMU 系统测试默认按 30 秒超时执行。低性能宿主机或特殊测试可显式放宽，例如 `--timeout 120`，但应在任务说明或验证记录中写清楚原因。

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
