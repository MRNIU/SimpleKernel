//! 物理帧分配器 + typestate 生命周期追踪。
//!
//! 使用 `adt_const_params` 实现 `Frames<const S: MemoryState>`，
//! 通过 const generic enum 按状态分派 Drop 行为。

use crate::error::MemoryError;
use address::PhysAddr;
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

/// 帧生命周期状态——参考 Theseus OS 的 `MemoryState` 设计。
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum MemoryState {
    /// 已分配、尚未映射到页表
    Allocated,
    /// 已映射到页表中——必须先 unmap 再释放
    Mapped,
}

/// 类型状态帧守卫——编译期追踪物理帧生命周期。
///
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧：
/// - `Allocated` → `Mapped`：[`Frames::into_mapped`]
/// - `Mapped` → `Allocated`：[`Frames::into_unmapped`]
pub struct Frames<const S: MemoryState> {
    paddr: PhysAddr,
}

/// 便利别名。
pub type AllocatedFrame = Frames<{ MemoryState::Allocated }>;
/// 便利别名。
pub type MappedFrame = Frames<{ MemoryState::Mapped }>;

impl<const S: MemoryState> Frames<S> {
    /// 返回该帧的物理地址。
    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }
}

impl Frames<{ MemoryState::Allocated }> {
    /// 分配一个物理帧（PAGE_SIZE 字节），内容清零。
    pub fn alloc() -> Result<Self, MemoryError> {
        let mut alloc = FRAME_ALLOCATOR.lock();
        if !alloc.initialized {
            return Err(MemoryError::AllocationFailed);
        }
        let frame_num = alloc.allocator.alloc(1).ok_or(MemoryError::OutOfMemory)?;
        let paddr = PhysAddr::new(frame_num * PAGE_SIZE);

        // SAFETY: 当前使用 identity mapping（VA == PA），物理地址可直接作为虚拟地址访问。
        // 帧刚从分配器获取，不存在其他引用。
        unsafe {
            core::ptr::write_bytes(paddr.as_usize() as *mut u8, 0, PAGE_SIZE);
        }

        Ok(Self { paddr })
    }

    /// 消费 Allocated 帧，转换为 Mapped 状态。
    ///
    /// 在帧被写入页表后调用。Mapped 帧若被意外 drop 会在 debug 构建 panic。
    pub fn into_mapped(self) -> Frames<{ MemoryState::Mapped }> {
        let paddr = self.paddr;
        core::mem::forget(self);
        Frames { paddr }
    }
}

impl Frames<{ MemoryState::Mapped }> {
    /// 消费 Mapped 帧，转换回 Allocated 状态。
    ///
    /// 在帧从页表中 unmap 后调用。返回的 Allocated 帧 drop 时正常归还分配器。
    pub fn into_unmapped(self) -> Frames<{ MemoryState::Allocated }> {
        let paddr = self.paddr;
        core::mem::forget(self);
        Frames { paddr }
    }
}

impl<const S: MemoryState> Drop for Frames<S> {
    fn drop(&mut self) {
        match S {
            MemoryState::Mapped => {
                debug_assert!(
                    false,
                    "Frames<Mapped> dropped without unmapping — frame at {} leaked",
                    self.paddr
                );
            }
            MemoryState::Allocated => {}
        }
        // 两种状态都归还帧：Allocated 正常释放，Mapped 兜底防泄漏
        let mut alloc = FRAME_ALLOCATOR.lock();
        let frame_num = self.paddr.as_usize() / PAGE_SIZE;
        alloc.allocator.dealloc(frame_num, 1);
    }
}
