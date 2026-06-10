<!-- Copyright The SimpleKernel Contributors -->

# device_core

`crates/device_core` 是 SimpleKernel 的设备核心模型 crate。它只描述驱动
descriptor、probe 语义、稳定设备身份和 typed capability registry，让上层设备
管理器可以在不暴露具体驱动实现的情况下注册和查询设备能力。

## 职责

- `descriptor` 定义 `DriverDescriptor`、probe 类型、probe 优先级、probe 结果和失败原因。
- `capability` 定义上层可持有的 typed capability，例如 `BlockDevice` 和 sector I/O 校验。
- `registry` 维护内建 driver descriptor 集合、probe 统计、设备实例和 capability 绑定。
  - `registry.rs` 只作为模块入口，统一 re-export 对外 API 和固定容量常量。
  - `registry/driver_registry.rs` 负责 descriptor 集合校验、排序和 probe 统计。
  - `registry/capability_registry.rs` 负责设备实例、capability 绑定和默认块设备选择。
  - `registry/types.rs` 定义 `DeviceId`、设备来源和注册记录类型。
  - `registry/error.rs` 定义 registry 构造和注册错误。
- `DeviceId`、`DeviceSource` 和 `DeviceType` 只提供稳定诊断身份，不承诺全局硬件拓扑模型。

## 边界

- 本 crate 可依赖 `platform_fdt` 的节点 id、节点名和 `reg` 类型，用于表达 FDT probe context。
- 本 crate 不拥有 DTB 生命周期，不扫描全局 FDT，也不决定哪些节点应被 probe。
- 本 crate 不实现 VirtIO、MMIO、DMA、文件系统或具体 block device 后端。
- 本 crate 不做动态驱动加载；当前 descriptor 集合仍是启动期内建驱动清单。
- registry 的固定容量是第一版启动期约束，调整容量时必须同步错误诊断和相关测试。

## 验证入口

- 文档-only 变更：`git diff --check`。
- API 或实现变更：`docker exec -w /workspace simplekernel-devcontainer cargo clippy -p device_core -- -D warnings`。
- registry/probe 纯模型语义变更：`docker exec -w /workspace simplekernel-devcontainer cargo test -p device_core`。
- 与平台总线或 VirtIO 绑定行为相关的变更：`docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --name device-test --timeout 30`。

## 不要假设

- 不要把 `FdtProbeContext` 当成 FDT 解析器；它只是平台总线传入 probe 的只读上下文。
- 不要在 public API 中泄漏具体驱动类型、DMA wrapper 或文件系统类型。
- 不要把 probe skip 写成成功绑定；`Skipped`、`Failed` 和 `Bound` 是三类不同结果。
- 不要把默认 block device 规则扩展成通用优先级系统；需要新策略时先更新设计文档。
