//! 虚拟页分配器——管理虚拟地址空间的分配与释放。
//!
//! 与 [`frame_allocator`] 对称设计：
//! - `frame_allocator` 管理物理帧（PA 空间）
//! - `page_allocator` 管理虚拟页（VA 空间）
//!
//! SAS 架构下为全局单例。初始化时将整个可用虚拟地址空间标记为空闲，
//! 随后通过 `reserve` 扣除已被占用的区域（内核段、MMIO 等）。
//!
//! 分配产出 [`AllocatedPages`]（move-only），释放由 Drop 自动完成。

#![cfg_attr(not(any(test, feature = "test-support")), no_std)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

extern crate alloc;

use alloc::collections::BTreeMap;

use address::{PageRange, VirtAddr, VirtPageNum};
use sync_crate::SpinLockIrq;

mod error;
mod state;

pub use error::PageAllocError;
pub use state::{AllocatedPages, FreePages, MemoryState, Pages};

static PAGE_ALLOCATOR: SpinLockIrq<PageAllocatorInner> = SpinLockIrq::new_with_level(
    PageAllocatorInner::new(),
    "page_alloc",
    sync_crate::lock_level::PAGE_ALLOC,
);

struct PageAllocatorInner {
    free_ranges: BTreeMap<VirtPageNum, PageRange>,
    initialized: bool,
}

impl PageAllocatorInner {
    const fn new() -> Self {
        Self {
            free_ranges: BTreeMap::new(),
            initialized: false,
        }
    }
}

/// 初始化页分配器——将 `[start, start+size)` 虚拟地址范围标记为空闲。
///
/// `start` 必须页对齐。通常在内核启动时用整个可用虚拟地址空间调用，
/// 随后通过 [`reserve`] 扣除已被占用的区域。
///
/// # Safety
/// 仅调用一次。调用方必须确保该虚拟地址范围有效且不与硬件保留区域重叠。
pub unsafe fn init(start: VirtAddr, size: usize) {
    let mut alloc = PAGE_ALLOCATOR.lock();
    assert!(!alloc.initialized, "page_allocator::init called twice");
    assert!(
        start.is_aligned(),
        "page_allocator::init: start not page-aligned"
    );
    assert!(size > 0, "page_allocator::init: size is zero");

    let start_pn = start.page_number();
    let end_pn = VirtPageNum::new(start_pn.as_usize() + size / config::PAGE_SIZE);
    let range = PageRange::new(start_pn, end_pn);
    alloc.free_ranges.insert(start_pn, range);
    alloc.initialized = true;

    log::info!(
        "PageInit: {} MB virtual from {}",
        size / (1024 * 1024),
        start
    );
}

/// 从空闲池中扣除指定区域——用于标记已被占用的虚拟地址。
///
/// 启动时调用：内核代码段、内核栈、堆、MMIO 等已有映射都需要 reserve。
pub fn reserve(start: VirtAddr, size: usize) {
    if size == 0 {
        return;
    }
    let start_pn = VirtPageNum::from(start);
    let end_pn = VirtPageNum::new(start_pn.as_usize() + size / config::PAGE_SIZE);

    let mut alloc = PAGE_ALLOCATOR.lock();
    assert!(
        alloc.initialized,
        "page_allocator::reserve: not initialized"
    );

    let target = PageRange::new(start_pn, end_pn);
    remove_range_from_free(&mut alloc.free_ranges, target);
}

/// 分配 `count` 个连续虚拟页——自动选择地址。
pub(crate) fn alloc_pages(count: usize) -> Result<PageRange, PageAllocError> {
    let mut alloc = PAGE_ALLOCATOR.lock();
    if !alloc.initialized {
        return Err(PageAllocError::AllocationFailed);
    }

    let needed = count;
    let mut found_key = None;

    for (&start_pn, range) in &alloc.free_ranges {
        if range.size() >= needed {
            found_key = Some(start_pn);
            break;
        }
    }

    let start_pn = found_key.ok_or(PageAllocError::OutOfVirtualSpace)?;
    let range = alloc.free_ranges.remove(&start_pn).expect("刚查到的 range");

    let alloc_start = range.start();
    let alloc_end = alloc_start + needed;
    let allocated = PageRange::new(alloc_start, alloc_end);

    if range.size() > needed {
        let remaining = PageRange::new(alloc_end, range.end());
        alloc.free_ranges.insert(remaining.start(), remaining);
    }

    Ok(allocated)
}

/// 在指定虚拟地址分配 `count` 个连续页。
pub(crate) fn alloc_pages_at(
    start: VirtPageNum,
    count: usize,
) -> Result<PageRange, PageAllocError> {
    let mut alloc = PAGE_ALLOCATOR.lock();
    if !alloc.initialized {
        return Err(PageAllocError::AllocationFailed);
    }

    let end = start + count;
    let target = PageRange::new(start, end);

    let containing_key = alloc
        .free_ranges
        .range(..=start)
        .next_back()
        .filter(|(_, r)| r.end() >= end)
        .map(|(&k, _)| k)
        .ok_or(PageAllocError::AddressNotAvailable)?;

    let range = alloc
        .free_ranges
        .remove(&containing_key)
        .expect("刚查到的 range");

    if range.start() < start {
        let before = PageRange::new(range.start(), start);
        alloc.free_ranges.insert(before.start(), before);
    }
    if range.end() > end {
        let after = PageRange::new(end, range.end());
        alloc.free_ranges.insert(after.start(), after);
    }

    Ok(target)
}

/// 归还虚拟页到空闲池。
pub(crate) fn dealloc_pages(range: PageRange) {
    if range.size() == 0 {
        return;
    }
    let mut alloc = PAGE_ALLOCATOR.lock();

    let start = range.start();
    let end = range.end();

    let mut merge_start = start;
    let mut merge_end = end;

    if let Some((&prev_start, prev_range)) = alloc.free_ranges.range(..start).next_back() {
        if prev_range.end() == start {
            merge_start = prev_start;
            alloc.free_ranges.remove(&prev_start);
        }
    }

    if let Some((&next_start, _)) = alloc.free_ranges.range(end..).next() {
        if next_start == end {
            let next_range = alloc.free_ranges.remove(&next_start).expect("刚查到");
            merge_end = next_range.end();
        }
    }

    let merged = PageRange::new(merge_start, merge_end);
    alloc.free_ranges.insert(merge_start, merged);
}

/// 从 free_ranges 中移除一段区域（用于 reserve）。
fn remove_range_from_free(free: &mut BTreeMap<VirtPageNum, PageRange>, target: PageRange) {
    let containing_key = free
        .range(..=target.start())
        .next_back()
        .filter(|(_, r)| r.end() >= target.end())
        .map(|(&k, _)| k);

    if let Some(key) = containing_key {
        let range = free.remove(&key).expect("刚查到");

        if range.start() < target.start() {
            let before = PageRange::new(range.start(), target.start());
            free.insert(before.start(), before);
        }
        if range.end() > target.end() {
            let after = PageRange::new(target.end(), range.end());
            free.insert(after.start(), after);
        }
    }
}

/// 测试用页分配器初始化。
#[cfg(any(test, feature = "test-support"))]
pub fn ensure_test_init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let start = VirtAddr::new(0x1000_0000);
        let size = 256 * config::PAGE_SIZE;
        // SAFETY: 测试专用虚拟地址范围
        unsafe { init(start, size) };
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 基础分配和释放。
    #[test]
    fn alloc_and_dealloc() {
        ensure_test_init();
        let pages = AllocatedPages::alloc(4).expect("分配 4 页应成功");
        assert_eq!(pages.count(), 4);
        assert!(pages.start_vaddr().is_aligned());
    }

    /// 指定地址分配。
    #[test]
    fn alloc_at_specific_address() {
        ensure_test_init();
        let va = VirtAddr::new(0x1000_0000 + 128 * config::PAGE_SIZE);
        let pages = AllocatedPages::alloc_at(va, 2).expect("指定地址分配应成功");
        assert_eq!(pages.start_vaddr(), va);
        assert_eq!(pages.count(), 2);
    }

    /// 分配后释放再分配应成功。
    #[test]
    fn alloc_dealloc_realloc() {
        ensure_test_init();
        let pages = AllocatedPages::alloc(2).expect("首次分配");
        let va = pages.start_vaddr();
        drop(pages);
        let pages2 = AllocatedPages::alloc(2).expect("重新分配应成功");
        assert!(pages2.start_vaddr().is_aligned());
    }

    /// 指定地址不可用时应返回错误。
    #[test]
    fn alloc_at_occupied_fails() {
        ensure_test_init();
        let va = VirtAddr::new(0x1000_0000 + 200 * config::PAGE_SIZE);
        let _pages = AllocatedPages::alloc_at(va, 4).expect("首次分配");
        let result = AllocatedPages::alloc_at(va, 4);
        assert!(result.is_err());
    }

    /// reserve 后该区域不可分配。
    #[test]
    fn reserve_prevents_alloc() {
        ensure_test_init();
        let va = VirtAddr::new(0x1000_0000 + 220 * config::PAGE_SIZE);
        reserve(va, 4 * config::PAGE_SIZE);
        let result = AllocatedPages::alloc_at(va, 4);
        assert!(result.is_err());
    }
}
