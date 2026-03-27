use buddy_system_allocator::LockedHeap;
use config::KERNEL_HEAP_SIZE;
use core::cell::SyncUnsafeCell;

/// 全局堆分配器。
///
/// **中断安全限制**：`LockedHeap` 内部使用 `spin::Mutex`（不禁用中断）。
/// 中断处理器中**禁止**进行堆分配（`Box::new`、`Vec::push`、`format!` 等），
/// 否则会因同核心自旋导致死锁。日志宏 `log::info!` 等使用栈缓冲区，安全。
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
