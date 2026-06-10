<!-- Copyright The SimpleKernel Contributors -->

# 贡献指南

感谢参与 SimpleKernel。提交前请先阅读根目录 `AGENTS.md`、`docs/conventions.md` 和本文件。

## 开发环境

开发环境默认使用 Dev Container。宿主机只保留 Docker 或兼容容器运行时、Git、编辑器/AI agent 和已有 Dev Container 入口工具；不要为了本项目在宿主机安装 Rust nightly、交叉编译器、QEMU、固件构建依赖或其他项目开发依赖。

```bash
devcontainer up --workspace-folder .
docker exec -w /workspace simplekernel-devcontainer bash
```

宿主机侧常用命令：

```bash
docker exec -w /workspace simplekernel-devcontainer cargo xtask build --arch riscv64
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --timeout 30
docker exec -w /workspace simplekernel-devcontainer cargo fmt --all -- --check
docker exec -w /workspace simplekernel-devcontainer cargo clippy -- -D warnings
```

通过 Bash 工具运行 QEMU 相关命令默认使用 `--timeout 30`；低性能宿主机或特殊测试可以显式放宽，但应写清楚原因。超时后清理残留 QEMU 进程。

## 提交流程

1. 从最新主分支创建小范围分支。
2. 按 trait 契约、设计文档和 ADR 理解边界。
3. 修改代码时同步更新测试和文档。
4. 运行与改动范围匹配的验证命令。
5. 使用 `git commit --signoff` 提交。
6. 创建 PR，并按 PR 模板说明测试、文档、风险和回滚。

## 文档同步要求

| 改动 | 必须同步检查 |
|------|--------------|
| 启动流程、命令、测试入口变化 | 根 `README.md`、`docs/AGENTS.md`、相关局部 `AGENTS.md`、相关设计或计划文档 |
| 架构不变量变化 | `docs/adr/`、SAD/SDD、`AGENTS.md` |
| 项目长期约定、Copyright、注释、文件规模、运行时配置规则变化 | `AGENTS.md`、`docs/conventions.md` |
| Git/commit/DCO/提交模板变化 | `.gitmessage` 是详细真值面；`AGENTS.md`、`README.md` 和本文件只保留最小入口 |
| 公开 trait、错误码、类型或模块边界变化 | 代码文档注释、SDD、最近的局部 `AGENTS.md` |
| 固件、第三方源码或外部交付物变化 | `3rd/` 记录、`README.md`、相关 ADR/设计/审计文档 |
| QEMU、固件链路或目标平台假设变化 | `README.md`、相关设计文档、相关测试说明 |

## 代码约定

- 注释和文档注释使用中文；`# Safety`、`# Errors`、`# Panics` 节标题保留英文。
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释。
- 不使用 `.unwrap()`；错误信息必须包含有助于定位问题的数据。
- 内核互斥使用项目自定义 `SpinLock<T>`。
- trait 是契约，不要为了某个实现把实现细节塞进 trait 定义。
- 错误类型和 Result alias 放在所属 crate 或子系统最近的 `error.rs`，并向上层暴露；不新增项目级全局 Result alias，syscall/ABI 边界显式转换错误码。
- crate 级 host 测试放在与 `src/` 同级的 `tests/` 目录；模块私有白盒测试内联在实现文件的 `#[cfg(test)] mod tests` 中；系统/QEMU 测试保持仓库 `tests/*/src/*.rs` 独立二进制。
- 新增自有源码、脚本、CI 配置、重要配置和长期维护文档时按 `docs/conventions.md` 添加 Copyright 文件头；Rust 源文件版权头后必须有 `//!` 文件或模块说明。
- 手写源码超过 300 行时主动检查职责边界；超过 500 行时 PR 说明暂不拆分理由或拆分计划。
- 运行时/platform 输入缺失或非法时 fail fast，不用隐式默认值掩盖配置或硬件描述问题。

## Commit

commit 必须带 DCO sign-off。提交格式、正文、footer 和发布卫生规则以 [.gitmessage](./.gitmessage) 为唯一详细模板；本文件不重复维护详细规则。

可选启用仓库提交模板：

```bash
git config commit.template .gitmessage
```

PR CI 会检查每个 commit 是否包含 `Signed-off-by` trailer。
