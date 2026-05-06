//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的依赖边界，避免 `dma_api::*`
//! 类型直接扩散到设备层和未来驱动层。
//!
//! 本提交只建立 crate 边界和依赖表面。后续任务会在这里暴露
//! `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping` 等内部 API，
//! 并加入 QEMU VirtIO identity mapping 后端。
//!
//! 真机 non-coherent DMA 的 cache/PTE 语义不属于当前提交承诺。

#![no_std]

pub mod direction;
pub mod error;

pub use direction::DmaDirection;
pub use error::{DmaError, DmaResult};
