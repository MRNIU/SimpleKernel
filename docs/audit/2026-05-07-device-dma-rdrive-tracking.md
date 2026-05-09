<!-- Copyright The SimpleKernel Contributors -->

# 设备与 DMA 真机语义跟踪

> 日期：2026-05-07
>
> 来源：R3 内存层复审第三组问题
>
> 范围：`crates/dma`、`src/device/hal.rs`、VirtIO 块设备路径、页表设备属性、
> 后续 rdrive 集成评估。

## 目标

本文把 R3 复审中剩余的设备/DMA 问题从内存层报告中独立出来，作为后续真机与
rdrive 集成评估的跟踪入口。

当前结论：

- 这些问题不阻塞 QEMU VirtIO + SAS identity mapping 路径。
- 这些问题阻塞 non-coherent 真机设备支持声明。
- 后续若引入 rdrive，应把 rdrive 的 DMA 能力、地址限制、缓存一致性模型和中断/队列
  形态一起纳入评估，而不是只在 VirtIO 路径上局部修补。

## 当前边界

`crates/dma` 当前只承诺 QEMU identity mapping：

- coherent allocation：分配连续帧、清零、用 tracker 持有生命周期。
- streaming mapping：把已有 buffer 的 VA 转成设备可见 PA。
- 不承诺真机 non-coherent DMA 的 cache clean / invalidate。
- 不承诺 coherent DMA RAM 的专用 PTE/cache 属性。
- 不承诺完整 DMA mask、IOMMU、bounce buffer 或设备 capability 策略。

因此，当前代码在 QEMU 中通过并不等价于真机 DMA 语义正确。

## 问题清单

| ID | 严重度 | 问题 | 当前状态 | 与 rdrive 的关系 |
|----|--------|------|----------|------------------|
| D1 | P1 | streaming DMA `share/unshare` 无 cache 同步 | 待设计 | rdrive 数据 buffer 若走普通 RAM，同样需要同步或 bounce |
| D2 | P1 | coherent DMA 仍是普通 cacheable RAM | 待设计 | rdrive descriptor/ring/queue 需要明确共享内存属性 |
| D3 | P1 | DMA mask / capability / IOMMU / bounce 策略未闭环 | 待设计 | rdrive 若有地址位宽限制或 IOMMU 需求，必须先建模 |
| D4 | P1 | RISC-V `kernel_device()` 依赖 PMA/Svpbmt，当前等同 `kernel_rw()` | 待设计 | RISC-V rdrive/MMIO 路径不能假设页表已提供 non-cacheable |

## D1：streaming DMA 同步缺失

### 现象

路径：

```text
VirtIOBlk::read_blocks/write_blocks
  -> SimpleKernelHal::share(...)
  -> dma::raw_map_single(...)
  -> device DMA
  -> SimpleKernelHal::unshare(...)
  -> dma::raw_unmap_single(...)
```

当前实现中：

- `raw_map_single()` 只做 VA -> PA。
- `raw_unmap_single()` 是空实现。
- `DmaDirection` 被传入后端，但 QEMU raw 后端不使用它做 cache maintenance。

### 后果

在 non-coherent 真机上：

- `DriverToDevice`：CPU 写入仍可能停留在 cache 中，设备读到旧内存。
- `DeviceToDriver`：设备写回内存后，CPU 可能继续读旧 cache line。
- `Bidirectional`：两个方向都可能发生可见性错误。

当前受影响的显式路径包括：

- `src/device/virtio.rs` 的 sector 0 读测试 buffer。
- `src/fs/fatfs_adapter.rs` 的 `sector_buf` read/write 路径。

### 待决方案

| 方案 | 优点 | 代价 |
|------|------|------|
| 在 `share()` / `unshare()` 做 clean / invalidate | 接近常见 DMA API，普通 buffer 可继续使用 | 需要架构 cache helper，RISC-V 平台差异要建模 |
| bounce buffer | 不依赖调用方 buffer cache 属性或物理连续性 | 有拷贝成本，需要管理 bounce pool |
| 只允许 DMA allocator buffer 进入设备 | 语义集中 | 上层 API 使用成本高，VirtIO/rdrive 都要适配 |
| 继续声明仅 QEMU 支持 | 改动最小 | 阻塞真机声明 |

## D2：coherent DMA 内存属性缺失

### 现象

`dma_alloc()` 路径：

```text
SimpleKernelHal::dma_alloc(...)
  -> dma::raw_alloc_pages(...)
  -> QemuIdentityDmaOp::alloc_coherent(...)
  -> AllocatedFrames::alloc(...)
```

当前 `alloc_coherent()` 没有改变 PTE 属性。分配出的 virtqueue descriptor、available ring
和 used ring 仍处在普通 RAM 的背景映射中。

### 后果

在 AArch64 non-coherent 设备上，普通 `kernel_rw` 是 Normal Write-Back cacheable：

- CPU 更新 descriptor，设备可能读不到。
- 设备更新 used ring，CPU 可能读不到。
- 多核 CPU 一致性不代表设备也一致。

### 待决方案

| 方案 | 优点 | 代价 |
|------|------|------|
| 新增 `kernel_dma_coherent()` PTE preset | coherent buffer 语义集中 | 需要 AArch64 MAIR Normal-NC；RISC-V 依赖 PMA/Svpbmt |
| 保持 cacheable，ring 操作显式同步 | 性能可控 | 同步点复杂，容易漏 |
| 独立 uncached/bounce DMA pool | 简单可靠 | 性能和内存占用增加 |
| QEMU backend 保持不变，新增 real-device backend | 边界清晰 | 后端抽象和设备选择逻辑需要设计 |

## D3：DMA 能力模型未闭环

### 现象

当前 typed `DmaDevice` 已能保存 `dma_mask`，错误类型也有 `DmaMaskNotMatch`；但 VirtIO raw
路径仍默认使用 `u64::MAX`，QEMU raw 后端也没有按设备 mask 做真实约束。

### 后果

真机设备可能只能访问一部分物理地址：

- 32-bit DMA 设备无法访问高于 4 GiB 的内存。
- 没有 IOMMU 时，设备不能靠映射表访问任意物理页。
- 地址截断或越界 DMA 可能导致 I/O 失败或写坏内存。

### 待决问题

- 设备模型是否需要显式记录 `dma_mask`、coherency、IOMMU domain、bounce 需求。
- frame allocator 是否要提供低地址 DMA pool。
- streaming DMA 和 coherent DMA 是否共用同一 mask/bounce 策略。
- rdrive 是否有固定 DMA 地址窗口、队列内存要求或 IOMMU 前提。

## D4：RISC-V MMIO/cache 属性边界

### 现象

RISC-V `PteFlags::kernel_device()` 当前等同 `kernel_rw()`。代码注释已说明：RISC-V 基础页表
没有通用页表级缓存属性控制，设备内存属性依赖 PMA 或 Svpbmt。

### 后果

如果目标平台没有通过 PMA 或 Svpbmt 保证 MMIO 为 non-cacheable / IO 属性：

- MMIO 寄存器读可能返回旧值。
- MMIO 写可能被缓存或重排。
- rdrive 的 MMIO doorbell、状态寄存器、队列寄存器访问可能出现不可诊断的时序问题。

### 待决方案

| 方案 | 优点 | 代价 |
|------|------|------|
| 无 Svpbmt 时只声明平台 PMA 保证 | 短期简单 | 需要板级文档和启动验证 |
| 支持 Svpbmt PBMT NC/IO 位 | 语义清晰 | 需要检测扩展、编码 PTE、补 TLB/cache 文档 |
| 对未知平台 fail-fast | 避免静默错误 | 会限制可运行平台 |

## rdrive 集成前检查清单

在把这些问题与 rdrive 集成合并评估前，至少需要补齐以下信息：

- rdrive 使用 MMIO、PCIe、platform bus 还是其他枚举方式。
- rdrive descriptor/ring/queue 是否由 CPU 和设备共享。
- rdrive data buffer 是设备读、设备写，还是双向。
- 设备可寻址 DMA 位宽和地址窗口。
- 目标平台是否 coherent；若 non-coherent，cache maintenance 由谁负责。
- 是否存在 IOMMU，是否需要建立 IOVA。
- RISC-V 目标是否支持 Svpbmt；若不支持，PMA 如何保证 MMIO/DMA 属性。
- 设备中断、doorbell 和队列提交路径是否需要额外 memory barrier。

## 后续交付物

- ADR：真机 DMA cache maintenance / bounce buffer 策略。
- ADR：coherent DMA RAM 的 PTE/cache 属性。
- 设备模型补充：DMA mask、coherency、IOMMU/bounce capability。
- RISC-V 设备内存属性说明：PMA/Svpbmt 支持矩阵。
- 测试：
  - QEMU read/write、multi-sector、跨页 buffer 回归。
  - mask failure / bounce path 单元或系统测试。
  - coherent DMA dealloc mismatch、zero pages、地址不匹配回归。
  - rdrive 集成后增加 queue/ring/data buffer 可见性验证。

## 当前状态

当前保持 ADR-014 的边界：QEMU identity backend 可用于现有 VirtIO 路径，但不声明
non-coherent 真机 DMA 正确性。后续 rdrive 集成评估应以本文作为设备/DMA 问题入口。
