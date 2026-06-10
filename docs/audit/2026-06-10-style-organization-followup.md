<!-- Copyright The SimpleKernel Contributors -->

# 代码风格与文件组织追踪表

本文记录 2026-06-10 对现有代码风格、文件组织和文档入口规则的排查结论。用途是给后续
subagent 分批执行整改；本文只记录已确认的处理方向，不替代具体实现计划。

## 执行原则

- 先按仓库当前真值面更新文档和规则入口，再处理代码风格追溯。
- 旧事实、旧工具链、旧 crate 名称应清理，不保留“历史兼容”表述，除非对应文档明确是历史设计记录。
- 后续执行时每个 subagent 应选择互不重叠的文件范围，避免同时修改同一模块。
- 代码改动后按影响面运行验证；文档-only 改动至少运行 `git diff --check`。

## 本轮执行状态

2026-06-10 本轮已用多个 subagent 并行执行，并由主线复扫、补漏和验证。原始待处理表保留作
排查索引，当前状态如下：

- 已落地：编号 1、2、5、6、7、8、11、14、16、18、19。
- 已确认当前满足或无需新增修改：编号 3（18 个 workspace crate 均已有 `AGENTS.md`）、
  4（repo-owned Rust 文件均已有 `//!` 模块说明）、13（无裸 `#[allow]`）、
  15（无 `.map_err(|_| ...)` 残留）。
- 已由当前代码语义覆盖并复验：编号 9、10、12；AArch64 boot argv、FDT、CORE_COUNT、
  panic/expect/assert 诊断均保留触发数据，运行时/platform 输入不走静默 fallback。
- 编号 17：`crates/device_core/src/registry.rs` 在本轮开始前已拆分；本轮只同步了
  `crates/device_core/AGENTS.md` 的拆分后文件地图。

最终验证见 `docs/audit/audit-progress.md` 中
“验证结果（2026-06-10 代码风格与文件组织 follow-up）”。

## 原始待处理表

| 编号 | 处理结论 | 范围 | 后续动作 |
|------|----------|------|----------|
| 1 | 更新 README | `README.md`、`README_ENG.md` | 清理旧 C++、CMake、GoogleTest、Doxygen 内容，用当前 Rust、`no_std`、Dev Container、`xtask` 入口重写或删除英文入口。 |
| 2 | 旧 crate 名全仓清干净 | `arch`、`fdt` 等旧命名残留 | 全仓扫描并改成当前 `arch_primitives`、`platform_fdt` 等真实名称；历史设计文档如需保留旧名，应明确标注“历史背景”。 |
| 3 | crate-local 手册都需要 | workspace crate | 为缺失的 workspace crate 补局部 `AGENTS.md`，至少覆盖职责、边界、验证入口和不要假设的事项。 |
| 4 | Rust 文件都要添加模块说明 | repo-owned `.rs` 文件 | 所有 repo-owned Rust 文件在版权头后补 `//!` 文件或模块职责说明，再接 crate 属性、`use` 和代码。 |
| 5 | 不新增 `rustfmt.toml`，修正文档表述 | 格式化规则文档 | 当前仓库没有该文件；将文档改为“使用 rustfmt 默认 100 字符宽度，执行 `cargo fmt --all`”，避免声明不存在的配置文件。 |
| 6 | 更新 `.gitmessage`，文档删重复规则 | commit 规则 | 以 `.gitmessage` 作为详细提交模板；`README.md`、`CONTRIBUTING.md`、`AGENTS.md` 只保留最小入口并指向 `.gitmessage`，避免多处真值面。 |
| 7 | QEMU 默认 30 秒，可按场景放宽 | QEMU run/test timeout | 文档统一写默认 `--timeout 30`；低性能宿主机或特殊测试允许显式放宽，但应在命令或说明中写清楚原因。 |
| 8 | 容器入口统一说明 | PR 模板、局部手册、验证说明 | 宿主机侧命令写明通过 `simplekernel-devcontainer` 执行；裸 `cargo ...` 仅用于已在容器内的语境。 |
| 9 | 理论不应失败路径改为 fail fast | `CORE_COUNT` 等运行时/platform 输入 | 移除隐式 fallback；未初始化、非法或不一致输入直接暴露为 panic 或明确错误，并包含可定位数据。 |
| 10 | boot 参数异常直接 panic | aarch64 boot argv/parse 路径 | 解析失败不再默认为 0 或空串；直接 panic，并带原始地址、长度、字符串或解析值。 |
| 11 | unsafe 统一强制 `SAFETY:` 格式 | `unsafe impl`、`unsafe extern`、unsafe block | 所有 unsafe 入口都补 `// SAFETY:`，说明不变量、调用前提和失败后果；不只限制普通 unsafe block。 |
| 12 | panic/expect 诊断带触发数据 | `.expect()`、`panic!()`、`assert!()` 相关路径 | 信息中补地址、大小、core id、索引、队列状态、设备 reg 等实际数据，避免只写“应成功”。 |
| 13 | `#[allow]` 改为有 reason 的 `#[expect]` | lint suppression | 优先使用 `#[expect(..., reason = "...")]`；宏生成代码如需保留例外，也要写清楚原因。 |
| 14 | public API 文档契约全仓追溯 | `pub`、`pub(crate)` API | 返回 `Result` 的公开入口补 `# Errors`；会 panic 的公开入口补 `# Panics`；`unsafe fn` 补 `# Safety`。该规则也适用于 `xtask` 中对外暴露的模块函数。 |
| 15 | 保留或解释错误来源 | <code>.map_err(&#124;_&#124; ...)</code> | 全仓扫描忽略原始错误的转换；能保留原始错误就改为保留，确实无有用信息的错误也要说明原因，并确保新错误含输入值、容量、地址或索引等定位数据。 |
| 16 | 注释中文化且避免行尾注释 | repo-owned 注释和文档注释 | 注释和文档注释使用中文；行尾注释上移到代码上方；保留 `SAFETY:`、`Copyright`、`@generated` 等约定前缀。 |
| 17 | 超大文件拆分 | 超 300/500 行文件 | 优先拆分 `crates/device_core/src/registry.rs`；随后处理其他超过 300 行的文件，按职责拆到子模块或局部 helper。 |
| 18 | GitHub issue template 删除旧内容后重写 | `.github/ISSUE_TEMPLATE/*.md` | 删除 i386、Bochs、`tools/env.sh` 等旧字段，改成当前 Rust、QEMU、Dev Container 表单；这些模板不需要补版权头。 |
| 19 | 旧 target 示例改为 `xtask` 入口 | `Cargo.toml` 注释和公开命令示例 | 删除 `targets/riscv64-none.json` 等旧示例；公开入口统一写 `cargo xtask ...`，底层 target 细节只在必要的工具链文档中说明。 |

## 已确认的非问题

- 没有发现裸 `.unwrap()`。
- 没有发现真实 `static mut` 定义。
- 没有发现 `spin::Mutex`。
- 没有发现目录级 README 扩散。
- JSON 文件可被严格解析。
- GitHub workflow 未发现临时安装 Rust、QEMU 或交叉工具链来绕过 Dockerfile 的做法。

## 建议拆分顺序

1. 文档入口与旧事实清理：编号 1、2、5、6、7、8、18、19。
2. crate-local 手册与文件组织：编号 3、17。
3. Rust 文件头、注释和 API 文档追溯：编号 4、14、16。
4. 运行时 fail-fast 与诊断质量：编号 9、10、12、15。
5. unsafe 与 lint suppression：编号 11、13。
