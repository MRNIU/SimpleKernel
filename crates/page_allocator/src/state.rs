//! 虚拟页类型状态——`MemoryState` 枚举、`Pages<S>` 结构体、分配接口与 Drop。
//!
//! 与 frame_allocator 的 Frames<S, P> 对称设计，但只有两个有效状态：
//! - `Free`：在分配器空闲池中
//! - `Allocated`：被用户持有
//!
//! 虚拟页没有 Mapped/Unmapped 状态——映射生命周期由 MappedPages 管理。

use address::{PageRange, VirtAddr, VirtPageNum};

use crate::error::PageAllocError;

/// 虚拟页生命周期状态。
///
/// 状态机：`Free → Allocated → Free → …`
#[derive(PartialEq, Eq, core::marker::ConstParamTy)]
pub enum MemoryState {
    /// 空闲——在分配器池中
    Free,
    /// 已分配——用户持有
    Allocated,
}

/// 类型状态虚拟页范围——编译期追踪虚拟页生命周期。
pub struct Pages<const S: MemoryState> {
    pub(crate) range: PageRange,
}

/// 便利别名——已分配页，用户持有。
pub type AllocatedPages = Pages<{ MemoryState::Allocated }>;

impl<const S: MemoryState> Pages<S> {
    /// 返回页范围。
    #[inline]
    pub fn range(&self) -> PageRange {
        self.range
    }

    /// 范围内页的数量。
    #[inline]
    pub fn count(&self) -> usize {
        self.range.size()
    }

    /// 起始虚拟页号。
    #[inline]
    pub fn start(&self) -> VirtPageNum {
        self.range.start()
    }

    /// 结束虚拟页号（不含）。
    #[inline]
    pub fn end(&self) -> VirtPageNum {
        self.range.end()
    }

    /// 起始虚拟地址。
    #[inline]
    pub fn start_vaddr(&self) -> VirtAddr {
        self.range.start().start_addr()
    }

    /// 虚拟地址范围大小（字节）。
    #[inline]
    pub fn size_in_bytes(&self) -> usize {
        self.count() * config::PAGE_SIZE
    }
}

impl AllocatedPages {
    /// 自动分配 `count` 个连续虚拟页。
    pub fn alloc(count: usize) -> Result<Self, PageAllocError> {
        let range = crate::alloc_pages(count)?;
        Ok(Self { range })
    }

    /// 在指定虚拟地址分配 `count` 个连续页。
    pub fn alloc_at(start: VirtAddr, count: usize) -> Result<Self, PageAllocError> {
        let start_pn = VirtPageNum::from(start);
        let range = crate::alloc_pages_at(start_pn, count)?;
        Ok(Self { range })
    }

    /// 在 `page_index` 处拆分为两段，消费 self。
    ///
    /// # Panics
    /// `page_index` 为 0 或超出范围时 panic。
    pub fn split(self, page_index: usize) -> (Self, Self) {
        assert!(
            page_index > 0 && page_index < self.count(),
            "AllocatedPages::split: page_index 必须在 (0, count) 范围内"
        );
        let mid = self.range.start() + page_index;
        let (left, right) = self.range.split_at(mid);
        core::mem::forget(self);
        (Self { range: left }, Self { range: right })
    }

    /// 合并两个首尾相接的页范围，消费两者。
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

impl<const S: MemoryState> Drop for Pages<S> {
    fn drop(&mut self) {
        match S {
            MemoryState::Free | MemoryState::Allocated => {
                crate::dealloc_pages(self.range);
            }
        }
    }
}
