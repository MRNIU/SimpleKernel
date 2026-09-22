<!-- Copyright The SimpleKernel Contributors -->

# 依赖核验

> 核验日期：2026-09-22；基线 `f5e1f5f` 加本轮未提交工作区。
> 本文记录依赖选择与升级限制；精确版本以 `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`
> 及工具配置为准。旧 2026-04-03 统计已被本次实测替代，不作为当前验证证据。

## 更新范围

- Rust 更新为 `nightly-2026-09-22`；Cargo registry 依赖按 crates.io 当前非撤回版本更新。
  `fdt` 已使用预发布接口，本轮从 alpha1 更新到 alpha2；`xshell` 使用最新正式版而非新预发布版。
- 移除源码与所有成员 manifest 均无消费者的 `bitfield-struct`、`gdbstub`、`hashbrown`、
  `intrusive-collections`、`qemu-exit`、`smoltcp` 六个预留依赖。不表示网络或内嵌 GDB 已实现。
- `arm-gic 0.9.1` 自动识别 redistributor frame 布局；现有构造点适配 `Result`，失败保留
  GIC 地址、CPU 数量和原始错误并 panic，不静默回退。
- GitHub Actions、pre-commit 与开发镜像工具版本同步升级。clap 显式启用 help/usage/error-context，
  修复原配置禁用默认功能导致 `cargo xtask --help` 不可用的问题。
- 固件 submodule 保持本轮开始时的 gitlink，不升级固件版本。

| 实际统计 | 修改前 | 修改后 |
|----------|--------|--------|
| workspace members | 32 | 32 |
| 根 workspace 外部依赖声明 | 28 | 22 |
| Cargo.lock 包数 | 110 | 103 |
| lockfile 外部包数（含多版本与传递依赖） | 78 | 71 |
| Git 来源包数 | 2 | 2 |

## 保留项与升级阻塞

| 依赖 | 本轮选择 | 原因与后续触发条件 |
|------|----------|--------------------|
| `dma-api` | `0.7.3`，保持 0.7 兼容线 | 最新 `0.10.2` 删除 `DmaHandle`/`DBox` 等接口，重做 `DmaOp` 的连续分配、流式映射和释放契约。已实际编译确认不兼容；按作者决定留待单独迁移，需覆盖现有所有权、释放失败、错误映射与 DMA 验证。 |
| `aarch64-cpu` | 11.2.0 fork，lockfile 固定 revision | 当前 crates.io 11.2.0 源码仍缺项目所需 `DAIFSet`/`DAIFClr` 和 `asm::tlbi`；未用正式版替换 fork。上游具备对应接口后再迁移。 |
| `fatfs` | Git 0.4.0，更新到 `2aefc2a0` | crates.io 最新正式版 0.3.6 不对应当前 no_std/alloc 接口，保留现有来源。 |
| 传递依赖旧 major | 由消费者约束 | 包括 DMA 引入的旧 `aarch64-cpu`/`tock-registers`、其他过程宏使用的 syn 2；不通过 patch 强制替换不兼容版本。 |

## 验证与证据边界

`cargo deny 0.20.2 check` 返回成功：advisories、bans、licenses、sources 均通过。
仍有配置允许的多版本和 path 依赖通配符告警；不将它描述为零告警。
Git workspace 依赖补充包版本约束后，不再出现该工具的 `unresolved-workspace-dependency` 诊断。
`deny.toml` 中已有 `paste` 不再维护告警例外继续保留，不新增 advisory 豁免。

本轮构建、测试及环境限制统一见 [审计进度](audit-progress.md#第二轮验证与工具限制2026-09-22)。
上游版本核验使用 [Rust nightly manifest](https://static.rust-lang.org/dist/channel-rust-nightly.toml)、
[crates.io](https://crates.io/)、各 Actions 仓库 release、[PyPI](https://pypi.org/project/pre-commit/)、
[Node 发布索引](https://nodejs.org/dist/index.json) 和 npm registry；这些“最新”结论只对应核验日期。
