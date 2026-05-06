# 2026-05-06 DMA Crate 抽象设计

## 背景

- 需求来源：R3 内存审计发现 `src/device/hal.rs` 直接把 VirtIO `Hal` trait 的
  `dma_alloc`、`share`、`unshare` 绑定到 `AllocatedFrames`、identity mapping 和
  QEMU 行为上，缺少内核自己的 DMA 语义边界。
- 关联记录：`docs/audit/audit-progress.md` 中的 DMA / VirtIO HAL 权限语义审计。
- 适用范围：当前阶段只覆盖 QEMU VirtIO MMIO block；`crates/dma` 封装第三方
  `dma-api` crate，复用其 typed DMA 容器与 cache sync 调用形状，同时避免
  `dma-api` 类型直接扩散到 SimpleKernel 上层模块。

## 目标

- 新增 `crates/dma/`，作为内核内部 DMA 抽象 crate，并在 crate 内部封装
  `dma-api = "0.7.2"`。
- 对外暴露 SimpleKernel 命名的薄封装：`DmaDevice`、`DmaBuffer<T>`、
  `DmaArray<T>`、`StreamingMapping<T>`。
- 将 VirtIO HAL 的裸指针接口隔离在 `src/device/hal.rs`，内核内部通过
  `crates/dma` 调用 `dma-api` 的 `DeviceDma` / `DBox` / `DArray` /
  `SArrayPtr`。
- 当前实现提供 `QemuIdentityDmaOp`，实现 `dma_api::DmaOp`，保持现有 QEMU
  VirtIO 行为不变。
- 明确文档化：当前实现假设 SAS identity mapping + QEMU VirtIO，不承诺
  non-coherent AArch64 真机 DMA 正确性。

## 非目标

- 不实现通用硬件 DMA 子系统。
- 不自研 `DmaBuffer<T>` 容器内部逻辑，优先复用 `dma-api::DBox<T>`。
- 不新增 `kernel_dma_coherent()` PTE factory 或 Normal-NC MAIR 属性。
- 不实现 AArch64 `dc cvac` / `dc ivac` / `dc civac` cache maintenance。
- 不实现 bounce buffer、DMA mask、IOMMU 或设备 capability 枚举。
- 不让 `dma-api` 类型直接成为 SimpleKernel 设备层和未来驱动层的公共边界。

## 用户/调用方场景

| 场景 | 调用方 | 输入 | 预期输出 | 失败处理 |
|------|--------|------|----------|----------|
| VirtIO coherent 分配 | `src/device/hal.rs::dma_alloc` | 页数、方向 | 物理地址 + 非空虚拟指针 | 分配失败 panic，暴露内核资源问题 |
| VirtIO coherent 释放 | `src/device/hal.rs::dma_dealloc` | 物理地址、虚拟指针、页数 | 释放记录并归还帧 | 找不到记录时返回错误码并记录日志 |
| VirtIO streaming 共享 | `src/device/hal.rs::share` | buffer、方向 | streaming mapping 的物理地址 | 当前 QEMU 实现只做 VA -> PA |
| VirtIO streaming 取消共享 | `src/device/hal.rs::unshare` | 物理地址、buffer、方向 | 结束映射 | 当前 QEMU 实现 no-op |
| typed coherent buffer | 未来 SimpleKernel 驱动 | `T`、方向、对齐 | `DmaBuffer<T>` | 包装 `dma-api::DmaError` |
| typed DMA array | 未来 SimpleKernel 驱动 | `T`、长度、方向、对齐 | `DmaArray<T>` | 包装 `dma-api::DmaError` |
| 后续真机扩展 | 未来平台 DMA 实现 | buffer、方向、平台能力 | cache sync 或 non-cacheable 映射 | 由新实现返回明确错误或 panic 策略 |

## 接口需求

| 接口/类型 | 必须支持 | 不支持 | 错误语义 |
|-----------|----------|--------|----------|
| `DmaDirection` | 包装 `dma_api::DmaDirection`，避免第三方枚举直接外泄 | 设备私有方向位 | 纯值类型，无错误 |
| `DmaDevice` | 包装 `dma_api::DeviceDma`，固定 SimpleKernel 的设备 DMA 入口 | 直接暴露 `DeviceDma` | 创建时绑定 `dma_mask` 与 `QemuIdentityDmaOp` |
| `DmaBuffer<T>` | 包装 `dma_api::DBox<T>`，支持 `read` / `write` / `modify` / `dma_addr` | 自研 typed 容器内部逻辑 | 返回包装后的 `DmaError` |
| `DmaArray<T>` | 包装 `dma_api::DArray<T>`，支持固定长度 typed coherent buffer | 动态 Vec 语义 | 返回包装后的 `DmaError` |
| `StreamingMapping<T>` | 包装 `dma_api::SArrayPtr<T>`，用于已有 buffer 的 streaming map | 自动生命周期借用证明 | 返回包装后的 `DmaError` |
| `RawDmaRegion` | 供 VirtIO HAL `dma_alloc(pages)` 使用的页级 raw coherent region | 作为上层驱动首选 API | tracker 找不到时返回错误 |
| `QemuIdentityDmaOp` | 实现 `dma_api::DmaOp`，接入 frame allocator 与 identity mapping | 真机 cache/PTE 语义 | 文档中明确边界 |

## Crate 边界

`crates/dma` 是 `no_std` crate，定位在设备驱动和底层内存 crate 之间。它是
SimpleKernel 对 `dma-api` 的唯一直接依赖点。

依赖方向：

```text
src/device/hal.rs
  -> crates/dma
      -> dma-api
      -> crates/frame_allocator
      -> crates/memory_types
      -> crates/sync
      -> crates/config
```

`crates/dma` 不依赖 `src/arch`、`src/device` 或 `virtio-drivers`。`dma-api`
类型不直接出现在 `crates/dma` 以外的内核接口中；未来如果替换 `dma-api`，
应只改 `crates/dma`。

## 数据流

coherent allocation：

```text
virtio-drivers::Hal::dma_alloc
  -> dma::raw_alloc_pages
      -> QemuIdentityDmaOp::alloc_coherent
          -> AllocatedFrames::alloc
          -> PA.to_virt()
          -> 清零
          -> tracker 持有 AllocatedFrames / DmaHandle
  -> 返回 (paddr, vaddr)
```

streaming mapping：

```text
virtio-drivers::Hal::share
  -> dma::raw_map_single
      -> QemuIdentityDmaOp::map_single
          -> VA.to_phys()
  -> 返回 paddr

virtio-drivers::Hal::unshare
  -> dma::raw_unmap_single
      -> no-op
```

typed buffer：

```text
future driver
  -> dma::DmaDevice::new_qemu_identity(...)
  -> DmaDevice::buffer_zeroed::<T>(...)
      -> dma_api::DeviceDma::box_zero_with_align::<T>(...)
      -> DmaBuffer<T>(dma_api::DBox<T>)
```

## 安全与错误处理

- `QemuIdentityDmaOp` 的安全前提写在 crate 文档和类型文档中：SAS identity mapping、
  QEMU VirtIO 设备模型、guest RAM 可直接由设备模型访问。
- `dma-api` 的 `DBox<T>` 对 `T` 的语义约束不足以表达“设备可安全解释此类型”。第一版
  `crates/dma` 只在文档中要求 DMA 描述符类型使用 `#[repr(C)]`、无引用、无 Drop；
  若后续需要硬约束，再追加 `DmaPod` marker trait。
- raw `coherent_alloc` 分配失败在 crate 层返回 `DmaError`，由 VirtIO HAL 按
  `virtio-drivers::Hal` trait 约束转成带数据的 `.expect(...)`。
- raw `coherent_dealloc` 找不到分配记录时返回错误，由 VirtIO HAL 转成 `-1` 并记录日志。
- `QemuIdentityDmaOp` 的 streaming map 当前不做真实 cache clean，streaming unmap
  当前不做真实 invalidate；文档必须明确这只适用于当前 QEMU 阶段。
- AArch64 下 `dma-api` 会带入 `aarch64-cpu-ext` 依赖；实施前必须在容器内做
  riscv64/aarch64 check，确认 target JSON、`no_std` 和现有 `aarch64-cpu` fork 不冲突。
- 所有 unsafe 块继续保留 `// SAFETY:` 中文说明。

## 验收标准

- [ ] 新增 `crates/dma` 并登记到 workspace。
- [ ] workspace 添加 `dma-api = "0.7.2"` 依赖，并记录其无 feature flags 的约束。
- [ ] 根 crate 添加 `dma = { path = "crates/dma" }` 依赖。
- [ ] `crates/dma` 实现 `QemuIdentityDmaOp: dma_api::DmaOp`。
- [ ] `crates/dma` 暴露 `DmaDevice`、`DmaBuffer<T>`、`DmaArray<T>`、
      `StreamingMapping<T>` 薄封装。
- [ ] `crates/dma` 提供 raw 页级接口供 `virtio-drivers::Hal::dma_alloc` /
      `dma_dealloc` / `share` / `unshare` 使用。
- [ ] `src/device/hal.rs` 不再直接依赖 `frame_allocator::AllocatedFrames` 或本地
      `DMA_TRACKER`。
- [ ] `src/device/hal.rs` 通过 `dma` crate 完成 VirtIO HAL 适配。
- [ ] `cargo fmt --all -- --check` 通过。
- [ ] 容器内 `cargo xtask check --arch riscv64` 通过。
- [ ] 容器内 `cargo xtask check --arch aarch64` 通过。
- [ ] 容器内 VirtIO 相关 QEMU 测试通过，命令设置 30 秒超时。
- [ ] 更新 `docs/audit/audit-progress.md`，把当前 DMA 状态改为“`crates/dma`
      已封装 `dma-api`，QEMU VirtIO 抽象已建立；真机 DMA 仍待后续设计”。

## 文档同步

- 需要 ADR：建议新增 `docs/adr/014-qemu-virtio-dma-abstraction.md`，状态为“提议”，
  记录现阶段封装 `dma-api`，不直接外泄第三方类型，不实现真机 non-coherent DMA。
- 需要 crate README：新增 `crates/dma/README.md`，说明接口、第三方依赖边界和当前
  QEMU 实现。
- 需要审计进度更新：同步 `docs/audit/audit-progress.md` 的下一步和未决设计问题。
- 暂不需要硬件、供应商或 SOP 文档。
