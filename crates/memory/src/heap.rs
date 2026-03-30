//! 中断安全的内核堆分配器。
//!
//! 通过 `SpinLock`（自动禁用/恢复中断）包装 `buddy_system_allocator`，
//! 作为 `#[global_allocator]` 为内核提供 `Box`、`Vec` 等堆分配能力。

use buddy_system_allocator::Heap;
use config::KERNEL_HEAP_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::SyncUnsafeCell;
use core::ptr::NonNull;
use sync_crate::SpinLockIrq;

/// 中断安全的全局堆分配器——参考 Theseus OS 设计。
///
/// 通过 `SpinLockIrq`（获取时禁用中断、释放时恢复）保证多核互斥和中断安全，
/// 防止中断处理器中的隐式分配导致同核心递归加锁。
struct IrqSafeHeap(SpinLockIrq<Heap<32>>);

unsafe impl GlobalAlloc for IrqSafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |nn| nn.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: ptr 来自先前的 alloc 且尚未被释放
        unsafe {
            self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout);
        }
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: IrqSafeHeap = IrqSafeHeap(SpinLockIrq::new_with_level(
    Heap::empty(),
    "heap",
    sync_crate::lock_level::HEAP,
));

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
