# 依赖审计

> **生成日期**：2026-04-03
> **分支**：`feat/rust-SAS`（commit: aff99bdd8）

## 总览

| 指标 | 数量 |
|------|------|
| Workspace crate | 18 |
| 外部依赖（含传递） | 65 |
| Cargo.lock 总包数 | 88 |
| Git 依赖（非 registry） | 2 |

## Git 依赖

| 依赖 | 版本 | 来源 | 状态 | 计划 |
|------|------|------|------|------|
| `aarch64-cpu` | 11.2.0 | `github.com/MRNIU/aarch64-cpu` branch `feat/add-tlbi-instructions` | Fork，等待上游 PR `rust-embedded/aarch64-cpu#77` | 上游合入后切回 crates.io |
| `fatfs` | 0.4.0 | `github.com/rafalh/rust-fatfs` | 无 crates.io 发布 | 更新到最新 commit，继续使用 |

## 值得关注的版本

| 依赖 | 版本 | 备注 | 行动 |
|------|------|------|------|
| `fdt` | `0.2.0-alpha1` | Alpha 版本，API 可能不稳定 | 暂不替换，R6 设备审查时重新评估 |
| `sbi-rt` | `0.0.3` | 0.x 系列 | 关注上游 breaking changes |
| `spin` | `0.10` | 仅用 `Once<T>` | 稳定，无需变更 |

## 许可证分布

| 许可证 | crate 数量 | 兼容性 |
|--------|-----------|--------|
| MIT OR Apache-2.0 | 42 | ✅ |
| MIT/Apache-2.0 | 6 | ✅ |
| MIT | 6 | ✅ |
| 0BSD | 2 | ✅ |
| BSD-3-Clause OR MIT OR Apache-2.0 | 2 | ✅ |
| MulanPSL-2.0 OR MIT | 2 | ✅（木兰宽松许可证） |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 | ✅ |
| Unlicense OR MIT | 1 | ✅ |
| MPL-2.0 | 1（`fdt`） | ⚠️ file-level copyleft，与 MIT 兼容 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 | ✅ |

**结论**：所有依赖许可证均与项目 MIT 许可证兼容。`fdt` 的 MPL-2.0 是 file-level copyleft，不影响项目整体许可。

## Workspace 内部依赖关系

详见 `docs/diagrams/crate-dependency-graph.md`。

## `deny.toml` 配置

已创建 `deny.toml`，包含：
- `[licenses]`：上述所有许可证的 allowlist
- `[bans]`：多版本警告、禁止通配符版本
- `[advisories]`：CVE 扫描（阻断）、不维护警告、已撤回阻断
- `[sources]`：仅允许 crates.io + 两个已知 git 源
