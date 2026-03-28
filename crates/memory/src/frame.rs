//! 物理帧分配器 + typestate 生命周期追踪。
//!
//! 参考 Theseus OS 的 `Frames<const S: MemoryState>` 设计：
//! - 帧以**范围**（[`FrameRange`]）为单位管理，而非单个地址
//! - 状态生命周期：`Allocated → Mapped → Unmapped → Allocated → …`
//! - 状态转换消费 self，编译期强制正确的生命周期路径
//! - Drop 按状态分派：`Mapped` 状态 panic（必须经 unmap），其余归还分配器

use crate::error::MemoryError;
use address::{FrameRange, PhysAddr, PhysPageNum};
use config::PAGE_SIZE;
use sync_crate::SpinLock;

/// 全局帧分配器，以页帧（PAGE_SIZE 字节）为单位管理物理内存。
static FRAME_ALLOCATOR: SpinLock<FrameAllocatorInner> =
    SpinLock::new(FrameAllocatorInner::new(), "frame_alloc");

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
    assert!(!alloc.initialized, "frame::init called twice");
    assert!(start.is_aligned(), "frame::init: start not page-aligned");
    assert!(size > 0, "frame::init: size is zero");
    assert!(
        start.as_usize().checked_add(size).is_some(),
        "frame::init: allocation region overflows address space"
    );

    let start_frame = start.as_usize() / PAGE_SIZE;
    let end_frame = start_frame + size / PAGE_SIZE;
    alloc.allocator.add_frame(start_frame, end_frame);
    alloc.initialized = true;

    log::info!(
        "FrameInit: {} MB available from {}",
        size / (1024 * 1024),
        start
    );
}

/// 帧生命周期状态——参考 Theseus OS 的状态模型。
///
/// 状态机：`Allocated → Mapped → Unmapped → Allocated → …`
///
/// 空闲帧的追踪由 buddy allocator 内部管理，不需要 typestate 表示。
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum MemoryState {
    /// 已分配——用户持有，Drop 归还分配器
    Allocated,
    /// 已映射到页表——Drop panic（必须先 unmap）
    Mapped,
    /// 从页表移除——Drop 归还分配器
    Unmapped,
}

/// 类型状态帧范围——编译期追踪物理帧生命周期。
///
/// 与单帧设计不同，`Frames` 持有一段**连续的物理帧范围**（[`FrameRange`]），
/// 支持 [`split_at`](Frames::split_at) 和 [`merge`](Frames::merge) 操作。
///
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧。
pub struct Frames<const S: MemoryState> {
    range: FrameRange,
}

/// 便利别名。
pub type AllocatedFrames = Frames<{ MemoryState::Allocated }>;
/// 便利别名。
pub type MappedFrames = Frames<{ MemoryState::Mapped }>;
/// 便利别名。
pub type UnmappedFrames = Frames<{ MemoryState::Unmapped }>;

impl<const S: MemoryState> Frames<S> {
    /// 返回帧范围。
    #[inline]
    pub fn range(&self) -> FrameRange {
        self.range
    }

    /// 范围内帧的数量。
    #[inline]
    pub fn count(&self) -> usize {
        self.range.size()
    }

    /// 起始物理页号。
    #[inline]
    pub fn start(&self) -> PhysPageNum {
        self.range.start()
    }

    /// 结束物理页号（不含）。
    #[inline]
    pub fn end(&self) -> PhysPageNum {
        self.range.end()
    }

    /// 起始物理地址。
    #[inline]
    pub fn start_paddr(&self) -> PhysAddr {
        self.range.start().start_addr()
    }

    /// 消费 self，以新的状态 `NEW` 返回——typestate 转换的共享实现。
    ///
    /// `mem::forget` 阻止旧状态的 Drop 执行，新 `Frames` 继承帧范围。
    #[inline]
    fn into_state<const NEW: MemoryState>(self) -> Frames<NEW> {
        let range = self.range;
        core::mem::forget(self);
        Frames { range }
    }

    /// 在 `mid` 处分割为两段，消费 self。
    ///
    /// # Panics
    ///
    /// `mid` 不在范围内时 panic。
    pub fn split_at(self, mid: PhysPageNum) -> (Self, Self) {
        let (left, right) = self.range.split_at(mid);
        core::mem::forget(self);
        (Self { range: left }, Self { range: right })
    }

    /// 合并两个首尾相接的同状态帧范围，消费两者。
    ///
    /// 不相邻时返回 `Err` 归还两者所有权。
    pub fn merge(self, other: Self) -> Result<Self, (Self, Self)> {
        match self.range.merge(other.range) {
            Some(merged) => {
                core::mem::forget(self);
                core::mem::forget(other);
                Ok(Self { range: merged })
            }
            None => Err((self, other)),
        }
    }
}

impl AllocatedFrames {
    /// 分配一个物理帧（PAGE_SIZE 字节），内容清零。
    pub fn alloc_one() -> Result<Self, MemoryError> {
        Self::alloc(1)
    }

    /// 分配 `count` 个连续物理帧，内容清零。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, MemoryError> {
        let mut alloc = FRAME_ALLOCATOR.lock();
        if !alloc.initialized {
            return Err(MemoryError::AllocationFailed);
        }
        let frame_num = alloc
            .allocator
            .alloc(count)
            .ok_or(MemoryError::OutOfMemory)?;
        let start = PhysPageNum::new(frame_num);
        let end = PhysPageNum::new(frame_num + count);

        // SAFETY: 通过 phys_to_virt 将物理地址转换为虚拟地址后写入。
        // 帧刚从分配器获取，不存在其他引用。
        unsafe {
            let va = crate::phys_to_virt(start.start_addr());
            core::ptr::write_bytes(va.as_usize() as *mut u8, 0, count * PAGE_SIZE);
        }

        Ok(Self {
            range: FrameRange::new(start, end),
        })
    }

    /// 消费 Allocated 帧，转换为 Mapped 状态。
    ///
    /// 在帧被写入页表后调用。
    pub fn into_mapped(self) -> MappedFrames {
        self.into_state()
    }
}

impl MappedFrames {
    /// 消费 Mapped 帧，转换为 Unmapped 状态。
    ///
    /// 在帧从页表中 unmap 后调用。
    pub fn into_unmapped(self) -> UnmappedFrames {
        self.into_state()
    }
}

impl UnmappedFrames {
    /// 消费 Unmapped 帧，转换回 Allocated 状态。
    pub fn into_allocated(self) -> AllocatedFrames {
        self.into_state()
    }
}

/// 归还帧到分配器的共享逻辑。
fn dealloc_frames(range: &FrameRange) {
    if range.size() == 0 {
        return;
    }
    let mut alloc = FRAME_ALLOCATOR.lock();
    let start = range.start().as_usize();
    let count = range.size();
    alloc.allocator.dealloc(start, count);
}

impl<const S: MemoryState> Drop for Frames<S> {
    fn drop(&mut self) {
        if S == MemoryState::Mapped {
            panic!(
                "Frames<Mapped> dropped without unmapping — frames at {} leaked",
                self.range.start()
            );
        }
        dealloc_frames(&self.range);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 初始化测试用帧分配器——分配一块堆内存模拟物理内存区域。
    ///
    /// 全局 static 只能 init 一次，用 `std::sync::Once` 保证幂等。
    fn ensure_init() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let layout =
                std::alloc::Layout::from_size_align(64 * PAGE_SIZE, PAGE_SIZE).expect("layout");
            // SAFETY: layout 有效且非零大小
            let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
            assert!(!ptr.is_null());
            let start = PhysAddr::new(ptr as usize);
            // SAFETY: 测试专用内存区域，不与其他分配重叠
            unsafe { init(start, 64 * PAGE_SIZE) };
        });
    }

    /// 分配单帧后帧计数应为 1，地址应页对齐。
    #[test]
    fn alloc_one_frame() {
        ensure_init();
        let frame = AllocatedFrames::alloc_one().expect("alloc_one 应成功");
        assert_eq!(frame.count(), 1);
        assert!(frame.start_paddr().is_aligned());
    }

    /// 分配多帧后帧计数应正确。
    #[test]
    fn alloc_multiple_frames() {
        ensure_init();
        let frames = AllocatedFrames::alloc(4).expect("alloc(4) 应成功");
        assert_eq!(frames.count(), 4);
    }

    /// 帧 drop 后应能重新分配（归还到分配器）。
    #[test]
    fn alloc_dealloc_realloc() {
        ensure_init();
        let addr1 = {
            let frame = AllocatedFrames::alloc_one().expect("分配");
            frame.start_paddr()
        };
        // frame 已 drop，帧应归还
        let frame2 = AllocatedFrames::alloc_one().expect("重新分配应成功");
        // buddy allocator 不保证地址相同，但至少分配成功
        assert!(frame2.start_paddr().is_aligned());
        _ = addr1;
    }

    /// Allocated → Mapped → Unmapped 状态转换链。
    #[test]
    fn typestate_transitions() {
        ensure_init();
        let allocated = AllocatedFrames::alloc_one().expect("分配");
        let pa = allocated.start_paddr();

        let mapped = allocated.into_mapped();
        assert_eq!(mapped.start_paddr(), pa);

        let unmapped = mapped.into_unmapped();
        assert_eq!(unmapped.start_paddr(), pa);

        // Unmapped drop 自动归还分配器
    }

    /// Unmapped → Allocated 回转。
    #[test]
    fn unmapped_back_to_allocated() {
        ensure_init();
        let allocated = AllocatedFrames::alloc_one().expect("分配");
        let mapped = allocated.into_mapped();
        let unmapped = mapped.into_unmapped();
        let reallocated = unmapped.into_allocated();
        assert_eq!(reallocated.count(), 1);
    }

    /// split_at 应正确分割帧范围。
    #[test]
    fn split_frames() {
        ensure_init();
        let frames = AllocatedFrames::alloc(4).expect("alloc(4)");
        let mid = PhysPageNum::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        assert_eq!(left.count(), 2);
        assert_eq!(right.count(), 2);
    }

    /// merge 相邻帧应成功。
    #[test]
    fn merge_adjacent_frames() {
        ensure_init();
        let frames = AllocatedFrames::alloc(4).expect("alloc(4)");
        let mid = PhysPageNum::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        let merged = left
            .merge(right)
            .unwrap_or_else(|_| panic!("相邻帧 merge 应成功"));
        assert_eq!(merged.count(), 4);
    }

    /// Mapped 帧 drop 时应 panic。
    #[test]
    #[should_panic(expected = "Frames<Mapped> dropped without unmapping")]
    fn mapped_drop_panics() {
        ensure_init();
        let allocated = AllocatedFrames::alloc_one().expect("分配");
        let _mapped = allocated.into_mapped();
        // _mapped drop 时应 panic
    }
}
