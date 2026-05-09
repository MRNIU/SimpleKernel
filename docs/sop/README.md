<!-- Copyright The SimpleKernel Contributors -->

# SOP

本目录保存标准作业流程。适用于固件构建、QEMU 验证、硬件诊断、生产测试、供应商验收、发布和回滚等重复操作。

## 使用规则

- 新建 SOP 时，从 `docs/templates/sop.md` 复制。
- SOP 必须包含前置条件、流程图、操作步骤、验收标准、失败处理、记录要求和回滚方式。
- 命令必须说明运行位置：容器内、CI、QEMU、硬件目标或生产环境。
- 涉及 QEMU 的 Bash 命令必须设置 30 秒超时，并说明残留进程清理方式。

## 命名

```text
docs/sop/topic.md
```
