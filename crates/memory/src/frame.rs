use crate::address::PhysAddr;
use crate::error::MemoryError;
use config::PAGE_SIZE;
use sync_crate::SpinLock;

/// Global frame allocator — wraps buddy_system_allocator::FrameAllocator.
/// Operates in units of page frames (PAGE_SIZE bytes each).
static FRAME_ALLOCATOR: SpinLock<FrameAllocatorInner> =
    SpinLock::new(FrameAllocatorInner::new(), "frame_alloc");

struct FrameAllocatorInner {
    allocator: buddy_system_allocator::FrameAllocator<32>,
    initialized: bool,
}

impl FrameAllocatorInner {
    const fn new() -> Self {
        Self {
            allocator: buddy_system_allocator::FrameAllocator::new(),
            initialized: false,
        }
    }
}

/// Initialize the frame allocator with available physical memory.
///
/// `start` must be page-aligned. The region `[start, start+size)` becomes
/// available for frame allocation.
///
/// # Safety
/// The memory region must be valid, not overlap with kernel/heap, and
/// this must be called exactly once.
pub unsafe fn init(start: PhysAddr, size: usize) {
    let mut alloc = FRAME_ALLOCATOR.lock();
    assert!(!alloc.initialized, "frame::init called twice");
    assert!(start.is_aligned(), "frame::init: start not page-aligned");
    assert!(size > 0, "frame::init: size is zero");
    // 防止可分配区域地址回绕（例如内核镜像过大导致 start 超出物理内存范围）
    assert!(
        start.as_usize().checked_add(size).is_some(),
        "frame::init: allocation region overflows address space"
    );

    let start_frame = start.as_usize() / PAGE_SIZE;
    let end_frame = start_frame + size / PAGE_SIZE;
    alloc.allocator.add_frame(start_frame, end_frame);
    alloc.initialized = true;

    log::info!(
        "FrameInit: {} MB available from {}",
        size / (1024 * 1024),
        start
    );
}

/// Physical frame RAII guard — automatically returns frame to allocator on Drop.
/// Eliminates "forgot to free_frame" physical memory leaks (rCore pattern).
pub struct FrameTracker {
    paddr: PhysAddr,
}

impl FrameTracker {
    /// Allocate a single physical frame (PAGE_SIZE bytes), zeroed.
    pub fn alloc() -> Result<Self, MemoryError> {
        let mut alloc = FRAME_ALLOCATOR.lock();
        if !alloc.initialized {
            return Err(MemoryError::AllocationFailed);
        }
        let frame_num = alloc.allocator.alloc(1).ok_or(MemoryError::OutOfMemory)?;
        let paddr = PhysAddr::new(frame_num * PAGE_SIZE);

        // SAFETY: 当前使用 identity mapping（VA == PA），物理地址可直接作为虚拟地址访问。
        // 帧刚从分配器获取，不存在其他引用。
        // 若未来切换为非 identity mapping，此处需通过 phys_to_virt() 转换。
        unsafe {
            core::ptr::write_bytes(paddr.as_usize() as *mut u8, 0, PAGE_SIZE);
        }

        Ok(Self { paddr })
    }

    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        let mut alloc = FRAME_ALLOCATOR.lock();
        let frame_num = self.paddr.as_usize() / PAGE_SIZE;
        alloc.allocator.dealloc(frame_num, 1);
    }
}
