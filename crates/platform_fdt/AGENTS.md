<!-- Copyright The SimpleKernel Contributors -->

# platform_fdt

`crates/platform_fdt` 是 SimpleKernel 的平台描述层。它负责把 bootloader 传入的
DTB 复制到内核自有 storage，并提供平台无关的 FDT 查询 API，供内存初始化、CPU
拓扑、定时器、中断控制器和设备平台总线读取硬件描述。

## 职责

- `storage` 管理 kernel-owned DTB storage、`init_from_raw()`、`get()` 和 storage 区间查询。
- `query` 提供 path/compatible 查询、节点遍历、`reg` 解析和稳定 `FdtNodeId`。
- `error` 集中定义 `FdtError`，让调用方区分 header、layout、property 和重复初始化错误。
- `PlatformFdt::from_static()` 只面向静态 fixture 或已证明生命周期足够长的 DTB。

## 边界

- 本 crate 只解析和暴露 FDT 信息，不注册设备、不执行 driver probe、不映射 MMIO。
- 本 crate 不决定 RAM 布局策略；`memory` crate 负责解释查询结果并初始化内存子系统。
- 本 crate 不持有 `src/device` 或 `device_core` 状态；设备枚举由平台总线消费查询 API 完成。
- 本 crate 不声明 bootloader 传入的原始 DTB 会长期有效；正常启动必须走复制路径。

## 验证入口

- 文档-only 变更：`git diff --check`。
- 查询或 storage API 变更：`docker exec -w /workspace simplekernel-devcontainer cargo test -p platform_fdt`。
- lint/实现变更：`docker exec -w /workspace simplekernel-devcontainer cargo clippy -p platform_fdt -- -D warnings`。
- 影响内存 FDT 解析时：`docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --name memory-test/fdt-multi-memory --timeout 30`。
- 影响设备 compatible 遍历时：`docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --name device-test --timeout 30`。

## 不要假设

- 不要假设 DTB 只有一个 RAM 节点；多段 RAM 当前必须显式报错而不是静默取第一段。
- 不要假设查询结果数量固定；优先使用 `visit_nodes()`，避免把平台实例数量写死。
- 不要把 `FdtNodeId` 当成硬件全局唯一 id；它只是在当前 FDT 遍历结果内稳定。
- 不要绕过 `FdtError` 使用默认值掩盖缺失或非法 platform 输入。
