# dma

`crates/dma` 是 SimpleKernel 的 DMA 抽象边界。它集中依赖 `dma-api`，避免
`dma_api::*` 类型直接扩散到设备层和未来驱动层。

当前 crate 已提供 QEMU VirtIO + SAS identity mapping 的 raw DMA 后端和 helper，
用于页级 coherent allocation 与 streaming mapping 地址转换。同时已经暴露
SimpleKernel 自己的 typed wrapper：

- `DmaDevice`：内部持有 `dma_api::DeviceDma`，作为设备 DMA 操作入口。
- `DmaBuffer<T>`：内部持有 `dma_api::DBox<T>`，表示单个 typed coherent DMA buffer。
- `DmaArray<T>`：内部持有 `dma_api::DArray<T>`，表示固定长度 typed coherent DMA array。
- `StreamingMapping<T>`：内部持有 `dma_api::SArrayPtr<T>`，表示已有 slice 的 streaming DMA mapping。

上层模块必须使用这些 wrapper，不要直接在 public API 或设备层实现中使用
`dma_api::*` 类型。`dma_api` 仍只允许作为本 crate 内部实现细节。

面向设备的 descriptor 类型应使用 `#[repr(C)]` 和固定宽度整数字段。`DmaValue`
只约束字节级可读写和复制语义，不表达设备 ABI 或 endian 约定。

当前 raw 后端不声明真机 non-coherent DMA 能力。cache clean / invalidate、PTE
属性和设备 DMA capability 仍需在后续设计中明确。

## 依赖影响

`dma-api 0.7.2` 会引入自身的 cache/barrier 支持 crate，包括 AArch64 支持依赖。
这些第三方类型被刻意限制在 `crates/dma` 边界内，后续如果替换 DMA 抽象或调整
cache maintenance 策略，应只影响本 crate 的实现和对外封装。

后续真机 non-coherent DMA 支持需要单独设计 cache maintenance、PTE 属性和设备
DMA capability。
