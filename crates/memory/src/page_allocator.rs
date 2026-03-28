//! 虚拟页分配器——管理虚拟地址空间，产出 [`AllocatedPages`]。
//!
//! 与帧分配器对称：帧分配器管理物理页号，页分配器管理虚拟页号。
//! `AllocatedPages` 是 move-only 类型，Drop 时自动归还分配器。

use crate::error::MemoryError;
use address::{PageRange, VirtAddr, VirtPageNum};
use config::PAGE_SIZE;
use sync_crate::SpinLock;

/// 全局虚拟页分配器。
static PAGE_ALLOCATOR: SpinLock<PageAllocatorInner> =
    SpinLock::new(PageAllocatorInner::new(), "page_alloc");

/// 页分配器内部状态。
struct PageAllocatorInner {
    allocator: buddy_system_allocator::FrameAllocator<32>,
    initialized: bool,
}

impl PageAllocatorInner {
    const fn new() -> Self {
        Self {
            allocator: buddy_system_allocator::FrameAllocator::new(),
            initialized: false,
        }
    }
}

/// 初始化虚拟页分配器，将 `[start_vpn, start_vpn + count)` 加入可分配池。
///
/// # Safety
///
/// 调用方必须确保该虚拟页号范围不与已有映射冲突，且仅调用一次。
pub unsafe fn init(start_vpn: VirtPageNum, count: usize) {
    let mut alloc = PAGE_ALLOCATOR.lock();
    assert!(!alloc.initialized, "page_allocator::init called twice");
    assert!(count > 0, "page_allocator::init: count is zero");

    let start = start_vpn.as_usize();
    alloc.allocator.add_frame(start, start + count);
    alloc.initialized = true;

    log::info!(
        "PageAllocInit: {} pages from {}",
        count,
        start_vpn.start_addr()
    );
}

/// 已分配的虚拟页范围——Drop 时自动归还分配器。
pub struct AllocatedPages {
    range: PageRange,
}

impl AllocatedPages {
    /// 分配 `count` 个连续虚拟页。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，虚拟页耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, MemoryError> {
        let mut alloc = PAGE_ALLOCATOR.lock();
        if !alloc.initialized {
            return Err(MemoryError::AllocationFailed);
        }
        let vpn_num = alloc
            .allocator
            .alloc(count)
            .ok_or(MemoryError::OutOfMemory)?;
        let start = VirtPageNum::new(vpn_num);
        let end = VirtPageNum::new(vpn_num + count);
        Ok(Self {
            range: PageRange::new(start, end),
        })
    }

    /// 分配单个虚拟页。
    pub fn alloc_one() -> Result<Self, MemoryError> {
        Self::alloc(1)
    }

    /// 返回页范围。
    #[inline]
    pub fn range(&self) -> PageRange {
        self.range
    }

    /// 范围内页数。
    #[inline]
    pub fn count(&self) -> usize {
        self.range.size()
    }

    /// 起始虚拟页号。
    #[inline]
    pub fn start(&self) -> VirtPageNum {
        self.range.start()
    }

    /// 起始虚拟地址。
    #[inline]
    pub fn start_vaddr(&self) -> VirtAddr {
        self.range.start().start_addr()
    }

    /// 映射总大小（字节）。
    #[inline]
    pub fn size_bytes(&self) -> usize {
        self.range.size() * PAGE_SIZE
    }
}

impl Drop for AllocatedPages {
    fn drop(&mut self) {
        if self.range.size() == 0 {
            return;
        }
        let mut alloc = PAGE_ALLOCATOR.lock();
        alloc
            .allocator
            .dealloc(self.range.start().as_usize(), self.range.size());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 初始化测试用页分配器。
    fn ensure_init() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            // 虚拟页号从 0x1000 开始，分配 64 页
            unsafe { init(VirtPageNum::new(0x1000), 64) };
        });
    }

    /// 分配单页后页数应为 1。
    #[test]
    fn alloc_one_page() {
        ensure_init();
        let pages = AllocatedPages::alloc_one().expect("alloc_one 应成功");
        assert_eq!(pages.count(), 1);
        assert_eq!(pages.size_bytes(), PAGE_SIZE);
    }

    /// 分配多页后页数应正确。
    #[test]
    fn alloc_multiple_pages() {
        ensure_init();
        let pages = AllocatedPages::alloc(4).expect("alloc(4) 应成功");
        assert_eq!(pages.count(), 4);
    }

    /// 页 drop 后应能重新分配。
    #[test]
    fn alloc_dealloc_realloc() {
        ensure_init();
        {
            let _pages = AllocatedPages::alloc(2).expect("分配");
        }
        let pages = AllocatedPages::alloc(2).expect("重新分配应成功");
        assert_eq!(pages.count(), 2);
    }

    /// 起始虚拟地址应正确。
    #[test]
    fn start_vaddr_correct() {
        ensure_init();
        let pages = AllocatedPages::alloc_one().expect("分配");
        // 起始 VPN >= 0x1000（测试分配器的起始值）
        assert!(pages.start().as_usize() >= 0x1000);
        assert_eq!(
            pages.start_vaddr(),
            VirtAddr::new(pages.start().as_usize() * PAGE_SIZE)
        );
    }

    /// 分配器已初始化后分配应成功，验证 ensure_init 幂等性。
    #[test]
    fn alloc_after_init_succeeds() {
        ensure_init();
        // 确认分配器已初始化后的正常分配
        let pages = AllocatedPages::alloc_one().expect("已初始化后分配应成功");
        assert_eq!(pages.count(), 1);
    }

    /// size_bytes 应返回正确的字节大小。
    #[test]
    fn size_bytes_correct() {
        ensure_init();
        let pages = AllocatedPages::alloc(3).expect("分配 3 页");
        assert_eq!(pages.size_bytes(), 3 * PAGE_SIZE);
    }
}
