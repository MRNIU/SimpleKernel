# Git 与 Commit 规范

## Git 规则

- PR 应小而可审，一个 PR 只处理一个主要目标。
- 一个 commit 尽量只处理一个关注点。
- 不把无关重构混入功能、bugfix 或审计修复。
- 未经明确批准，不重写共享历史。
- 破坏性操作需要人工确认，包括 force push、重写历史、批量删除、reset 和生产数据修改。
- 提交前检查优先在 Dev Container 或 CI 声明的隔离环境中执行。

## Commit 格式

采用 Conventional Commits 风格：

```text
<type>(<scope>): <subject>

<body>

<footer>
```

允许的 `type`：

| type | 使用场景 |
|------|----------|
| `feat` | 新功能或能力 |
| `fix` | bug 修复 |
| `refactor` | 不改变外部行为的重构 |
| `test` | 测试新增或调整 |
| `docs` | 文档变更 |
| `chore` | 维护性杂项 |
| `build` | 构建系统、依赖、工具链 |
| `ci` | CI/CD 配置 |
| `perf` | 性能优化 |
| `style` | 格式、空白、排序等不改变语义的变更 |
| `revert` | 回退先前提交 |

规则：

- `scope` 使用英文小写，表示影响范围，例如 `memory`、`sync`、`arch`、`xtask`、`docs`。
- `subject` 优先使用中文，不加句号，尽量不超过 72 个字符。
- 破坏性变化必须使用 `!` 并在正文或 footer 中写明迁移方式。
- 每条 commit 必须使用 DCO sign-off：`git commit --signoff` 或 `git commit -s`。
- 禁止使用无信息量提交信息，例如 `update`、`fix bug`、`misc`、`wip`、`changes`。

仓库提供 `.gitmessage` 作为可选提交模板；需要时可执行：

```bash
git config commit.template .gitmessage
```

示例：

```text
docs(conventions): 补充文档类型边界
fix(memory): 修复页表权限覆盖的错误码
test(sync): 增加中断重入场景测试
refactor(xtask)!: 调整测试参数解析
```

## 分支建议

```text
feat/<topic>
fix/<topic>
docs/<topic>
audit/<phase-or-module>
```

分支名使用英文小写和连字符，避免包含个人姓名、机器名或临时目录。
