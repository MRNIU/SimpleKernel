// TODO: 待 adt_const_params 稳定后，迁移为 const generic enum 方案：
//   enum MemoryState { Free, Allocated, Mapped }
//   struct Frames<const S: MemoryState> { ... }
// 当前 nightly (2026-03-24) 的 adt_const_params 与分状态 Drop impl 存在
// 循环依赖 ICE，因此使用 sealed trait + PhantomData 作为等价替代。

use crate::address::PhysAddr;
use crate::error::MemoryError;
use config::PAGE_SIZE;
use core::marker::PhantomData;
use sync_crate::SpinLock;

/// 全局帧分配器，以页帧（PAGE_SIZE 字节）为单位管理物理内存。
static FRAME_ALLOCATOR: SpinLock<FrameAllocatorInner> =
    SpinLock::new(FrameAllocatorInner::new(), "frame_alloc");

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
    // 防止可分配区域地址回绕（例如内核镜像过大导致 start 超出物理内存范围）
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

mod sealed {
    pub trait Sealed {}
}

/// 帧生命周期状态 trait（sealed，外部不可实现）。
///
/// 临时方案：用 sealed trait 的关联常量模拟 const generic enum 的按状态 Drop 分派。
/// 待 `adt_const_params` 稳定后迁移为 `Frames<const S: MemoryState>`。
pub trait FrameState: sealed::Sealed {
    /// drop 时是否归还帧到分配器
    const DEALLOC_ON_DROP: bool;
    /// drop 时是否 panic（检测"帧还在页表中就被释放"的 bug）
    const PANIC_ON_DROP: bool;
}

/// 已分配、尚未映射到页表的帧。
pub struct Allocated;
impl sealed::Sealed for Allocated {}
impl FrameState for Allocated {
    const DEALLOC_ON_DROP: bool = true;
    const PANIC_ON_DROP: bool = false;
}

/// 已映射到页表中的帧——必须先 unmap 再释放。
pub struct Mapped;
impl sealed::Sealed for Mapped {}
impl FrameState for Mapped {
    // release 兜底释放，避免泄漏
    const DEALLOC_ON_DROP: bool = true;
    // debug 构建 panic 报告 bug
    const PANIC_ON_DROP: bool = true;
}

/// 类型状态帧守卫——编译期追踪物理帧生命周期。
///
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧：
/// - `Allocated` → `Mapped`：[`Frames::into_mapped`]
/// - `Mapped` → `Allocated`：[`Frames::into_unmapped`]
pub struct Frames<S: FrameState> {
    paddr: PhysAddr,
    _state: PhantomData<S>,
}

/// 便利别名。
pub type AllocatedFrame = Frames<Allocated>;
/// 便利别名。
pub type MappedFrame = Frames<Mapped>;

impl<S: FrameState> Frames<S> {
    /// 返回该帧的物理地址。
    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }
}

impl Frames<Allocated> {
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
        // 若未来切换为非 identity mapping，此处需通过 phys_to_virt() 转换。
        unsafe {
            core::ptr::write_bytes(paddr.as_usize() as *mut u8, 0, PAGE_SIZE);
        }

        Ok(Self {
            paddr,
            _state: PhantomData,
        })
    }

    /// 消费 Allocated 帧，转换为 Mapped 状态。
    ///
    /// 在帧被写入页表后调用。Mapped 帧若被意外 drop 会在 debug 构建 panic。
    pub fn into_mapped(self) -> Frames<Mapped> {
        let paddr = self.paddr;
        core::mem::forget(self);
        Frames {
            paddr,
            _state: PhantomData,
        }
    }
}

impl Frames<Mapped> {
    /// 消费 Mapped 帧，转换回 Allocated 状态。
    ///
    /// 在帧从页表中 unmap 后调用。返回的 Allocated 帧 drop 时正常归还分配器。
    pub fn into_unmapped(self) -> Frames<Allocated> {
        let paddr = self.paddr;
        core::mem::forget(self);
        Frames {
            paddr,
            _state: PhantomData,
        }
    }
}

impl<S: FrameState> Drop for Frames<S> {
    fn drop(&mut self) {
        if S::PANIC_ON_DROP {
            debug_assert!(
                false,
                "Frames<Mapped> dropped without unmapping — frame at {} leaked",
                self.paddr
            );
        }
        if S::DEALLOC_ON_DROP {
            let mut alloc = FRAME_ALLOCATOR.lock();
            let frame_num = self.paddr.as_usize() / PAGE_SIZE;
            alloc.allocator.dealloc(frame_num, 1);
        }
    }
}
