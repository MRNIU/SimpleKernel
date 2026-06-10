// Copyright The SimpleKernel Contributors

//! 内核堆分配器——`#[global_allocator]` 实现。
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

// SAFETY: `SafeHeap` 通过 `SpinLock` 串行化伙伴系统分配器访问；alloc/dealloc
// 在进入锁前拒绝中断上下文，避免中断重入造成递归加锁或破坏分配器元数据。
unsafe impl GlobalAlloc for SafeHeap {
    /// # Safety
    ///
    /// 调用方必须满足 [`GlobalAlloc::alloc`] 契约，传入有效的 `layout`。
    /// 本实现只返回全局堆中独占的未初始化内存；返回空指针表示分配失败。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert_not_in_irq();
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |nn| nn.as_ptr())
    }

    /// # Safety
    ///
    /// 调用方必须满足 [`GlobalAlloc::dealloc`] 契约，`ptr` 必须来自此前成功的
    /// `alloc` 调用且尚未释放，`layout` 必须与原分配匹配。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        assert_not_in_irq();
        // SAFETY: `GlobalAlloc::dealloc` 的调用方保证 `ptr` 非空、来自本分配器且
        // `layout` 与原分配匹配；违反这些前提会破坏伙伴系统分配器元数据并导致 UB。
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
///
/// 必须恰好调用一次，且在任何堆分配之前调用。
///
/// # Panics
///
/// 在中断上下文中调用时 panic。启动期堆初始化必须在线程/中断路径介入前完成。
pub unsafe fn init() {
    let start = BOOTSTRAP_HEAP.get() as usize;
    // SAFETY: `BOOTSTRAP_HEAP` 是静态 BSS 区域，安全契约要求本函数只在首次堆使用前
    // 调用一次；重复初始化会覆盖伙伴系统分配器状态并泄漏或重叠已有分配。
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
/// 在 `frame_allocator::init()` 之后调用。SAS 恒等映射下
/// 物理地址即虚拟地址，帧内存可直接作为堆空间使用。
///
/// # Safety
///
/// - `start` 必须是有效的、页对齐的物理地址（已恒等映射）
/// - `[start, start+size)` 范围内的内存不被其他模块使用
/// - 调用方负责持有对应的 `AllocatedFrames`（防止帧被回收）
/// - `start + size` 不得溢出
///
/// # Panics
///
/// 在中断上下文中调用时 panic。堆扩展会修改全局分配器状态，不能在硬中断中执行。
pub unsafe fn extend(start: usize, size: usize) {
    // SAFETY: 安全契约要求 `[start, start + size)` 是有效、独占且已恒等映射
    // 的堆扩展区；若范围与其他所有者重叠，后续堆分配会产生别名和内存破坏。
    unsafe {
        HEAP_ALLOCATOR.0.lock().add_to_heap(start, start + size);
    }
    log::info!("HeapExtend: +{}KB at {:#x}", size / 1024, start);
}
