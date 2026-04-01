//! 分页子系统——页表 + 仿射类型映射所有权。
//!
//! 本 crate 合并了原 `page_table` 和 `mapped_pages` 两个 crate，
//! 实现了 **编译期强制的仿射类型安全**：
//!
//! - [`PageTable`] 的写操作（`map_page`、`unmap_page` 等）为 `pub(crate)`，
//!   外部 crate 无法直接调用。
//! - 只有同 crate 内的 `MappedPages` / `MmioRegion`（后续 Task 添加）
//!   才能调用这些方法，确保映射的创建与销毁始终通过 RAII 类型管理。
//!
//! PTE 编解码由 [`page_table_entry`] crate 提供。
//! 帧分配通过 [`NodeFrame`] 类型别名选择具体实现（`cfg` 区分裸机与测试），
//! 页表和映射类型均为非泛型。

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

#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mapping;
#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub use mapping::{MappedPages, PermanentMapping};
/// 创建测试用 `Arc<SpinLock<PageTable>>`。
#[cfg(any(test, feature = "test-support"))]
pub fn test_pt() -> alloc::sync::Arc<sync_crate::SpinLock<PageTable>> {
    let pt = PageTable::create().expect("创建页表");
    alloc::sync::Arc::new(sync_crate::SpinLock::new(pt, "test_pt"))
}

#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mmio;

/// 页表节点帧的统一接口。
///
/// 裸机和测试各自提供一个实现，通过 [`NodeFrame`] 类型别名选择。
/// 页表、映射等类型直接使用 `NodeFrame`，不再泛型化。
pub trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, error::PagingError>;
    /// 获取帧的物理地址（裸机 identity mapping）或堆地址（测试）。
    fn paddr(&self) -> PhysAddr;
    /// 从物理地址回收帧——仅在 PageTable::drop 中使用。
    ///
    /// # Safety
    ///
    /// `paddr` 必须是本 trait 的 `alloc()` 分配、尚未释放的帧，
    /// 且调用方保证无并发访问。
    unsafe fn reclaim(paddr: PhysAddr);
}

// ── 裸机：包装 AllocatedFrames ─────────────────────────────

/// 裸机页表节点帧——包装 `AllocatedFrames`。
///
/// Newtype 用于为外部类型 `AllocatedFrames` 实现本 crate 的 `NodeFrameOps`。
#[cfg(target_os = "none")]
pub struct KernelNodeFrame(frame_allocator::AllocatedFrames);

#[cfg(target_os = "none")]
impl NodeFrameOps for KernelNodeFrame {
    fn alloc() -> Result<Self, error::PagingError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|_| error::PagingError::AllocationFailed)
    }
    fn paddr(&self) -> PhysAddr {
        self.0.start_paddr()
    }
    unsafe fn reclaim(paddr: PhysAddr) {
        use address::{FrameRange, PhysPageNum};
        let ppn = PhysPageNum::from(paddr);
        let range = FrameRange::new(ppn, ppn + 1);
        // SAFETY: 调用方保证帧由 alloc() 分配且未释放，无并发访问。
        let _reclaimed = unsafe { frame_allocator::UnmappedFrames::from_range(range) };
    }
}

// ── 测试：堆分配模拟 ──────────────────────────────────────

/// 测试用页表节点帧——从堆分配，模拟物理帧。
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
    fn alloc() -> Result<Self, error::PagingError> {
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("HeapNodeFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err(error::PagingError::AllocationFailed);
        }
        Ok(Self { ptr, layout })
    }
    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
    }
    unsafe fn reclaim(paddr: PhysAddr) {
        let ptr = paddr.as_usize() as *mut u8;
        // layout 必须与 alloc() 中的 Layout::from_size_align(PAGE_SIZE, PAGE_SIZE) 一致
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("HeapNodeFrame::reclaim: invalid layout");
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配，调用方保证未释放
        unsafe { alloc::alloc::dealloc(ptr, layout) };
    }
}

// ── 类型选择 ──────────────────────────────────────────────

/// 当前编译目标使用的页表节点帧类型。
///
/// - 裸机（`target_os = "none"`）：[`KernelNodeFrame`]（物理帧分配器）
/// - 测试 / `test-support`：[`HeapNodeFrame`]（堆分配模拟）
#[cfg(target_os = "none")]
pub type NodeFrame = KernelNodeFrame;
/// 当前编译目标使用的页表节点帧类型（测试）。
#[cfg(any(test, feature = "test-support"))]
pub type NodeFrame = HeapNodeFrame;

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

    /// Acquire load——无锁 walk 路径。
    #[inline]
    pub(crate) fn read_acquire(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let val = unsafe { (*self.base.add(index)).load(Ordering::Acquire) };
        PageTableEntry::from_raw(val)
    }

    /// AcqRel swap——无锁 unmap（原子清零叶 PTE）。
    #[inline]
    pub(crate) fn swap(&self, index: usize, val: u64) -> u64 {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).swap(val, Ordering::AcqRel) }
    }

    /// AcqRel CAS——无锁 map（安装中间节点或叶 PTE）。
    ///
    /// 返回 `Result`：`Ok(old)` CAS 成功，`Err(actual)` CAS 失败。
    /// 与 `std::sync::atomic::AtomicU64::compare_exchange` 语义一致。
    #[inline]
    pub(crate) fn compare_exchange(
        &self,
        index: usize,
        expected: u64,
        new: u64,
    ) -> Result<u64, u64> {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe {
            (*self.base.add(index)).compare_exchange(
                expected,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use address::VirtAddr;

    /// 验证 Level 0 的参数从 PAGE_SIZE 正确推导。
    #[test]
    fn level0_params_match_page_size() {
        assert_eq!(LEVEL_INFO[0].shift, config::PAGE_SIZE_BITS);
        assert_eq!(LEVEL_INFO[0].index_mask, ENTRIES_PER_TABLE - 1);
    }

    /// 验证各级 shift 链式递增，步长为 INDEX_BITS。
    #[test]
    fn level_shifts_are_chained() {
        for i in 1..LEVEL_INFO.len() {
            assert_eq!(
                LEVEL_INFO[i].shift,
                LEVEL_INFO[i - 1].shift + INDEX_BITS,
                "level {} shift 不正确",
                i
            );
        }
    }

    /// 验证 page_size_at_level 返回正确的页大小。
    #[test]
    fn page_size_at_level_values() {
        assert_eq!(page_size_at_level(0), config::PAGE_SIZE);
        assert_eq!(page_size_at_level(1), ENTRIES_PER_TABLE * config::PAGE_SIZE);
        assert_eq!(
            page_size_at_level(2),
            ENTRIES_PER_TABLE * ENTRIES_PER_TABLE * config::PAGE_SIZE
        );
    }

    /// vpn_index 应正确提取各级索引。
    #[test]
    fn vpn_index_extracts_correct_bits() {
        let va = VirtAddr::new(0x1000);
        assert_eq!(vpn_index(va, 0), 1);
        assert_eq!(vpn_index(va, 1), 0);
        assert_eq!(vpn_index(va, 2), 0);

        let va_high = VirtAddr::new(0x4000_0000);
        assert_eq!(vpn_index(va_high, 0), 0);
        assert_eq!(vpn_index(va_high, 1), 0);
        assert_eq!(vpn_index(va_high, 2), 1);
    }
}
