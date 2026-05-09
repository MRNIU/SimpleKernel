// Copyright The SimpleKernel Contributors

//! DMA 抽象层。
//!
//! 本 crate 是 SimpleKernel 对 `dma-api` 的依赖边界，避免 `dma_api::*`
//! 类型直接扩散到设备层和未来驱动层。
//!
//! 当前已提供 `DmaDevice` / `DmaBuffer` / `DmaArray` / `StreamingMapping`
//! 等 typed wrapper；后端仍是 QEMU VirtIO identity mapping raw backend。
//!
//! 真机 non-coherent DMA 的 cache/PTE 语义不属于当前提交承诺。

#![no_std]

pub mod device;
pub mod direction;
pub mod error;
mod qemu;

pub use device::{DmaArray, DmaBuffer, DmaDevice, DmaValue, StreamingMapping};
pub use direction::DmaDirection;
pub use error::{DmaError, DmaResult};
pub use qemu::{raw_alloc_pages, raw_dealloc_pages, raw_map_single, raw_unmap_single};
