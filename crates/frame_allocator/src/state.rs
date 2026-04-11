//! 帧类型状态定义——`FrameState` 枚举、`Frames<S>` 结构体、通用操作与 Drop。
//!
//! 状态机：`Free -> Allocated -> Free`（ADR-006 简化为 2-state）。

use memory_types::PhysAddr;

use crate::FrameSpan;

use crate::alloc::dealloc_to_backend;

/// 帧生命周期状态——追踪物理帧的**所有权**。
///
/// SAS 全量映射下所有帧始终有 identity mapping（背景层 kernel_rw）。
/// 状态不代表 PTE 是否存在，而代表谁持有帧：
///
/// ```text
/// Free -> Allocated -> Free
/// ```
///
/// - `Free`：分配器持有，背景 kernel_rw 权限
/// - `Allocated`：用户持有，可能通过 OwnedPages 管理权限
///
/// ADR-006 移除了 `Mapped`/`Unmapped` 状态——SAS 全量映射下 PTE 始终存在，
/// 这两个状态不对应任何硬件状态变化。
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum FrameState {
    /// 空闲——分配器持有，背景 kernel_rw 权限。Drop 归还分配器
    Free,
    /// 已分配——用户持有。Drop 归还分配器
    Allocated,
}

/// 类型状态帧范围——编译期追踪物理帧生命周期。
///
/// `Frames` 持有一段**连续的物理帧范围**（[`FrameSpan`]）。
/// 状态转换通过消费 self 的方法实现，防止在错误状态下操作帧。
pub struct Frames<const S: FrameState> {
    pub(crate) range: FrameSpan,
}

impl<const S: FrameState> core::fmt::Debug for Frames<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Frames({}-{})", self.range.start(), self.range.end())
    }
}

/// 便利别名——空闲帧，分配器内部持有。
pub type FreeFrames = Frames<{ FrameState::Free }>;
/// 便利别名——已分配帧，用户持有。
pub type AllocatedFrames = Frames<{ FrameState::Allocated }>;

impl<const S: FrameState> Frames<S> {
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

    /// 从 FrameSpan 构造（crate 内部使用）。
    #[inline]
    pub(crate) fn from_range(range: FrameSpan) -> Self {
        Self { range }
    }

    /// 消费 self，以新的状态 `NEW` 返回。
    #[inline]
    pub(crate) fn into_state<const NEW: FrameState>(self) -> Frames<NEW> {
        let range = self.range;
        core::mem::forget(self);
        Frames { range }
    }
}

impl<const S: FrameState> Drop for Frames<S> {
    fn drop(&mut self) {
        dealloc_to_backend(self.range);
    }
}
