// Copyright The SimpleKernel Contributors

//! DMA 方向类型。

/// DMA 传输方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DmaDirection {
    /// CPU 写入，设备读取。
    ToDevice,
    /// 设备写入，CPU 读取。
    FromDevice,
    /// CPU 和设备都可能读写。
    Bidirectional,
}

impl From<DmaDirection> for dma_api::DmaDirection {
    fn from(direction: DmaDirection) -> Self {
        match direction {
            DmaDirection::ToDevice => dma_api::DmaDirection::ToDevice,
            DmaDirection::FromDevice => dma_api::DmaDirection::FromDevice,
            DmaDirection::Bidirectional => dma_api::DmaDirection::Bidirectional,
        }
    }
}
