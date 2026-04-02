# 架构决策记录（ADR）

本目录记录 SimpleKernel 审计与重构过程中的重要架构决策。

## 模板

新建 ADR 时，复制 `docs/templates/adr-template.md`，文件命名为 `NNNN-简短描述.md`（如 `0001-mmio-region-lifetime.md`）。

## 索引

| 编号 | 标题 | 状态 | 日期 | 阶段 |
|------|------|------|------|------|
| （审计开始后在此追加） | | | | |

## 状态规则

- AI 生成的 ADR 状态**必须为"提议"**。只有项目作者 review 后才可改为"已接受"。
- "已废弃"和"已取代"同样只能由项目作者标记。

## 何时需要写 ADR

- 在两个以上合理方案中做了选择（如 `ManuallyDrop` vs `mem::forget`）
- 改变了现有设计的方向（如从 `dyn Trait` 改为 enum dispatch）
- 引入了新的 Rust 范式（如 typestate 编码状态机）
- 决定**不做**某件事（如决定不引入 RwLock），且理由不显而易见
