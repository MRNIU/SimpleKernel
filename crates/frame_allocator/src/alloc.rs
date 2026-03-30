//! 全局帧分配器——buddy allocator 封装与初始化。

//
// TODO: Per-CPU 帧缓存——消除 SMP 全局锁瓶颈
//
// 当前所有 alloc/dealloc 都竞争同一把 `SpinLock`，核数越多缓存行弹跳越严重。
// 应引入两级结构（参考 Linux `struct per_cpu_pages`，`include/linux/mmzone.h`）：
//
// 架构：
//   每个 CPU 持有本地缓存（`#[cpu_local] static PER_CPU_CACHE`），
//   快速路径只需关中断即可操作，无需任何锁；
//   缓存耗尽/溢出时才拿全局锁批量 refill/drain。
//
// 关键参数：
//   - HIGH  = 64  缓存上限，超过则 drain BATCH 帧到全局
//   - BATCH = 16  每次 refill/drain 的帧数
//
// 数据结构：
//   固定大小数组 + 计数器（栈式），只缓存单帧（order-0）。
//   多帧连续分配（count > 1）直接走全局 buddy allocator。
//
// 中断安全：
//   通过 `HeldInterrupts` token 证明中断已关闭，
//   `PerCpuFrameCache` 的方法签名接受 `&HeldInterrupts` 参数。
//
// 对现有代码的影响：
//   仅修改本文件（alloc.rs）；state.rs 和 transitions.rs 无需改动。
//
// 前置条件：
//   需要用户进程（页分配频率足够高才有优化价值）。

use address::{FrameRange, PhysAddr, PhysPageNum};
use sync_crate::SpinLockIrq;

use crate::FrameAllocError;
use crate::state::FreeFrames;

/// 全局帧分配器，以页帧（PAGE_SIZE 字节）为单位管理物理内存。
static FRAME_ALLOCATOR: SpinLockIrq<FrameAllocatorInner> = SpinLockIrq::new_with_level(
    FrameAllocatorInner::new(),
    "frame_alloc",
    sync_crate::lock_level::FRAME_ALLOC,
);

/// 帧分配器内部状态。
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

/// 初始化帧分配器，将 `[start, start+size)` 区域加入可分配池。
///
/// `start` 必须页对齐。
///
/// # Safety
/// 该内存区域必须有效、不与内核/堆重叠，且仅调用一次。
pub unsafe fn init(start: PhysAddr, size: usize) {
    let mut alloc = FRAME_ALLOCATOR.lock();
    assert!(!alloc.initialized, "frame_allocator::init called twice");
    assert!(
        start.is_aligned(),
        "frame_allocator::init: start not page-aligned"
    );
    assert!(size > 0, "frame_allocator::init: size is zero");
    assert!(
        start.as_usize().checked_add(size).is_some(),
        "frame_allocator::init: allocation region overflows address space"
    );

    let start_frame = start.page_number().as_usize();
    let end_frame = PhysAddr::new(start.as_usize() + size)
        .page_number()
        .as_usize();
    alloc.allocator.add_frame(start_frame, end_frame);
    alloc.initialized = true;

    log::info!(
        "FrameInit: {} MB available from {}",
        size / (1024 * 1024),
        start
    );
}

/// 从 buddy allocator 取出帧，构造 `FreeFrames`。
///
/// 这是与底层分配器交互的唯一分配出口——所有分配路径都经过此函数。
pub(crate) fn alloc_from_buddy(count: usize) -> Result<FreeFrames, FrameAllocError> {
    let mut alloc = FRAME_ALLOCATOR.lock();
    if !alloc.initialized {
        return Err(FrameAllocError::AllocationFailed);
    }
    let frame_num = alloc
        .allocator
        .alloc(count)
        // TODO: OOM 时应尝试回收（页缓存淘汰、swap out），
        // 而非直接失败。待引入 page cache / swap 后实现。
        .ok_or(FrameAllocError::OutOfMemory)?;
    let start = PhysPageNum::new(frame_num);
    let end = PhysPageNum::new(frame_num + count);
    Ok(FreeFrames {
        range: FrameRange::new(start, end),
    })
}

/// 归还帧到 buddy allocator——仅由 `Frames` 的 Drop 调用。
pub(crate) fn dealloc_to_buddy(range: FrameRange) {
    if range.size() == 0 {
        return;
    }
    let mut alloc = FRAME_ALLOCATOR.lock();
    let start = range.start().as_usize();
    let count = range.size();
    alloc.allocator.dealloc(start, count);
}
