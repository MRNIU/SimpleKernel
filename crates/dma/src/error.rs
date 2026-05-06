//! DMA 错误类型。

use core::{alloc::LayoutError, fmt};

/// DMA 操作结果。
pub type DmaResult<T> = Result<T, DmaError>;

/// SimpleKernel DMA 错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DmaError {
    /// DMA 内存不足。
    NoMemory,
    /// DMA 内存布局无效。
    InvalidLayout,
    /// DMA 地址不满足设备 mask。
    DmaMaskNotMatch { addr: u64, mask: u64 },
    /// DMA 地址不满足对齐要求。
    AlignMismatch { required: usize, address: u64 },
    /// DMA 指针为空。
    NullPointer,
    /// DMA buffer 大小为 0。
    ZeroSizedBuffer,
    /// 页数为 0，无法分配或释放 DMA 区域。
    ZeroPages,
    /// 释放不存在的 raw DMA 区域。
    UnknownRawRegion { paddr: u64 },
    /// 释放 raw DMA 区域时页数和分配记录不一致。
    RawRegionPageMismatch {
        paddr: u64,
        expected_pages: usize,
        actual_pages: usize,
    },
    /// 释放 raw DMA 区域时虚拟地址和分配记录不一致。
    RawRegionVirtualAddressMismatch {
        paddr: u64,
        expected_vaddr: usize,
        actual_vaddr: usize,
    },
    /// DMA 虚拟地址为空。
    NullVirtualAddress { paddr: u64 },
}

impl From<LayoutError> for DmaError {
    fn from(_error: LayoutError) -> Self {
        DmaError::InvalidLayout
    }
}

impl DmaError {
    pub(crate) fn from_api(error: dma_api::DmaError) -> Self {
        match error {
            dma_api::DmaError::NoMemory => DmaError::NoMemory,
            dma_api::DmaError::LayoutError(_error) => DmaError::InvalidLayout,
            dma_api::DmaError::DmaMaskNotMatch { addr, mask } => DmaError::DmaMaskNotMatch {
                addr: addr.as_u64(),
                mask,
            },
            dma_api::DmaError::AlignMismatch { required, address } => DmaError::AlignMismatch {
                required,
                address: address.as_u64(),
            },
            dma_api::DmaError::NullPointer => DmaError::NullPointer,
            dma_api::DmaError::ZeroSizedBuffer => DmaError::ZeroSizedBuffer,
        }
    }
}

impl fmt::Display for DmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DmaError::NoMemory => write!(f, "DMA 内存不足"),
            DmaError::InvalidLayout => write!(f, "DMA 内存布局无效"),
            DmaError::DmaMaskNotMatch { addr, mask } => {
                write!(f, "DMA 地址不满足设备 mask: addr={addr:#x}, mask={mask:#x}")
            }
            DmaError::AlignMismatch { required, address } => write!(
                f,
                "DMA 地址不满足对齐要求: required={required}, address={address:#x}",
            ),
            DmaError::NullPointer => write!(f, "DMA 指针为空"),
            DmaError::ZeroSizedBuffer => write!(f, "DMA buffer 大小为 0"),
            DmaError::ZeroPages => write!(f, "DMA 页数不能为 0"),
            DmaError::UnknownRawRegion { paddr } => {
                write!(f, "未找到 raw DMA 区域: paddr={paddr:#x}")
            }
            DmaError::RawRegionPageMismatch {
                paddr,
                expected_pages,
                actual_pages,
            } => write!(
                f,
                "raw DMA 区域页数不匹配: paddr={paddr:#x}, expected={expected_pages}, actual={actual_pages}",
            ),
            DmaError::RawRegionVirtualAddressMismatch {
                paddr,
                expected_vaddr,
                actual_vaddr,
            } => write!(
                f,
                "raw DMA 区域虚拟地址不匹配: paddr={paddr:#x}, expected_vaddr={expected_vaddr:#x}, actual_vaddr={actual_vaddr:#x}",
            ),
            DmaError::NullVirtualAddress { paddr } => {
                write!(f, "DMA 虚拟地址为空: paddr={paddr:#x}")
            }
        }
    }
}

impl core::error::Error for DmaError {}
