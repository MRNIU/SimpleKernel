//! 多级页表抽象。
//!
//! - `PageFlags` / `PageTableEntry`：架构无关的类型定义（本文件）
//! - `pte_*.rs`：各架构的 PTE 编码实现（Sv39 / ARMv8）
//! - `table.rs`：页表 walk / map / unmap 逻辑

use bitflags::bitflags;

#[cfg(all(not(test), target_arch = "aarch64"))]
mod pte_aarch64;
#[cfg(all(not(test), target_arch = "riscv64"))]
mod pte_riscv64;
#[cfg(test)]
mod pte_test;

#[cfg(target_os = "none")]
mod table;
#[cfg(target_os = "none")]
pub use table::PageTable;

#[cfg(test)]
mod test_table;
#[cfg(test)]
mod tests;

bitflags! {
    /// 架构无关的页表项标志位。
    ///
    /// 位位置与 RISC-V Sv39 对齐。AArch64 的 `pte_aarch64.rs` 在构造 PTE 时
    /// 将这些标志翻译为 ARM 描述符位。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageFlags: u64 {
        const VALID    = 1 << 0;
        const READ     = 1 << 1;
        const WRITE    = 1 << 2;
        const EXECUTE  = 1 << 3;
        const USER     = 1 << 4;
        const GLOBAL   = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY    = 1 << 7;
    }
}

impl PageFlags {
    /// 内核读写数据映射 (V | R | W | G | A | D)。
    #[inline]
    pub fn kernel_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::GLOBAL | Self::ACCESSED | Self::DIRTY
    }

    /// 内核读-执行映射 (V | R | X | G | A)。
    #[inline]
    pub fn kernel_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核只读映射 (V | R | G | A)。
    #[inline]
    pub fn kernel_ro() -> Self {
        Self::VALID | Self::READ | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核读写执行映射 (V | R | W | X | G | A | D)。
    #[inline]
    pub fn kernel_rwx() -> Self {
        Self::VALID
            | Self::READ
            | Self::WRITE
            | Self::EXECUTE
            | Self::GLOBAL
            | Self::ACCESSED
            | Self::DIRTY
    }
}

/// 单个硬件页表项（64 位）。
///
/// 方法 `new`、`paddr`、`flags`、`is_valid`、`is_leaf`、`empty`、`new_intermediate`
/// 由各架构的 `pte_*.rs` 提供。
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub(crate) u64);
