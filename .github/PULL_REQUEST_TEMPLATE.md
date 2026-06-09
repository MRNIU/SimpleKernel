<!-- Copyright The SimpleKernel Contributors -->

## 摘要

TODO

## 变更内容

- TODO

## 影响面检查

- [ ] 公开 trait、API、错误码、数据结构或协议边界无变化，或已在摘要/风险中说明
- [ ] 启动流程、架构边界、SAS 不变量或跨模块依赖无变化，或已补充 ADR/SAD/SDD/RFC/Spec/Plan
- [ ] Rust `unsafe`、并发、锁序、中断上下文或内存权限无变化，或已说明验证方式
- [ ] 第三方源码、固件、submodule、生成物或外部交付物无变化，或已说明来源、版本、许可证、验证与真值源
- [ ] QEMU、固件链路、目标平台假设或外部边界无变化，或已同步更新对应设计/审计文档
- [ ] 开发环境、CI、Dev Container、QEMU 或发布流程无变化，或已更新文档
- [ ] 没有破坏性变化，或已用 `!`/`BREAKING CHANGE` 和迁移说明标注
- [ ] 每个 commit 都包含 DCO `Signed-off-by` trailer，或已说明例外原因

## 测试

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo xtask test --arch riscv64`
- [ ] 其他：TODO

## 文档

- [ ] 如果命令或入口变化，已更新 `README.md`
- [ ] 如果长期约定变化，已更新 `docs/conventions.md` 或 `AGENTS.md`
- [ ] 如果架构决策变化，已更新 `docs/adr/`
- [ ] 如果适用，已更新 SAD/SDD/RFC/Spec/Plan

## 风险与回滚

TODO
