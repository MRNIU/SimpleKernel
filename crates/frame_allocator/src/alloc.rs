//! 全局帧分配器——buddy system 封装与初始化。
//!
//! 后端使用 [`buddy_system_allocator::FrameAllocator<32>`]：
//! - O(log n) alloc / O(log n) dealloc（buddy 合并）
//! - 分配结果按请求大小的 next_power_of_two 对齐——天然满足大页对齐需求
//! - 元数据存储在堆上的 `BTreeSet`，不触碰空闲帧内存

// TODO: Per-CPU 帧缓存消除 SMP 全局锁瓶颈——待引入用户进程后再评估。

use buddy_system_allocator::FrameAllocator;
use memory_types::{Frame, PhysAddr};

use sync_crate::SpinLockIrq;

use crate::FrameAllocError;
use crate::FrameSpan;
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
    let start_frame = free_start.page_number().as_usize();
    let end_frame = (free_start + free_size).page_number().as_usize();

    FRAME_ALLOCATOR.lock().add_frame(start_frame, end_frame);

    log::info!(
        "FrameInit: {} MB free from {}",
        free_size / (1024 * 1024),
        free_start
    );
}

/// 校验空闲物理内存范围，并转换为 buddy 使用的半开帧区间。
///
/// `free_size` 以字节为单位；返回的 [`FrameSpan`] 表示
/// `[free_start.page_number(), free_end.page_number())`。
///
/// # Panics
///
/// `free_start` 未页对齐、`free_size == 0`、结束地址溢出或结束地址未页对齐时
/// panic。启动期空闲内存描述错误是内核初始化不变量错误，必须在入 buddy 前暴露。
fn free_span(free_start: PhysAddr, free_size: usize) -> FrameSpan {
    assert!(
        free_start.is_aligned(),
        "frame_allocator::init: free_start 未页对齐: {free_start}"
    );
    assert!(free_size > 0, "frame_allocator::init: free_size 为 0");

    let free_end = free_start + free_size;
    assert!(
        free_end.is_aligned(),
        "frame_allocator::init: free 结束地址未页对齐: start={free_start}, size={free_size}, end={free_end}"
    );

    FrameSpan::new(free_start.page_number(), free_end.page_number())
}

/// 校验单个预留物理范围，并转换为半开帧区间。
///
/// `count` 以 4K 页为单位；`index` 只用于 panic 信息定位 `reserved[index]`。
///
/// # Panics
///
/// `start` 未页对齐、`count == 0`、或 `start + count` 超出当前架构可表示的
/// 物理帧范围时 panic。
fn reserved_span(index: usize, start: PhysAddr, count: usize) -> FrameSpan {
    assert!(
        start.is_aligned(),
        "frame_allocator::init: reserved[{index}] 范围未页对齐: {start}"
    );
    assert!(
        count > 0,
        "frame_allocator::init: reserved[{index}] 范围 count 为 0"
    );

    let start_frame = start.page_number();
    FrameSpan::new(start_frame, start_frame + count)
}

/// 校验预留范围不污染 buddy 的空闲池描述。
///
/// 此函数必须在 [`init_buddy`] 之前调用：`reserved` 不会从 buddy 中扣除页面，
/// 因此调用方传入的 `free` 必须已经排除所有预留区。
///
/// # Panics
///
/// 任一 `reserved` 条目非法、与 `free` 重叠、或与前面的 `reserved` 条目重叠时
/// panic。
fn validate_reserved_ranges(free: FrameSpan, reserved: &[(PhysAddr, usize)]) {
    for (i, &(start, count)) in reserved.iter().enumerate() {
        let span = reserved_span(i, start, count);
        assert!(
            !span.overlaps(free),
            "frame_allocator::init: reserved[{i}] 范围 [{}, {}) 与 free 范围 [{}, {}) 重叠",
            span.start(),
            span.end(),
            free.start(),
            free.end()
        );

        for (j, &(other_start, other_count)) in reserved[..i].iter().enumerate() {
            let other = reserved_span(j, other_start, other_count);
            assert!(
                !span.overlaps(other),
                "frame_allocator::init: reserved[{i}] 范围 [{}, {}) 与 reserved[{j}] 范围 [{}, {}) 重叠",
                span.start(),
                span.end(),
                other.start(),
                other.end()
            );
        }
    }
}

/// 初始化帧分配器——空闲内存入 buddy，校验并记录预留范围。
///
/// - `free_start` / `free_size`：空闲物理内存范围，加入 buddy allocator
/// - `reserved`：需要预留的物理地址范围 `(start, page_count)` 列表；
///   用于校验和日志记录；调用方必须保证 `free` 范围已经排除这些区域。
///
/// # Safety
///
/// - 所有范围必须有效、页对齐、互不重叠
/// - `free` 范围和 `reserved` 范围不得重叠
/// - 仅调用一次
///
/// # Panics
///
/// `free` 或 `reserved` 任一条目未页对齐、`count == 0`、结束地址溢出，
/// 或范围发生重叠时 panic——初始化描述错误是内核 bug。
pub unsafe fn init(free_start: PhysAddr, free_size: usize, reserved: &[(PhysAddr, usize)]) {
    let free = free_span(free_start, free_size);
    validate_reserved_ranges(free, reserved);

    // SAFETY: 调用方约束直接转发给 init_buddy
    unsafe { init_buddy(free_start, free_size) };

    for &(start, count) in reserved {
        log::info!("FrameInit: reserved {} pages at {}", count, start);
    }
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
