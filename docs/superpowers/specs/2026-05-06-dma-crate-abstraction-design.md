# 2026-05-06 DMA Crate 抽象设计

## 背景

- 需求来源：R3 内存审计发现 `src/device/hal.rs` 直接把 VirtIO `Hal` trait 的
  `dma_alloc`、`share`、`unshare` 绑定到 `AllocatedFrames`、identity mapping 和
  QEMU 行为上，缺少内核自己的 DMA 语义边界。
- 关联记录：`docs/audit/audit-progress.md` 中的 DMA / VirtIO HAL 权限语义审计。
- 适用范围：当前阶段只覆盖 QEMU VirtIO MMIO block；接口需要为后续真机 DMA、
  cache maintenance、coherent allocation 和 streaming mapping 留出扩展点。

## 目标

- 新增 `crates/dma/`，作为内核内部 DMA 抽象 crate。
- 将 VirtIO HAL 的裸指针接口隔离在 `src/device/hal.rs`，内核内部使用 `dma`
  crate 的类型表达 DMA 方向、coherent buffer 和 streaming mapping。
- 当前实现提供 `QemuIdentityDma`，保持现有 QEMU VirtIO 行为不变。
- 明确文档化：当前实现假设 SAS identity mapping + QEMU VirtIO，不承诺
  non-coherent AArch64 真机 DMA 正确性。

## 非目标

- 不实现通用硬件 DMA 子系统。
- 不新增 `DmaBuffer<T>` 所有权封装。
- 不新增 `kernel_dma_coherent()` PTE factory 或 Normal-NC MAIR 属性。
- 不实现 AArch64 `dc cvac` / `dc ivac` / `dc civac` cache maintenance。
- 不实现 bounce buffer、DMA mask、IOMMU 或设备 capability 枚举。
- 不引入外部 DMA abstraction crate。

## 用户/调用方场景

| 场景 | 调用方 | 输入 | 预期输出 | 失败处理 |
|------|--------|------|----------|----------|
| VirtIO coherent 分配 | `src/device/hal.rs::dma_alloc` | 页数、方向 | 物理地址 + 非空虚拟指针 | 分配失败 panic，暴露内核资源问题 |
| VirtIO coherent 释放 | `src/device/hal.rs::dma_dealloc` | 物理地址、虚拟指针、页数 | 释放记录并归还帧 | 找不到记录时返回错误码并记录日志 |
| VirtIO streaming 共享 | `src/device/hal.rs::share` | buffer、方向 | streaming mapping 的物理地址 | 当前 QEMU 实现只做 VA -> PA |
| VirtIO streaming 取消共享 | `src/device/hal.rs::unshare` | 物理地址、buffer、方向 | 结束映射 | 当前 QEMU 实现 no-op |
| 后续真机扩展 | 未来平台 DMA 实现 | buffer、方向、平台能力 | cache sync 或 non-cacheable 映射 | 由新实现返回明确错误或 panic 策略 |

## 接口需求

| 接口/类型 | 必须支持 | 不支持 | 错误语义 |
|-----------|----------|--------|----------|
| `DmaDirection` | `ToDevice`、`FromDevice`、`Bidirectional` | 设备私有方向位 | 纯值类型，无错误 |
| `DmaCapabilities` | 描述是否默认 coherent、是否需要 cache maintenance | 自动探测硬件能力 | 纯值类型，无错误 |
| `CoherentAllocation` | 保存 PA、VA、页数，供 VirtIO HAL 返回 | 暴露可变 slice 所有权 | 构造由实现负责校验 |
| `StreamingMapping` | 保存 PA、VA、长度、方向 | 生命周期绑定 buffer 借用 | 当前只做值对象 |
| `DmaError` / `DmaResult<T>` | 表达分配失败、释放未知映射、无效 buffer | 复杂 errno 分类 | crate 层统一返回 `Result` |
| `DmaOps` | coherent alloc/dealloc、streaming map/unmap、capabilities | IOMMU、DMA mask | 除 `capabilities` 外返回 `DmaResult<T>` |
| `QemuIdentityDma` | 现有 QEMU VirtIO 最小路径 | 真机 cache/PTE 语义 | 文档中明确边界 |

## Crate 边界

`crates/dma` 是 `no_std` crate，定位在设备驱动和底层内存 crate 之间。

依赖方向：

```text
src/device/hal.rs
  -> crates/dma
      -> crates/frame_allocator
      -> crates/memory_types
      -> crates/sync
      -> crates/config
```

`crates/dma` 不依赖 `src/arch`、`src/device` 或 `virtio-drivers`。这样可以避免
DMA 抽象被 VirtIO trait 或具体架构实现反向污染。

## 数据流

coherent allocation：

```text
virtio-drivers::Hal::dma_alloc
  -> dma::QemuIdentityDma::coherent_alloc
      -> AllocatedFrames::alloc
      -> PA.to_virt()
      -> 清零
      -> tracker 持有 AllocatedFrames
  -> 返回 (paddr, vaddr)
```

streaming mapping：

```text
virtio-drivers::Hal::share
  -> dma::QemuIdentityDma::streaming_map
      -> VA.to_phys()
  -> 返回 paddr

virtio-drivers::Hal::unshare
  -> dma::QemuIdentityDma::streaming_unmap
      -> no-op
```

## 安全与错误处理

- `QemuIdentityDma` 的安全前提写在 crate 文档和类型文档中：SAS identity mapping、
  QEMU VirtIO 设备模型、guest RAM 可直接由设备模型访问。
- `coherent_alloc` 分配失败在 crate 层返回 `DmaError::OutOfMemory`，由 VirtIO HAL
  按 `virtio-drivers::Hal` trait 约束转成带数据的 `.expect(...)`。
- `coherent_dealloc` 找不到分配记录时返回 `DmaError::UnknownMapping`，由 VirtIO HAL
  转成 `-1` 并记录日志。
- `streaming_map` 当前不做 cache clean，`streaming_unmap` 当前不做 invalidate；文档必须明确
  这只适用于当前 QEMU 阶段。
- 所有 unsafe 块继续保留 `// SAFETY:` 中文说明。

## 验收标准

- [ ] 新增 `crates/dma` 并登记到 workspace。
- [ ] 根 crate 添加 `dma = { path = "crates/dma" }` 依赖。
- [ ] `src/device/hal.rs` 不再直接依赖 `frame_allocator::AllocatedFrames` 或本地
      `DMA_TRACKER`。
- [ ] `src/device/hal.rs` 通过 `dma` crate 完成 VirtIO HAL 适配。
- [ ] `cargo fmt --all -- --check` 通过。
- [ ] 容器内 `cargo xtask check --arch riscv64` 通过。
- [ ] 容器内 `cargo xtask check --arch aarch64` 通过。
- [ ] 容器内 VirtIO 相关 QEMU 测试通过，命令设置 30 秒超时。
- [ ] 更新 `docs/audit/audit-progress.md`，把当前 DMA 状态改为“QEMU VirtIO 抽象已建立；
      真机 DMA 仍待后续设计”。

## 文档同步

- 需要 ADR：建议新增 `docs/adr/014-qemu-virtio-dma-abstraction.md`，状态为“提议”，
  记录现阶段只建立 DMA crate 抽象、不实现真机 non-coherent DMA。
- 需要 crate README：新增 `crates/dma/README.md`，说明接口、边界和当前 QEMU 实现。
- 需要审计进度更新：同步 `docs/audit/audit-progress.md` 的下一步和未决设计问题。
- 暂不需要硬件、供应商或 SOP 文档。
