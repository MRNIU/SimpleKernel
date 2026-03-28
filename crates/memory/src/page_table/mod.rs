//! 多级页表抽象。
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - `PteFlags` / `PageTableEntry`：各架构在 `pte_*.rs` 中定义硬件原生标志位
//! - `pte_*.rs`：各架构的 PTE 编码实现（Sv39 / ARMv8）
//! - `table.rs`：页表 walk / map / unmap 逻辑

#[cfg(any(test, target_os = "none"))]
use core::marker::PhantomData;

use address::PhysAddr;

#[cfg(all(not(test), target_arch = "aarch64"))]
mod pte_aarch64;
#[cfg(any(test, target_arch = "riscv64"))]
mod pte_riscv64;

#[cfg(all(not(test), target_arch = "aarch64"))]
pub use pte_aarch64::PteFlags;
#[cfg(any(test, target_arch = "riscv64"))]
pub use pte_riscv64::PteFlags;

#[cfg(any(test, target_os = "none"))]
pub(crate) mod table;
#[cfg(any(test, target_os = "none"))]
pub use table::PageTable;

#[cfg(test)]
mod tests;

/// 页表项标志位的统一接口——各架构必须实现。
///
/// 保证 RISC-V 和 AArch64 的 `PteFlags` 提供完全相同的方法集，
/// 避免新增 preset 时某一架构遗漏。
pub trait PteFlagsOps: Copy + core::fmt::Debug {
    /// 内核读写数据映射。
    fn kernel_rw() -> Self;
    /// 内核读-执行映射。
    fn kernel_rx() -> Self;
    /// 内核只读映射。
    fn kernel_ro() -> Self;
    /// 内核读写执行映射。
    fn kernel_rwx() -> Self;
    /// 设备 MMIO 映射（不可缓存、不可执行）。
    fn kernel_device() -> Self;
    /// 是否具有写权限。
    fn is_writable(self) -> bool;
    /// 将标志位适配为指定层级的叶描述符格式。
    ///
    /// 在 AArch64 上，Level 0 (page descriptor) 需要 TABLE 位，
    /// 而 Level > 0 (block descriptor) 不能设置 TABLE 位。
    /// RISC-V 无此区分，直接返回 self。
    fn for_leaf_at_level(self, level: usize) -> Self;

    /// 是否设置了 EXCLUSIVE 软件位。
    ///
    /// EXCLUSIVE 位标记该 PTE "拥有"其物理帧——unmap 时应归还帧分配器。
    /// 未设置 EXCLUSIVE 的 PTE（如 identity mapping）unmap 时不回收帧。
    fn is_exclusive(self) -> bool;

    /// 返回设置了 EXCLUSIVE 位的新标志。
    fn with_exclusive(self) -> Self;
}

/// 页表项的统一接口——各架构必须实现。
///
/// 保证 RISC-V 和 AArch64 的 `PageTableEntry` 提供完全相同的操作集。
/// 使用关联类型 `Flags` 引用对应架构的标志位类型，避免 cfg 循环依赖。
pub trait PteOps: Copy + core::fmt::Debug {
    /// 对应架构的标志位类型
    type Flags: PteFlagsOps;
    /// 从物理地址和标志构造叶 PTE。
    fn new(paddr: PhysAddr, flags: Self::Flags) -> Self;
    /// 从 PTE 提取物理地址。
    fn paddr(self) -> PhysAddr;
    /// 从 PTE 提取标志位。
    fn flags(self) -> Self::Flags;
    /// PTE 是否有效。
    fn is_valid(self) -> bool;
    /// 是否为叶节点（`level` 为该 PTE 所在的层级编号）。
    fn is_leaf(self, level: usize) -> bool;
    /// 空 PTE（全零）。
    fn empty() -> Self;
    /// 中间节点 PTE（指向下一级页表）。
    fn new_intermediate(paddr: PhysAddr) -> Self;
}

/// 单个硬件页表项（64 位）。
///
/// 各架构通过 [`PteOps`] trait 提供操作方法。
/// 标志位类型为 [`PteFlags`]（各架构通过 [`PteFlagsOps`] trait 提供）。
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
pub struct LevelInfo {
    /// 该级 VPN 在虚拟地址中的起始位位置
    pub shift: usize,
    /// 索引掩码
    pub index_mask: usize,
}

pub const LEVEL_INFO: [LevelInfo; 5] = [
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
#[cfg(any(test, target_os = "none"))]
pub(crate) struct Table<L: PageLevel> {
    base: *mut PageTableEntry,
    _level: PhantomData<L>,
}

#[cfg(any(test, target_os = "none"))]
impl<L: PageLevel> Table<L> {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    /// - 当前使用 identity mapping（VA == PA）
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: address::PhysAddr) -> Self {
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
#[cfg(any(test, target_os = "none"))]
pub(crate) fn vpn_index(va: address::VirtAddr, level: usize) -> usize {
    let info = &LEVEL_INFO[level];
    (va.as_usize() >> info.shift) & info.index_mask
}

/// 返回第 `level` 级映射的页大小（字节）。
///
/// - Level 0 = `PAGE_SIZE`（4KB）
/// - Level 1 = 2MB（Sv39 megapage / ARMv8 block）
/// - Level 2 = 1GB（Sv39 gigapage / ARMv8 block）
#[inline]
pub const fn page_size_at_level(level: usize) -> usize {
    1usize << LEVEL_INFO[level].shift
}

/// 编译期断言：当前架构的 PageTableEntry 实现了 PteOps。
///
/// PteOps 的关联类型 `type Flags: PteFlagsOps` 同时保证标志位类型满足统一接口。
/// 如果某架构遗漏了 trait 实现，此处会产生编译错误。
#[cfg(any(test, target_os = "none"))]
fn _assert_trait_impl() {
    fn _assert<T: PteOps>() {}
    _assert::<PageTableEntry>();
}
