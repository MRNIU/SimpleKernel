//! mapped_pages 错误定义。

use core::fmt;

/// 映射操作错误类型——包装底层错误，不做有损转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedPagesError {
    /// 页表操作错误
    PageTable(page_table::error::PageTableError),
    /// 帧分配错误
    FrameAlloc(frame_allocator::FrameAllocError),
}

impl fmt::Display for MappedPagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PageTable(e) => write!(f, "page table: {e:?}"),
            Self::FrameAlloc(e) => write!(f, "frame alloc: {e:?}"),
        }
    }
}

impl core::error::Error for MappedPagesError {}

impl From<page_table::error::PageTableError> for MappedPagesError {
    fn from(e: page_table::error::PageTableError) -> Self {
        Self::PageTable(e)
    }
}

impl From<frame_allocator::FrameAllocError> for MappedPagesError {
    fn from(e: frame_allocator::FrameAllocError) -> Self {
        Self::FrameAlloc(e)
    }
}
