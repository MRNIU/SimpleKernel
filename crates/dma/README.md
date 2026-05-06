# dma

`crates/dma` 是 SimpleKernel 的 DMA 抽象边界。它集中依赖 `dma-api`，避免
`dma_api::*` 类型直接扩散到设备层和未来驱动层。

本初始 crate 只建立边界和计划中的依赖表面，尚未实现运行时 DMA API 或后端。
后续任务会在本 crate 内暴露：

- `DmaDevice`
- `DmaBuffer`
- `DmaArray`
- `StreamingMapping`

计划中的第一阶段后端是 QEMU VirtIO + SAS identity mapping。届时 coherent
allocation、streaming mapping、cache clean / invalidate 的语义会在实现中明确；
当前提交不声明这些能力已经存在。

## 依赖影响

`dma-api 0.7.2` 会引入自身的 cache/barrier 支持 crate，包括 AArch64 支持依赖。
这些第三方类型被刻意限制在 `crates/dma` 边界内，后续如果替换 DMA 抽象或调整
cache maintenance 策略，应只影响本 crate 的实现和对外封装。

后续真机 non-coherent DMA 支持需要单独设计 cache maintenance、PTE 属性和设备
DMA capability。
