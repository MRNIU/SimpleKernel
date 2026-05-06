# dma

`crates/dma` 是 SimpleKernel 的 DMA 抽象边界。它集中依赖 `dma-api`，避免
`dma_api::*` 类型直接扩散到设备层和未来驱动层。

当前 crate 已提供 QEMU VirtIO + SAS identity mapping 的 raw DMA 后端和 helper，
用于页级 coherent allocation 与 streaming mapping 地址转换。下一步会在本 crate
内暴露类型化 wrapper：

- `DmaDevice`
- `DmaBuffer`
- `DmaArray`
- `StreamingMapping`

当前 raw 后端不声明真机 non-coherent DMA 能力。cache clean / invalidate、PTE
属性和设备 DMA capability 仍需在后续设计中明确。

## 依赖影响

`dma-api 0.7.2` 会引入自身的 cache/barrier 支持 crate，包括 AArch64 支持依赖。
这些第三方类型被刻意限制在 `crates/dma` 边界内，后续如果替换 DMA 抽象或调整
cache maintenance 策略，应只影响本 crate 的实现和对外封装。

后续真机 non-coherent DMA 支持需要单独设计 cache maintenance、PTE 属性和设备
DMA capability。
