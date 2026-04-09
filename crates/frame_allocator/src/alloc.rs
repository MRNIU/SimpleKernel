//! 全局帧分配器——bitmap allocator 封装与初始化。

//
// TODO: Per-CPU 帧缓存——消除 SMP 全局锁瓶颈
//
// 当前所有 alloc/dealloc 都竞争同一把 `SpinLock`，核数越多缓存行弹跳越严重。
// 应引入两级结构：
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
//   多帧连续分配（count > 1）直接走全局分配器。
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

use memory_types::{Frame, FrameSpan, PhysAddr};
use sync_crate::SpinLockIrq;

use crate::backend::FrameAllocBackend;
use crate::bitmap::BitmapAllocator;
use crate::state::AllocatedFrames;

use crate::FrameAllocError;
use crate::state::FreeFrames;

/// 全局帧分配器，以页帧（PAGE_SIZE 字节）为单位管理物理内存。
static FRAME_ALLOCATOR: SpinLockIrq<FrameAllocatorInner> = SpinLockIrq::new(
    FrameAllocatorInner::new(),
    "frame_alloc",
    sync_crate::lock_level::FRAME_ALLOC,
);

/// 帧分配器内部状态。
struct FrameAllocatorInner {
    allocator: BitmapAllocator,
    initialized: bool,
}

impl FrameAllocatorInner {
    const fn new() -> Self {
        Self {
            allocator: BitmapAllocator::new(),
            initialized: false,
        }
    }
}

/// 初始化帧分配器——空闲内存入 bitmap，预留范围构造为 `AllocatedFrames` 返回。
///
/// - `free_start`/`free_size`：空闲物理内存范围，加入 bitmap allocator
/// - `reserved`：需要预留的物理地址范围列表 `(start, page_count)`，
///   不经过 bitmap——直接构造为 `AllocatedFrames` 返回给调用方
///
/// 预留范围的帧由调用方负责生命周期管理（通常由 `AddressSpace`
/// 通过 `OwnedPages` 持有直到关机）。
///
/// # Safety
///
/// - 所有范围必须有效、页对齐、互不重叠
/// - `free` 范围和 `reserved` 范围不得重叠
/// - 仅调用一次
pub unsafe fn init(
    free_start: PhysAddr,
    free_size: usize,
    reserved: &[(PhysAddr, usize)],
) -> heapless::Vec<AllocatedFrames, 8> {
    let mut alloc = FRAME_ALLOCATOR.lock();
    assert!(!alloc.initialized, "frame_allocator::init called twice");
    assert!(
        free_start.is_aligned(),
        "frame_allocator::init: free_start not page-aligned"
    );
    assert!(free_size > 0, "frame_allocator::init: free_size is zero");
    assert!(
        free_start.as_usize().checked_add(free_size).is_some(),
        "frame_allocator::init: allocation region overflows address space"
    );

    let start_frame = free_start.page_number().as_usize();
    let end_frame = PhysAddr::new(free_start.as_usize() + free_size)
        .page_number()
        .as_usize();
    alloc.allocator.add_frames(start_frame, end_frame);
    alloc.initialized = true;

    log::info!(
        "FrameInit: {} MB free from {}",
        free_size / (1024 * 1024),
        free_start
    );

    // 预留范围构造为 AllocatedFrames（不经过 bitmap）
    let mut result = heapless::Vec::new();
    for &(start, count) in reserved {
        assert!(
            start.is_aligned(),
            "frame_allocator::init: reserved range not page-aligned: {start}"
        );
        assert!(
            count > 0,
            "frame_allocator::init: reserved range count is zero"
        );
        let s = Frame::new(start.page_number().as_usize());
        let e = Frame::new(s.as_usize() + count);
        let frames = AllocatedFrames::from_range(FrameSpan::new(s, e));
        result
            .push(frames)
            .expect("frame_allocator::init: reserved 范围数不超过 8");
        log::info!("FrameInit: reserved {} pages at {}", count, start);
    }

    result
}

/// 从 bitmap allocator 取出帧，构造 `FreeFrames`。
///
/// 这是与底层分配器交互的唯一分配出口——所有分配路径都经过此函数。
pub(crate) fn alloc_from_backend(count: usize) -> Result<FreeFrames, FrameAllocError> {
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
    let start = Frame::new(frame_num);
    let end = Frame::new(frame_num + count);
    Ok(FreeFrames::from_range(FrameSpan::new(start, end)))
}

/// 归还帧到 bitmap allocator——仅由 `Frames` 的 Drop 调用。
pub(crate) fn dealloc_to_backend(range: FrameSpan) {
    if range.size() == 0 {
        return;
    }
    let mut alloc = FRAME_ALLOCATOR.lock();
    let start = range.start().as_usize();
    let count = range.size();
    alloc.allocator.dealloc(start, count);
}
