//! 帧类型状态定义——`MemoryState` 枚举、`Frames<S, P>` 结构体、通用操作与 Drop。
//!
//! 状态机：`Free -> Allocated -> Mapped -> Unmapped -> Free`。

use core::marker::PhantomData;

use memory_types::{FrameSpan, Page4K, PageSize, PhysAddr};

use crate::alloc::dealloc_to_backend;

/// 帧生命周期状态——追踪物理帧的**所有权**。
///
/// SAS 全量映射下所有帧始终有 identity mapping（背景层 kernel_rw）。
/// 状态不代表 PTE 是否存在，而代表谁持有帧：
///
/// ```text
/// Free -> Allocated -> Mapped -> Unmapped --> Free
///                                         \-> Allocated（重新映射）
/// ```
///
/// - `Free`：分配器持有，背景 kernel_rw 权限
/// - `Allocated`：用户持有，尚未通过 OwnedPages 管理权限
/// - `Mapped`：OwnedPages 持有，PTE flags 可能已被覆盖为非默认值
/// - `Unmapped`：从 OwnedPages 释放，PTE 已恢复为 kernel_rw，等待回收
///
/// `Mapped` 帧禁止直接 drop——必须先 unmap 转为 `Unmapped`。
/// 其余状态 Drop 时归还分配器。
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum MemoryState {
    /// 空闲——分配器持有，背景 kernel_rw 权限。Drop 归还分配器
    Free,
    /// 已分配——用户持有，尚未通过 OwnedPages 管理权限。Drop 归还分配器
    Allocated,
    /// 已映射——OwnedPages 持有，PTE flags 可能已被覆盖为非默认值。**Drop 会 panic**
    Mapped,
    /// 已解映射——PTE 已恢复为 kernel_rw，等待回收或重新映射。Drop 归还分配器
    Unmapped,
}

/// 类型状态帧范围——编译期追踪物理帧生命周期和页大小。
///
/// `Frames` 持有一段**连续的物理帧范围**（[`FrameSpan`]）。
/// 泛型参数 `P` 标记帧的粒度（4K/2M/1G），Drop 时自动转换为 4K 粒度归还分配器。
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
/// 便利别名——已映射帧，页表持有。Drop 会 panic。
pub type MappedFrames<P = Page4K> = Frames<{ MemoryState::Mapped }, P>;
/// 便利别名——已解映射帧，等待回收。
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

    /// 消费 self，同时变更状态和页大小标记。
    ///
    /// 用于 `FreeFrames`（4K）→ `AllocatedFrames<P>`（可能大页）的转换。
    /// 编译期无法检查 P2 的对齐约束，因此包含运行时断言。
    #[inline]
    pub(crate) fn into_state_and_size<const NEW: MemoryState, P2: PageSize>(
        self,
    ) -> Frames<NEW, P2> {
        assert!(
            P2::NUM_4K_PAGES == 1 || self.range.start().as_usize() & (P2::NUM_4K_PAGES - 1) == 0,
            "into_state_and_size: 帧范围起始 {} 未对齐到 {} 页边界",
            self.range.start(),
            P2::NAME
        );
        let range = self.range;
        core::mem::forget(self);
        Frames {
            range,
            _marker: PhantomData,
        }
    }
}

impl<const S: MemoryState, P: PageSize> Drop for Frames<S, P> {
    fn drop(&mut self) {
        match S {
            MemoryState::Mapped => {
                panic!(
                    "Frames<Mapped> dropped without unmap! range: {}-{}. \
                     必须先调用 into_unmapped() 再释放",
                    self.range.start(),
                    self.range.end()
                );
            }
            MemoryState::Free | MemoryState::Allocated | MemoryState::Unmapped => {
                dealloc_to_backend(self.range);
            }
        }
    }
}
