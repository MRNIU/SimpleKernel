<!-- Copyright The SimpleKernel Contributors -->

# ADR-014: QEMU VirtIO DMA 抽象封装 `dma-api`

> **状态**: 提议
>
> **日期**: 2026-05-06
>
> **审计阶段**: R3/R6 — DMA / VirtIO HAL
>
> **涉及模块**: `crates/dma`, `src/device/hal.rs`, `src/device/virtio.rs`

## 背景

当前 `virtio-drivers::Hal` 需要提供三类 DMA 能力：

- coherent DMA 分配与释放，用于 VirtIO queue、descriptor table、available ring
  和 used ring 等设备与 CPU 共同访问的数据结构；
- streaming DMA map/unmap，用于普通 buffer 在提交给设备前后转换地址并同步可见性；
- 将 SimpleKernel 的 SAS identity mapping 地址模型适配到 `virtio-drivers` 期望的
  `PhysAddr` / `NonNull<u8>` 边界。

旧实现把这些责任直接放在 `src/device/hal.rs`：

- `dma_alloc` 直接持有 `frame_allocator::AllocatedFrames`；
- VA/PA 转换依赖当前 QEMU + SAS identity mapping 假设；
- DMA tracker、裸指针转换、帧生命周期和 VirtIO HAL trait 适配混在同一层。

这让 `src/device/hal.rs` 同时承担 VirtIO 适配层、DMA 策略层和内存分配生命周期层三种
职责。它也不利于后续把 QEMU-only identity mapping 后端与真机 non-coherent DMA 语义区分开。

`dma-api 0.7.2` 已提供适合封装的基础抽象：

- `DeviceDma` 作为设备 DMA 操作入口；
- `DBox<T>` / `DArray<T>` 等 typed coherent DMA 容器；
- `SArrayPtr<T>` 表达 streaming mapping 的指针形状；
- `DmaOp` trait 作为 OS/平台后端接入点。

但 SimpleKernel 不应让 `dma_api::*` 类型直接扩散到设备层和未来驱动层。外部 crate 的 API
是实现依赖，不应成为内核上层模块的长期架构契约。

## 备选方案

### 方案 A：继续在 `src/device/hal.rs` 直接实现 DMA

维持 `src/device/hal.rs` 直接管理 `AllocatedFrames`、identity mapping 转换和 DMA tracker。

**优点**：

- 改动最少，当前 QEMU VirtIO 行为已经可运行。
- 不引入新的封装层，短期代码路径直接。

**缺点**：

- VirtIO trait 适配层继续承载 DMA 分配、生命周期和后端策略职责。
- typed DMA buffer 与 streaming mapping 的边界不清晰。
- 后续实现 cache maintenance、PTE 属性或 IOMMU/capability 时会继续挤在设备 HAL 中。
- 难以保证 `dma-api` 或其他第三方 DMA 抽象不会向上层泄漏。

### 方案 B：在 `crates/dma` 封装 `dma-api`

新增 `crates/dma` 作为 SimpleKernel 唯一直接依赖 `dma-api` 的边界。对外暴露
SimpleKernel 自己命名的 `DmaDevice`、`DmaBuffer<T>`、`DmaArray<T>` 和
`StreamingMapping<T>`，在 crate 内实现私有 QEMU identity backend `QemuIdentityDmaOp`。
`src/device/hal.rs` 只保留 `virtio-drivers::Hal` 适配逻辑。

**优点**：

- 第三方依赖边界清晰：上层模块依赖 SimpleKernel 的 DMA 名称，不依赖 `dma_api::*`。
- 当前 QEMU identity mapping 行为可以保持不变。
- DMA tracker、raw page helper、typed DMA 容器和 streaming mapping 集中在 DMA crate 内。
- 后续替换 DMA abstraction crate 或补真机后端时，主要修改点限制在 `crates/dma`。

**缺点**：

- 新增一层 crate 和 wrapper 类型，短期代码量增加。
- 当前 wrapper 只能表达 QEMU identity mapping 后端，不自动解决真机 non-coherent DMA。
- `virtio-drivers::Hal` 的 raw trait 边界仍需要适配裸指针，无法完全消除 unsafe 边界。

### 方案 C：直接在设备层公开使用 `dma-api`

让 `src/device/hal.rs` 和未来驱动直接使用 `dma_api::DeviceDma`、`DBox<T>`、
`DArray<T>` 和 `SArrayPtr<T>`。

**优点**：

- 少写本地 wrapper，可直接使用 `dma-api` 的类型和文档。
- 对熟悉该 crate 的读者而言语义直接。

**缺点**：

- 第三方 crate 成为 SimpleKernel 设备层公开架构边界。
- 未来替换 DMA 抽象或调整错误类型、cache 策略时会产生跨模块扩散修改。
- 不符合本项目 trait/门面先行的分层习惯。

## 决策

选择 **方案 B**：新增 `crates/dma`，作为 SimpleKernel 唯一直接依赖 `dma-api` 的封装层。

`crates/dma` 对外暴露 SimpleKernel 命名的 `DmaDevice`、`DmaBuffer<T>`、`DmaArray<T>`
和 `StreamingMapping<T>`。这些类型分别包装 `dma_api::DeviceDma`、`dma_api::DBox<T>`、
`dma_api::DArray<T>` 和 `dma_api::SArrayPtr<T>`。

当前后端实现为私有的 `QemuIdentityDmaOp`，仅承诺 QEMU VirtIO + SAS identity mapping
场景。`src/device/hal.rs` 保持为 `virtio-drivers::Hal` adapter，不再直接持有
`AllocatedFrames`、DMA tracker 或 `dma_api::*` 类型。

## 理由

### 保持当前 QEMU 行为不变

本决策不是一次真机 DMA 语义修复。当前阶段的目标是把已经可运行的 QEMU VirtIO DMA 路径
从设备 HAL 中拆出，形成后续可以扩展的边界。`QemuIdentityDmaOp` 保留当前假设：

- coherent allocation 仍使用连续物理页；
- VA/PA 转换仍基于 SAS identity mapping；
- QEMU 阶段仍不提供真实 cache clean / invalidate 或 PTE 属性切换语义。

### 第三方类型不向上泄漏

`dma-api` 的 typed container 和 `DmaOp` 形状有复用价值，但 SimpleKernel 上层模块不应直接
以 `dma_api::*` 作为架构契约。使用本地 wrapper 后：

- 上层只看见 `DmaDevice` / `DmaBuffer<T>` / `DmaArray<T>` / `StreamingMapping<T>`；
- `dma-api` 版本、错误类型和具体后端约束留在 `crates/dma` 内；
- 如果未来替换 DMA abstraction crate，主要影响本 crate，而不是设备层和驱动层。

### `src/device/hal.rs` 回到适配层职责

`virtio-drivers::Hal` 是外部驱动 crate 要求的 trait 边界。它需要返回裸 `PhysAddr` 和
`NonNull<u8>`，因此无法直接表达 SimpleKernel 希望长期拥有的 typed DMA 生命周期模型。
把 DMA 策略放进 `crates/dma` 后，`src/device/hal.rs` 只负责把 VirtIO trait 调用转发到
SimpleKernel DMA 边界，职责更接近 adapter。

### 真机 non-coherent DMA 仍需单独决策

本 ADR 不声明 non-coherent AArch64 真机 DMA 已正确。真实硬件仍需要后续设计：

- cache maintenance：streaming map/unmap 时的 clean / invalidate；
- PTE 属性：coherent DMA RAM 是否需要 Normal Non-Cacheable 或其他属性；
- 设备 DMA capability：DMA mask、IOMMU、bounce buffer 或 capability token。

这些问题需要继续审计和设计，不应被 QEMU identity backend 的通过误认为已经解决。

## 影响

- **代码变更**：
  - 新增 `crates/dma`，集中封装 `dma-api 0.7.2`。
  - 新增私有 QEMU identity backend `QemuIdentityDmaOp`。
  - `src/device/hal.rs` 改为 `virtio-drivers::Hal` adapter，委托 `crates/dma` 完成 raw DMA
    分配、释放和 streaming mapping。
- **API 变更**：
  - 新增 SimpleKernel DMA wrapper 类型：`DmaDevice`、`DmaBuffer<T>`、`DmaArray<T>`、
    `StreamingMapping<T>`。
  - 上层模块不得直接使用 `dma_api::*`。
- **行为影响**：
  - 当前 QEMU VirtIO 行为保持不变。
  - 不承诺 non-coherent AArch64 真机 DMA 正确性。
- **测试**：
  - 需要验证 `cargo check -p dma`。
  - 需要验证 `cargo xtask check --arch riscv64` 和 `cargo xtask check --arch aarch64`。
  - 需要验证 `cargo xtask test --arch riscv64 --name device-test`；QEMU 命令使用 30 秒超时，
    超时后清理残留 `qemu-system` 进程。
- **文档**：
  - `docs/adr/README.md` 增加 ADR-014 索引。
  - `crates/dma/README.md` 记录第三方依赖边界。
  - `docs/audit/audit-progress.md` 更新 DMA / VirtIO HAL 后续验证状态。

## 参考

- [`dma-api 0.7.2`](https://crates.io/crates/dma-api) — typed DMA container 与 `DmaOp`
  后端接口。
- [`virtio-drivers`](https://crates.io/crates/virtio-drivers) — `Hal` trait 定义了当前
  VirtIO DMA 适配边界。
- [ADR-013: 删除 `OwnedPages` 抽象层](013-ownedpages-necessity.md) — 说明 DMA 不应复用
  `OwnedPages`，而应拥有专用 `DmaBuffer<T>` 类型。
- `docs/audit/audit-progress.md` — R3 DMA / VirtIO HAL 权限语义审计记录。
