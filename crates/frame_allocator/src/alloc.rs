//! 全局帧分配器——buddy system 封装与初始化。
//!
//! 后端使用 [`buddy_system_allocator::FrameAllocator<32>`]：
//! - O(log n) alloc / O(log n) dealloc（buddy 合并）
//! - 分配结果按请求大小的 next_power_of_two 对齐——天然满足大页对齐需求
//! - 元数据存储在堆上的 `BTreeSet`，不触碰空闲帧内存

// TODO: Per-CPU 帧缓存消除 SMP 全局锁瓶颈——待引入用户进程后再评估。

use buddy_system_allocator::FrameAllocator;
use memory_types::{Frame, PhysAddr};

use crate::FrameSpan;
use sync_crate::SpinLockIrq;

use crate::FrameAllocError;
use crate::frames::AllocatedFrames;

/// 全局帧分配器，以页帧（PAGE_SIZE 字节）为单位管理物理内存。
///
/// 调用方（`memory::init`）保证 `init` 仅调用一次。
static FRAME_ALLOCATOR: SpinLockIrq<FrameAllocator<32>> = SpinLockIrq::new(
    FrameAllocator::new(),
    "frame_alloc",
    sync_crate::lock_level::FRAME_ALLOC,
);

/// 将空闲物理内存加入 buddy 后端。
///
/// # Safety
///
/// - `free_start` 页对齐、`free_size > 0`、`[free_start, free_start+free_size)` 有效
/// - 仅调用一次（由上层启动流程保证）
unsafe fn init_buddy(free_start: PhysAddr, free_size: usize) {
    assert!(
        free_start.is_aligned(),
        "frame_allocator::init: free_start 未页对齐: {free_start}"
    );
    assert!(free_size > 0, "frame_allocator::init: free_size 为 0");
    assert!(
        free_start.as_usize().checked_add(free_size).is_some(),
        "frame_allocator::init: 分配区间越过地址空间"
    );

    let start_frame = free_start.page_number().as_usize();
    let end_frame = (free_start + free_size).page_number().as_usize();

    FRAME_ALLOCATOR.lock().add_frame(start_frame, end_frame);

    log::info!(
        "FrameInit: {} MB free from {}",
        free_size / (1024 * 1024),
        free_start
    );
}

/// 将预留范围直接构造为 `AllocatedFrames`，不经过 buddy。
///
/// 调用方负责预留范围的生命周期（内核段通过 `mem::forget` 永久持有）。
///
/// # Panics
///
/// `start` 未页对齐或 `count == 0` 时 panic——预留描述错误是内核 bug。
fn claim_reserved(start: PhysAddr, count: usize) -> AllocatedFrames {
    assert!(
        start.is_aligned(),
        "frame_allocator::init: reserved 范围未页对齐: {start}"
    );
    assert!(count > 0, "frame_allocator::init: reserved 范围 count 为 0");
    let s = start.page_number();
    log::info!("FrameInit: reserved {} pages at {}", count, start);
    AllocatedFrames::from_range(FrameSpan::new(s, s + count))
}

/// 初始化帧分配器——空闲内存入 buddy，预留范围构造为 `AllocatedFrames` 返回。
///
/// - `free_start` / `free_size`：空闲物理内存范围，加入 buddy allocator
/// - `reserved`：需要预留的物理地址范围 `(start, page_count)` 列表；
///   不经过 buddy，直接构造为 `AllocatedFrames` 返回给调用方
///
/// 预留范围的帧由调用方负责生命周期管理（内核段通过 `mem::forget` 永久持有）。
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
    // SAFETY: 调用方约束直接转发给 init_buddy
    unsafe { init_buddy(free_start, free_size) };

    let mut result = heapless::Vec::new();
    for &(start, count) in reserved {
        result
            .push(claim_reserved(start, count))
            .expect("frame_allocator::init: reserved 范围数不超过 8");
    }
    result
}

/// 从 buddy allocator 取出帧，构造 `AllocatedFrames`。
///
/// 这是与底层分配器交互的唯一分配出口——所有分配路径都经过此函数。
///
/// 注意 buddy 内部会将 `count` 向上取整到 2 的幂次，
/// 实际分配的帧数可能多于请求数，但 `FrameSpan` 仅跟踪请求的帧。
///
/// # Panics
///
/// `count == 0` 时 panic（调用方逻辑错误，不是 OOM）。
pub(crate) fn alloc_from_backend(count: usize) -> Result<AllocatedFrames, FrameAllocError> {
    assert!(count > 0, "alloc_from_backend: count 不能为 0");

    let frame_num = FRAME_ALLOCATOR
        .lock()
        .alloc(count)
        // TODO: OOM 时应尝试回收（页缓存淘汰、swap out），
        // 而非直接失败。待引入 page cache / swap 后实现。
        .ok_or(FrameAllocError::OutOfMemory)?;
    let start = Frame::new(frame_num);
    Ok(AllocatedFrames::from_range(FrameSpan::new(
        start,
        start + count,
    )))
}

/// 归还帧到 buddy allocator——仅由 `AllocatedFrames` 的 Drop 调用。
///
/// # Panics
///
/// `range` 为空时 panic——零大小帧不应存在，表示分配器内部逻辑错误。
pub(crate) fn dealloc_to_backend(range: FrameSpan) {
    let count = range.size();
    assert!(
        count > 0,
        "dealloc_to_backend: 归还零大小帧范围 ({range:?})，分配器逻辑错误"
    );
    FRAME_ALLOCATOR
        .lock()
        .dealloc(range.start().as_usize(), count);
}
