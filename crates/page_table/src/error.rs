//! 页表操作错误类型。

use core::fmt;

/// 页表操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableError {
    /// 帧分配失败（分配器未初始化或帧耗尽）
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
    /// walk 路径上遇到大页，无法继续向下遍历
    HugePageConflict,
    /// 目标虚拟页未映射
    PageNotMapped,
    /// 地址范围无效（start >= end）
    InvalidRange,
}

impl fmt::Display for PageTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for PageTableError {}
