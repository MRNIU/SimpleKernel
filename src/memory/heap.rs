use crate::config::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;
use core::cell::SyncUnsafeCell;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

/// BSS 区域堆后备存储。
///
/// 使用 `SyncUnsafeCell` 代替 `static mut`，遵循项目规范。
/// 仅在 `init()` 中通过指针访问，之后由 `HEAP_ALLOCATOR` 独占管理。
static HEAP_SPACE: SyncUnsafeCell<[u8; KERNEL_HEAP_SIZE]> =
    SyncUnsafeCell::new([0; KERNEL_HEAP_SIZE]);

/// 初始化内核堆分配器。
///
/// # Safety
/// 必须恰好调用一次，且在任何堆分配之前调用。
pub unsafe fn init() {
    let heap_start = HEAP_SPACE.get() as usize;
    // SAFETY: HEAP_SPACE 是静态 BSS 区域，init 仅调用一次，
    // 之后该区域完全由 HEAP_ALLOCATOR 内部锁保护
    unsafe {
        HEAP_ALLOCATOR.lock().init(heap_start, KERNEL_HEAP_SIZE);
    }
    log::info!(
        "HeapInit: {}MB heap at {:#x}",
        KERNEL_HEAP_SIZE / (1024 * 1024),
        heap_start
    );
}
