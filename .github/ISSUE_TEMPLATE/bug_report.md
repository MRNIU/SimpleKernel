---
name: Bug report
about: Report a SimpleKernel bug
title: "[BUG]"
labels: bug
assignees: ''

---

## 问题描述

请简要说明 bug 的现象、影响范围和是否可稳定复现。

## 复现步骤

1. 使用的分支或 commit：
2. 使用的架构：`riscv64` / `aarch64`
3. 执行的命令：
   ```bash
   docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --timeout 30
   ```
4. 实际结果：

## 期望行为

说明你期望看到的行为或输出。

## 日志或截图

请粘贴关键错误、panic、QEMU 串口日志或 CI 链接；不要粘贴无关长日志。

## 环境

- 运行位置：Dev Container / Codespaces / CI
- 容器入口：`simplekernel-devcontainer`
- 宿主机操作系统：
- QEMU 命令是否使用 `--timeout 30`，如放宽请说明原因：
- 是否修改过 Dev Container、固件、QEMU 参数或测试镜像：

## 其他上下文

补充相关设计文档、ADR、PR 或 issue 链接。
