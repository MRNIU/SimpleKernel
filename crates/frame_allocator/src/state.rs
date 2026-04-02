//! 帧类型状态定义——`MemoryState` 枚举、`Frames<S, P>` 结构体、通用操作与 Drop。

use core::marker::PhantomData;

use memory_types::{Frame, FrameSpan, Page4K, PageSize, PhysAddr};

use crate::alloc::dealloc_to_buddy;

/// 帧生命周期状态。
///
/// 状态机：`Free → Allocated → Mapped → Unmapped → Free → …`
///
/// 所有解分配路径最终汇聚到 `Free` 状态，由 `Free` 的 Drop 统一归还
/// buddy allocator。
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

/// 类型状态帧范围——编译期追踪物理帧生命周期和页大小。
///
/// 与单帧设计不同，`Frames` 持有一段**连续的物理帧范围**（[`FrameSpan`]），
/// 支持 [`split_at`](Frames::split_at) 和 [`merge`](Frames::merge) 操作。
///
/// 泛型参数 `P` 标记帧的粒度（4K/2M/1G），Drop 时自动转换为 4K 粒度
/// 归还 buddy allocator。
///
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧。
pub struct Frames<const S: MemoryState, P: PageSize = Page4K> {
    pub(crate) range: FrameSpan,
    _marker: PhantomData<P>,
}

impl<const S: MemoryState, P: PageSize> core::fmt::Debug for Frames<S, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Frames({}-{})", self.range.start(), self.range.end())
    }
}

/// 便利别名——空闲帧，分配器内部持有。始终 4K 粒度。
pub type FreeFrames = Frames<{ MemoryState::Free }, Page4K>;
/// 便利别名——已分配帧，用户持有。
pub type AllocatedFrames<P = Page4K> = Frames<{ MemoryState::Allocated }, P>;
/// 便利别名——已映射帧，页表持有。
pub type MappedFrames<P = Page4K> = Frames<{ MemoryState::Mapped }, P>;
/// 便利别名——已取消映射帧，等待回收。
pub type UnmappedFrames<P = Page4K> = Frames<{ MemoryState::Unmapped }, P>;

impl<const S: MemoryState, P: PageSize> Frames<S, P> {
    /// 返回帧范围（4K 粒度）。
    #[inline]
    pub fn range(&self) -> FrameSpan {
        self.range
    }

    /// 范围内 4K 帧的数量。
    #[inline]
    pub fn count(&self) -> usize {
        self.range.size()
    }

    /// 起始物理页号（4K 粒度）。
    #[inline]
    pub fn start(&self) -> Frame {
        self.range.start()
    }

    /// 结束物理页号（不含，4K 粒度）。
    #[inline]
    pub fn end(&self) -> Frame {
        self.range.end()
    }

    /// 起始物理地址。
    #[inline]
    pub fn start_paddr(&self) -> PhysAddr {
        self.range.start().start_addr()
    }

    /// 从 FrameSpan 和 PageSize 标记构造（crate 内部使用）。
    #[inline]
    pub(crate) fn from_range(range: FrameSpan) -> Self {
        Self {
            range,
            _marker: PhantomData,
        }
    }

    /// 消费 self，以新的状态 `NEW` 返回——typestate 转换的共享实现。
    ///
    /// `mem::forget` 阻止旧状态的 Drop 执行，新 `Frames` 继承帧范围。
    #[inline]
    pub(crate) fn into_state<const NEW: MemoryState>(self) -> Frames<NEW, P> {
        let range = self.range;
        core::mem::forget(self);
        Frames {
            range,
            _marker: PhantomData,
        }
    }

    /// 在 `mid` 处分割为两段，消费 self。
    ///
    /// # Panics
    ///
    /// `mid` 不在范围内时 panic。
    pub fn split_at(self, mid: Frame<P>) -> (Self, Self) {
        let mid_4k = Frame::new(mid.as_usize());
        let (left, right) = self.range.split_at(mid_4k);
        core::mem::forget(self);
        (
            Self {
                range: left,
                _marker: PhantomData,
            },
            Self {
                range: right,
                _marker: PhantomData,
            },
        )
    }

    /// 合并两个首尾相接的同状态帧范围，消费两者。
    ///
    /// 不相邻时返回 `Err` 归还两者所有权。
    pub fn merge(self, other: Self) -> Result<Self, (Self, Self)> {
        match self.range.merge(other.range) {
            Some(merged) => {
                core::mem::forget(self);
                core::mem::forget(other);
                Ok(Self {
                    range: merged,
                    _marker: PhantomData,
                })
            }
            None => Err((self, other)),
        }
    }
}

impl<const S: MemoryState, P: PageSize> Drop for Frames<S, P> {
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
