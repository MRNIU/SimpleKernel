//! 架构无关的多级页表。
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - `PteFlags` / `PageTableEntry`：各架构在 `pte_*.rs` 中定义硬件原生标志位
//! - `table.rs`：页表 walk / map / unmap 逻辑（`PageTable<F>` 泛型）
//!
//! `PageTable` 对帧分配的依赖通过 [`NodeFrameOps`] trait 抽象——
//! 消费方提供具体实现（裸机用物理帧分配器，测试用堆分配）。

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use address::PhysAddr;

pub mod error;

#[cfg(any(target_arch = "aarch64", feature = "test-aarch64"))]
mod pte_aarch64;
#[cfg(not(any(target_arch = "aarch64", feature = "test-aarch64")))]
mod pte_riscv64;

#[cfg(any(target_arch = "aarch64", feature = "test-aarch64"))]
pub use pte_aarch64::PteFlags;
#[cfg(not(any(target_arch = "aarch64", feature = "test-aarch64")))]
pub use pte_riscv64::PteFlags;

pub mod table;
pub use table::PageTable;

#[cfg(test)]
mod tests;

/// 页表节点帧的统一接口。
///
/// 消费方通过实现此 trait 向页表注入帧分配能力：
/// - 裸机：`impl NodeFrameOps for AllocatedFrames`（在 `memory` crate 中）
/// - 测试：`impl NodeFrameOps for HeapNodeFrame`（本 crate 内置）
pub trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, error::PageTableError>;
    /// 获取帧的物理地址（裸机 identity mapping）或堆地址（测试）。
    fn paddr(&self) -> PhysAddr;
}

/// 测试用页表节点帧——从堆分配，模拟物理帧。
///
/// 通过 `test-support` feature 或 `cfg(test)` 启用。
#[cfg(any(test, feature = "test-support"))]
pub struct HeapNodeFrame {
    ptr: *mut u8,
    layout: core::alloc::Layout,
}

// SAFETY: HeapNodeFrame 独占其分配的内存，可安全跨线程传递。
#[cfg(any(test, feature = "test-support"))]
unsafe impl Send for HeapNodeFrame {}

#[cfg(any(test, feature = "test-support"))]
impl Drop for HeapNodeFrame {
    fn drop(&mut self) {
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
        unsafe { alloc::alloc::dealloc(self.ptr, self.layout) };
    }
}

#[cfg(any(test, feature = "test-support"))]
impl NodeFrameOps for HeapNodeFrame {
    fn alloc() -> Result<Self, error::PageTableError> {
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("HeapNodeFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "HeapNodeFrame: allocation failed");
        Ok(Self { ptr, layout })
    }
    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
    }
}

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
    fn for_leaf_at_level(self, level: usize) -> Self;
    /// 是否设置了 EXCLUSIVE 软件位。
    fn is_exclusive(self) -> bool;
    /// 返回设置了 EXCLUSIVE 位的新标志。
    fn with_exclusive(self) -> Self;
}

/// 页表项的统一接口——各架构必须实现。
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
    /// 是否为叶节点。
    fn is_leaf(self, level: usize) -> bool;
    /// 空 PTE（全零）。
    fn empty() -> Self;
    /// 中间节点 PTE（指向下一级页表）。
    fn new_intermediate(paddr: PhysAddr) -> Self;
}

/// 单个硬件页表项（64 位）。
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub(crate) u64);

/// 页表层级标记——最多五级（Level4 = 根，Level0 = 叶）。
pub struct Level4;
pub struct Level3;
pub struct Level2;
pub struct Level1;
pub struct Level0;

/// 页表层级 trait——提供每级的结构参数。
pub trait PageLevel {
    /// 该级 VPN 在虚拟地址中的起始位位置
    const SHIFT: usize;
    /// 该级索引的位宽
    const INDEX_BITS: usize;
    /// 该级的 entries 数量
    const ENTRIES: usize = 1 << Self::INDEX_BITS;
    /// 索引掩码
    const INDEX_MASK: usize = Self::ENTRIES - 1;
}

impl PageLevel for Level0 {
    const SHIFT: usize = config::PAGE_SIZE.trailing_zeros() as usize;
    const INDEX_BITS: usize = Self::SHIFT - 3;
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

/// 运行时层级参数表。
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

/// 页表节点——封装 PTE 数组的裸指针访问。
pub(crate) struct Table<L: PageLevel> {
    base: *mut PageTableEntry,
    _level: core::marker::PhantomData<L>,
}

impl<L: PageLevel> Table<L> {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: address::PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut PageTableEntry,
            _level: core::marker::PhantomData,
        }
    }

    #[inline]
    pub(crate) fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index).read() }
    }

    #[inline]
    pub(crate) fn write(&mut self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index).write(pte) }
    }

    #[inline]
    pub(crate) fn entry_ptr(&mut self, index: usize) -> *mut PageTableEntry {
        debug_assert!(index < L::ENTRIES, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { self.base.add(index) }
    }
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
#[inline]
pub(crate) fn vpn_index(va: address::VirtAddr, level: usize) -> usize {
    let info = &LEVEL_INFO[level];
    (va.as_usize() >> info.shift) & info.index_mask
}

/// 返回第 `level` 级映射的页大小（字节）。
#[inline]
pub const fn page_size_at_level(level: usize) -> usize {
    1usize << LEVEL_INFO[level].shift
}

/// 编译期断言：当前架构的 PageTableEntry 实现了 PteOps。
fn _assert_trait_impl() {
    fn _assert<T: PteOps>() {}
    _assert::<PageTableEntry>();
}
