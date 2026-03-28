//! 架构无关的多级页表。
//!
//! 页表 walk / map / unmap 逻辑（`PageTable<F>` 泛型）。
//! PTE 编解码由 [`page_table_entry`] crate 提供。
//!
//! `PageTable` 对帧分配的依赖通过 [`NodeFrameOps`] trait 抽象——
//! 消费方提供具体实现（裸机用物理帧分配器，测试用堆分配）。

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use address::PhysAddr;
use core::sync::atomic::{AtomicU64, Ordering};

pub mod error;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

/// PTE 大小的位移量——`log2(sizeof(u64))` = 3。
///
/// 两种架构的 PTE 均为 64 位，此常量在所有架构下一致。
pub const PTE_SIZE_SHIFT: usize = core::mem::size_of::<u64>().trailing_zeros() as usize;

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

// SAFETY: HeapNodeFrame 独占其分配的内存（*mut u8 阻止了 auto-Send），
// 可安全跨线程传递。
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
        if ptr.is_null() {
            return Err(error::PageTableError::AllocationFailed);
        }
        Ok(Self { ptr, layout })
    }
    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
    }
}

/// 每张页表中的条目数（PAGE_SIZE / sizeof(PTE)）。
///
/// 64 位架构中 PTE 均为 8 字节，4KB 页对应 512 条目。
pub const ENTRIES_PER_TABLE: usize = config::PAGE_SIZE / core::mem::size_of::<u64>();

/// 单级索引位宽（log2(ENTRIES_PER_TABLE)）。
pub(crate) const INDEX_BITS: usize = config::PAGE_SIZE_BITS - PTE_SIZE_SHIFT;

/// 层级参数。
#[derive(Clone, Copy)]
pub struct LevelInfo {
    /// 该级 VPN 在虚拟地址中的起始位位置
    pub shift: usize,
    /// 索引掩码
    pub index_mask: usize,
}

/// 最大页表层级数（Sv57 五级）。
const MAX_LEVELS: usize = 5;

/// 编译期计算各级层级参数。
const fn compute_level_info() -> [LevelInfo; MAX_LEVELS] {
    let mask = ENTRIES_PER_TABLE - 1;
    let mut info = [LevelInfo {
        shift: 0,
        index_mask: mask,
    }; MAX_LEVELS];
    info[0].shift = config::PAGE_SIZE_BITS;
    let mut i = 1;
    while i < MAX_LEVELS {
        info[i].shift = info[i - 1].shift + INDEX_BITS;
        i += 1;
    }
    info
}

pub const LEVEL_INFO: [LevelInfo; MAX_LEVELS] = compute_level_info();

/// 页表节点——封装 PTE 数组的原子访问。
///
/// 使用 `AtomicU64` 保证 SMP 下单个 PTE 读写不会 torn read/write。
/// 外层 `SpinLock` 负责更高层的互斥，此处仅保证单次访问的原子性。
pub(crate) struct Table {
    base: *mut AtomicU64,
}

impl Table {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: address::PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut AtomicU64,
        }
    }

    #[inline]
    pub(crate) fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查。
        // Relaxed 即可——外层 SpinLock 提供必要的 memory barrier。
        let val = unsafe { (*self.base.add(index)).load(Ordering::Relaxed) };
        PageTableEntry::from_raw(val)
    }

    #[inline]
    pub(crate) fn write(&mut self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Relaxed) };
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
