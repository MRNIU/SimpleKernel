//! 内核堆分配器——`#[global_allocator]` 实现。
//!
//! # 两阶段初始化
//!
//! ```text
//! memory::init()
//!    │
//!    ├── 1. heap::init()              ← BSS 引导堆（64KB，仅够 BTreeSet）
//!    ├── 2. frame_allocator::init()   （依赖引导堆）
//!    ├── 3. heap::extend(addr, size)  ← 帧分配器就绪后，用物理帧扩展堆
//!    └── 4. PageTable / OwnedPages    （使用扩展后的完整堆）
//! ```
//!
//! 引导堆存在的原因：`frame_allocator` 的 buddy 后端内部使用 `BTreeSet`
//! （第三方 crate `buddy_system_allocator`），需要堆分配。
//! 引导堆是打破 "堆需要帧 ↔ 帧需要堆" 循环依赖的最小 BSS 垫片。
//!
//! **禁止在中断上下文中进行堆分配**——alloc/dealloc 入口包含运行时断言。

#![no_std]
#![feature(sync_unsafe_cell)]

use buddy_system_allocator::Heap;
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

/// BSS 引导堆——打破 "堆需要帧 ↔ 帧需要堆" 循环依赖的最小垫片。
///
/// 仅 `BOOTSTRAP_HEAP_SIZE`（64KB），够 `frame_allocator::init()` 的
/// `BTreeSet` 分配。帧分配器就绪后通过 [`extend`] 用物理帧扩展到完整堆。
static BOOTSTRAP_HEAP: SyncUnsafeCell<[u8; config::BOOTSTRAP_HEAP_SIZE]> =
    SyncUnsafeCell::new([0; config::BOOTSTRAP_HEAP_SIZE]);

/// 初始化引导堆——仅提供 `frame_allocator::init()` 所需的最小堆。
///
/// # Safety
/// 必须恰好调用一次，且在任何堆分配之前调用。
pub unsafe fn init() {
    let start = BOOTSTRAP_HEAP.get() as usize;
    // SAFETY: BOOTSTRAP_HEAP 是静态 BSS 区域，init 仅调用一次
    unsafe {
        HEAP_ALLOCATOR
            .0
            .lock()
            .init(start, config::BOOTSTRAP_HEAP_SIZE);
    }
    log::info!(
        "HeapInit: {}KB bootstrap heap at {:#x}",
        config::BOOTSTRAP_HEAP_SIZE / 1024,
        start
    );
}

/// 用帧分配器提供的物理内存扩展堆。
///
/// 在 `frame_allocator::init()` 之后调用。SAS identity mapping 下
/// 物理地址即虚拟地址，帧内存可直接作为堆空间使用。
///
/// # Safety
/// - `start` 必须是有效的、页对齐的物理地址（identity-mapped）
/// - `[start, start+size)` 范围内的内存不被其他模块使用
/// - 调用方负责持有对应的 `AllocatedFrames`（防止帧被回收）
pub unsafe fn extend(start: usize, size: usize) {
    // SAFETY: 调用方保证内存范围有效且独占
    unsafe {
        HEAP_ALLOCATOR.0.lock().add_to_heap(start, start + size);
    }
    log::info!("HeapExtend: +{}KB at {:#x}", size / 1024, start);
}
