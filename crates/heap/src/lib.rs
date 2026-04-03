//! 内核堆分配器。
//!
//! 通过 `SpinLock` 包装 `buddy_system_allocator`，
//! 作为 `#[global_allocator]` 为内核提供 `Box`、`Vec` 等堆分配能力。
//!
//! **禁止在中断上下文中进行堆分配**——alloc/dealloc 入口包含运行时断言。

#![cfg_attr(not(test), no_std)]
#![feature(sync_unsafe_cell)]

use buddy_system_allocator::Heap;
use config::KERNEL_HEAP_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::SyncUnsafeCell;
use core::ptr::NonNull;
use sync_crate::SpinLock;

/// 全局堆分配器。
///
/// 禁止在中断上下文中使用——中断处理器应使用栈分配或 `heapless` 容器。
struct SafeHeap(SpinLock<Heap<32>>);

/// 断言当前不在中断上下文中——中断中堆操作会死锁或破坏分配器状态。
#[inline(always)]
fn assert_not_in_irq() {
    assert!(
        !interrupt_state::is_in_interrupt(),
        "禁止在中断上下文中进行堆操作"
    );
}

unsafe impl GlobalAlloc for SafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert_not_in_irq();
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |nn| nn.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        assert_not_in_irq();
        // SAFETY: ptr 来自先前的 alloc 且尚未被释放
        unsafe {
            self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout);
        }
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: SafeHeap = SafeHeap(SpinLock::new(
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
