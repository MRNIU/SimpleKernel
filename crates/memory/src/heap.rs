use buddy_system_allocator::Heap;
use config::KERNEL_HEAP_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::SyncUnsafeCell;
use core::ptr::NonNull;

/// 中断安全的全局堆分配器——参考 Theseus OS 设计。
///
/// 在分配/释放前通过 `HeldInterrupts` 禁用中断，
/// 防止中断处理器中的隐式分配导致同核心自旋死锁。
/// 使用 `spin::Mutex` 保证多核互斥。
struct IrqSafeHeap(spin::Mutex<Heap<32>>);

// SAFETY: IrqSafeHeap 通过 spin::Mutex 保证内部互斥，
// 通过 HeldInterrupts 保证中断安全。
unsafe impl Sync for IrqSafeHeap {}

unsafe impl GlobalAlloc for IrqSafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _held = sync_crate::HeldInterrupts::hold();
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |nn| nn.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _held = sync_crate::HeldInterrupts::hold();
        // SAFETY: ptr 来自先前的 alloc 且尚未被释放
        unsafe {
            self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout);
        }
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: IrqSafeHeap = IrqSafeHeap(spin::Mutex::new(Heap::empty()));

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
        HEAP_ALLOCATOR.0.lock().init(heap_start, KERNEL_HEAP_SIZE);
    }
    log::info!(
        "HeapInit: {}MB heap at {:#x}",
        KERNEL_HEAP_SIZE / (1024 * 1024),
        heap_start
    );
}
