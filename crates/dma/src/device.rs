// Copyright The SimpleKernel Contributors

//! Typed DMA 容器封装。

use core::ptr::NonNull;

use crate::{DmaDirection, DmaError, DmaResult};

/// 可安全放入 typed DMA buffer 的 Plain Old Data 类型。
///
/// `FromBytes` 保证任意设备写入位模式可构造为 `T`，`IntoBytes` 保证 CPU 写入
/// 可以按字节交给设备读取，`Copy` 避免从 DMA 内存读取时产生所有权搬移语义。
///
/// 面向设备的 descriptor 类型仍应使用 `#[repr(C)]` 和固定宽度整数字段；
/// `DmaValue` 不编码设备 ABI 或 endian 语义。
pub trait DmaValue: zerocopy::FromBytes + zerocopy::IntoBytes + Copy {}

impl<T> DmaValue for T where T: zerocopy::FromBytes + zerocopy::IntoBytes + Copy {}

/// 设备 DMA 入口。
#[derive(Clone)]
pub struct DmaDevice {
    inner: dma_api::DeviceDma,
}

impl DmaDevice {
    /// 创建 QEMU identity-mapped DMA 设备入口。
    pub fn qemu_identity(dma_mask: u64) -> Self {
        Self {
            inner: dma_api::DeviceDma::new(dma_mask, &crate::qemu::QEMU_IDENTITY_DMA_OP),
        }
    }

    /// 返回设备 DMA mask。
    pub fn dma_mask(&self) -> u64 {
        self.inner.dma_mask()
    }

    /// 分配单个 typed coherent DMA buffer。
    pub fn buffer_zeroed<T: DmaValue>(
        &self,
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<DmaBuffer<T>> {
        self.inner
            .box_zero_with_align::<T>(align, direction.into())
            .map(DmaBuffer)
            .map_err(DmaError::from_api)
    }

    /// 分配固定长度 typed coherent DMA array。
    pub fn array_zeroed<T: DmaValue>(
        &self,
        len: usize,
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<DmaArray<T>> {
        self.inner
            .array_zero_with_align::<T>(len, align, direction.into())
            .map(DmaArray)
            .map_err(DmaError::from_api)
    }

    /// 映射已有 slice 为 streaming DMA 区域。
    ///
    /// # Safety
    ///
    /// `buffer` 必须在返回的 mapping 生命周期内保持有效；调用方必须按目标 DMA
    /// 方向维护别名和同步规则，避免 CPU 与设备并发访问同一内存时破坏一致性。
    pub unsafe fn map_slice<T: DmaValue>(
        &self,
        buffer: &[T],
        align: usize,
        direction: DmaDirection,
    ) -> DmaResult<StreamingMapping<T>> {
        self.inner
            .map_single_array::<T>(buffer, align, direction.into())
            .map(StreamingMapping)
            .map_err(DmaError::from_api)
    }
}

/// 单个 typed coherent DMA buffer。
pub struct DmaBuffer<T: DmaValue>(dma_api::DBox<T>);

impl<T: DmaValue> DmaBuffer<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回 CPU 可访问指针。
    ///
    /// # Safety
    ///
    /// 返回的指针仅在 `self` 存活期间有效；解引用会绕过 wrapper 的 cache sync
    /// 语义，调用方必须确保不会违反设备/CPU 别名或同步规则。
    pub unsafe fn as_ptr(&self) -> NonNull<T> {
        self.0.as_ptr()
    }

    /// 读取值。
    pub fn read(&self) -> T {
        self.0.read()
    }

    /// 写入值。
    pub fn write(&mut self, value: T) {
        self.0.write(value);
    }

    /// 读改写值。
    pub fn modify(&mut self, f: impl FnOnce(&mut T)) {
        self.0.modify(f);
    }
}

/// typed coherent DMA array。
pub struct DmaArray<T: DmaValue>(dma_api::DArray<T>);

impl<T: DmaValue> DmaArray<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回元素数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 判断数组是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 读取一个元素。
    pub fn read(&self, index: usize) -> Option<T> {
        self.0.read(index)
    }

    /// 写入一个元素。
    pub fn set(&mut self, index: usize, value: T) {
        let len = self.len();
        if index >= len {
            panic!("DMA 数组写入越界: index={index}, len={len}");
        }
        self.0.set(index, value);
    }

    /// 从 slice 复制数据到 DMA 数组。
    pub fn copy_from_slice(&mut self, source: &[T]) {
        self.0.copy_from_slice(source);
    }

    /// 在 CPU 读取前同步整个数组。
    pub fn prepare_read_all(&self) {
        self.0.prepare_read_all();
    }

    /// 在设备读取前同步整个数组。
    pub fn confirm_write_all(&self) {
        self.0.confirm_write_all();
    }
}

/// streaming DMA mapping。
pub struct StreamingMapping<T: DmaValue>(dma_api::SArrayPtr<T>);

impl<T: DmaValue> StreamingMapping<T> {
    /// 返回设备可见 DMA 地址。
    pub fn dma_addr(&self) -> u64 {
        self.0.dma_addr().as_u64()
    }

    /// 返回元素数量。
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 判断映射是否为空。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 读取一个元素，并按方向执行读取前同步。
    pub fn read(&self, index: usize) -> Option<T> {
        self.0.read(index)
    }

    /// 写入一个元素，并按方向执行写入后同步。
    pub fn set(&mut self, index: usize, value: T) {
        let len = self.len();
        if index >= len {
            panic!("DMA streaming 映射写入越界: index={index}, len={len}");
        }
        self.0.set(index, value);
    }

    /// 手动确认 CPU 写入对设备可见。
    pub fn copy_from_slice(&mut self, source: &[T]) {
        self.0.copy_from_slice(source);
    }

    /// 手动准备 CPU 读取设备写入。
    pub fn prepare_read_all(&self) {
        self.0.prepare_read_all();
    }

    /// 手动确认整个映射对设备可见。
    pub fn confirm_write_all(&self) {
        self.0.confirm_write_all();
    }
}
