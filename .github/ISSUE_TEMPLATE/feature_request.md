---
name: Feature request
about: Suggest a SimpleKernel feature or improvement
title: ''
labels: feature request
assignees: ''

---

## 背景

说明这个需求对应的学习目标、内核子系统、架构边界或开发痛点。

## 期望方案

描述你希望 SimpleKernel 支持的行为、接口或文档改进。

## 影响范围

- 需求类型：Rust API / 内核子系统 / QEMU 系统测试 / Dev Container / `xtask` / 文档
- 目标架构：`riscv64` / `aarch64` / 双架构 / 不适用
- 可能涉及的模块或 crate：
- 是否影响 Dev Container、QEMU、固件、CI 或 `xtask` 入口：

## 替代方案

列出你考虑过的其他方案、参考内核或已有 issue/ADR。

## 验证思路

说明可以用哪些命令、测试包或文档检查验证该需求；QEMU 运行和系统测试默认使用 `--timeout 30`。

```bash
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --timeout 30
```
