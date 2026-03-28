//! 多级页表抽象。
//!
//! - `PageFlags` / `PageTableEntry`：架构无关的类型定义（本文件）
//! - `pte_*.rs`：各架构的 PTE 编码实现（Sv39 / ARMv8）
//! - `table.rs`：页表 walk / map / unmap 逻辑

use bitflags::bitflags;
use core::marker::PhantomData;

#[cfg(all(not(test), target_arch = "aarch64"))]
mod pte_aarch64;
#[cfg(any(test, target_arch = "riscv64"))]
mod pte_riscv64;

#[cfg(any(test, target_os = "none"))]
mod table;
#[cfg(target_os = "none")]
pub use table::PageTable;

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

/// 页表层级标记——最多五级（Level4 = 根，Level0 = 叶）。
///
/// 参考 Linux `pgd → p4d → pud → pmd → pte` 五级模型。
/// 缺少的层级通过 `config::PT_LEVELS` 控制的 walker 循环范围自动折叠：
/// - Sv39（3 级）：walker 遍历 Level2 → Level1 → Level0
/// - Sv48 / ARMv8 4KB（4 级）：walker 遍历 Level3 → Level2 → Level1 → Level0
/// - Sv57（5 级，未来）：walker 遍历 Level4 → … → Level0
pub struct Level4; // PGD（最高级）
pub struct Level3; // P4D
pub struct Level2; // PUD
pub struct Level1; // PMD
pub struct Level0; // PTE（叶级）

/// 页表层级 trait——提供每级的结构参数。
///
/// 不同架构 / granule 可能有不同的每级位宽：
/// - 4KB granule（Sv39/Sv48/ARMv8 4K）：9 位索引，512 entries
/// - 16KB granule（ARMv8 16K）：11 位索引，2048 entries
/// - 64KB granule（ARMv8 64K）：13 位索引，8192 entries
pub trait PageLevel {
    /// 该级 VPN 在虚拟地址中的起始位位置
    const SHIFT: usize;
    /// 该级索引的位宽（决定每级的 entries 数量 = 1 << INDEX_BITS）
    const INDEX_BITS: usize;
    /// 该级的 entries 数量
    const ENTRIES: usize = 1 << Self::INDEX_BITS;
    /// 索引掩码
    const INDEX_MASK: usize = Self::ENTRIES - 1;
}

impl PageLevel for Level0 {
    const SHIFT: usize = config::PAGE_SIZE.trailing_zeros() as usize;
    const INDEX_BITS: usize = Self::SHIFT - 3; // 每个 PTE 8 字节 = 2^3
}
impl PageLevel for Level1 {
    const SHIFT: usize = Level0::SHIFT + Level0::INDEX_BITS;
    const INDEX_BITS: usize = Level0::INDEX_BITS;
}
impl PageLevel for Level2 {
    const SHIFT: usize = Level1::SHIFT + Level1::INDEX_BITS;
    const INDEX_BITS: usize = Level0::INDEX_BITS;
}
impl PageLevel for Level3 {
    const SHIFT: usize = Level2::SHIFT + Level2::INDEX_BITS;
    const INDEX_BITS: usize = Level0::INDEX_BITS;
}
impl PageLevel for Level4 {
    const SHIFT: usize = Level3::SHIFT + Level3::INDEX_BITS;
    const INDEX_BITS: usize = Level0::INDEX_BITS;
}

/// 运行时层级参数表——walker 循环通过此表查询每级的 SHIFT 和 INDEX_MASK。
///
/// 索引 0 = Level0, 1 = Level1, ..., 4 = Level4。
/// 避免在运行时循环中使用编译期类型参数（Rust 不支持）。
pub(crate) struct LevelInfo {
    pub shift: usize,
    pub index_mask: usize,
}

pub(crate) const LEVEL_INFO: [LevelInfo; 5] = [
    LevelInfo {
        shift: Level0::SHIFT,
        index_mask: Level0::INDEX_MASK,
    },
    LevelInfo {
        shift: Level1::SHIFT,
        index_mask: Level1::INDEX_MASK,
    },
    LevelInfo {
        shift: Level2::SHIFT,
        index_mask: Level2::INDEX_MASK,
    },
    LevelInfo {
        shift: Level3::SHIFT,
        index_mask: Level3::INDEX_MASK,
    },
    LevelInfo {
        shift: Level4::SHIFT,
        index_mask: Level4::INDEX_MASK,
    },
];

/// 页表节点——封装 PTE 数组的裸指针访问，消除 `&'static mut` aliasing UB。
///
/// 类型参数 `L` 标记层级，用于编译期区分不同层级的表（如大页支持时
/// 需要区分 Level1 block entry 和 Level0 page entry）。
pub(crate) struct Table<L: PageLevel> {
    base: *mut PageTableEntry,
    _level: PhantomData<L>,
}

impl<L: PageLevel> Table<L> {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    /// - 当前使用 identity mapping（VA == PA）
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: crate::address::PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut PageTableEntry,
            _level: PhantomData,
        }
    }

    /// 读取指定索引的 PTE（Copy 值）。
    #[inline]
    pub(crate) fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index).read() }
    }

    /// 写入指定索引的 PTE。
    #[inline]
    pub(crate) fn write(&mut self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index).write(pte) }
    }

    /// 返回指定索引 PTE 的裸指针。
    #[inline]
    pub(crate) fn entry_ptr(&mut self, index: usize) -> *mut PageTableEntry {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index) }
    }
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
///
/// 通过 [`LEVEL_INFO`] 查表获取每级的 SHIFT 和 INDEX_MASK，
/// 支持不同 granule 下不同的每级位宽。
#[inline]
pub(crate) fn vpn_index(va: crate::address::VirtAddr, level: usize) -> usize {
    let info = &LEVEL_INFO[level];
    (va.as_usize() >> info.shift) & info.index_mask
}
