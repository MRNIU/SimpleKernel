use core::fmt;

/// 内存子系统错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// 帧分配器未初始化时尝试分配
    AllocationFailed,
    /// 物理帧耗尽
    OutOfMemory,
    /// 页表映射失败（如重复映射）
    MapFailed,
    /// 目标虚拟页未映射
    PageNotMapped,
    /// 全局内核页表未初始化
    InvalidPageTable,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for MemoryError {}
