//! 帧类型状态定义——`MemoryState` 枚举、`Frames<S>` 结构体、通用操作与 Drop。

use address::{FrameRange, PhysAddr, PhysPageNum};

use crate::alloc::dealloc_to_buddy;

/// 帧生命周期状态——参考 Theseus OS 的状态模型。
///
/// 状态机：`Free → Allocated → Mapped → Unmapped → Free → …`
///
/// 所有解分配路径最终汇聚到 `Free` 状态，由 `Free` 的 Drop 统一归还
/// buddy allocator。这使状态机在类型系统中完全闭环。
///
/// 参考 Theseus: `kernel/frame_allocator/src/lib.rs` 的 `FreeFrames` 类型。
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum MemoryState {
    /// 空闲——被分配器持有，Drop 归还 buddy allocator
    Free,
    /// 已分配——用户持有，Drop 经由 `Free` 归还分配器
    Allocated,
    /// 已映射到页表——Drop panic（必须先 unmap）
    Mapped,
    /// 从页表移除——Drop 经由 `Free` 归还分配器
    Unmapped,
}

/// 类型状态帧范围——编译期追踪物理帧生命周期。
///
/// 与单帧设计不同，`Frames` 持有一段**连续的物理帧范围**（[`FrameRange`]），
/// 支持 [`split_at`](Frames::split_at) 和 [`merge`](Frames::merge) 操作。
///
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧。
pub struct Frames<const S: MemoryState> {
    pub(crate) range: FrameRange,
}

/// 便利别名——空闲帧，分配器内部持有。
pub type FreeFrames = Frames<{ MemoryState::Free }>;
/// 便利别名——已分配帧，用户持有。
pub type AllocatedFrames = Frames<{ MemoryState::Allocated }>;
/// 便利别名——已映射帧，页表持有。
pub type MappedFrames = Frames<{ MemoryState::Mapped }>;
/// 便利别名——已取消映射帧，等待回收。
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
    pub(crate) fn into_state<const NEW: MemoryState>(self) -> Frames<NEW> {
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

impl<const S: MemoryState> Drop for Frames<S> {
    fn drop(&mut self) {
        match S {
            MemoryState::Free | MemoryState::Allocated | MemoryState::Unmapped => {
                dealloc_to_buddy(self.range);
            }
            // Mapped 帧必须先 unmap，直接 drop 说明存在泄漏
            MemoryState::Mapped => {
                panic!(
                    "Frames<Mapped> dropped without unmapping — frames at {} leaked",
                    self.range.start()
                );
            }
        }
    }
}
