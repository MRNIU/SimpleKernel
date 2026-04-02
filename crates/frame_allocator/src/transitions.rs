//! 各状态专属的 impl 块——状态转换方法与分配接口。

use config::PAGE_SIZE;
use memory_types::PageSize;

use crate::FrameAllocError;
use crate::alloc::alloc_from_buddy;
use crate::state::{AllocatedFrames, FreeFrames};

impl FreeFrames {
    /// 消费 Free 帧，转换为 Allocated 状态。
    pub fn into_allocated(self) -> AllocatedFrames {
        self.into_state()
    }
}

impl<P: PageSize> AllocatedFrames<P> {
    /// 分配一个 P 大小的物理帧，内容清零。
    pub fn alloc_one() -> Result<Self, FrameAllocError> {
        Self::alloc(1)
    }

    /// 分配 `count` 个连续的 P 大小物理帧，内容清零。
    ///
    /// 内部路径：buddy allocator（4K 粒度）→ `FreeFrames` → `AllocatedFrames<P>`。
    /// 对于大页（P != Page4K），请求的 4K 帧数 = `count × P::NUM_4K_PAGES`。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
        let count_4k = count << P::NUM_4K_PAGES_SHIFT;
        let free = alloc_from_buddy(count_4k)?;

        // SAFETY: 通过 phys_to_virt 将物理地址转换为虚拟地址后写入。
        // 帧刚从分配器获取，不存在其他引用。
        unsafe {
            let ptr = memory_types::phys_to_virt(free.start_paddr()).as_mut_ptr::<u8>();
            core::ptr::write_bytes(ptr, 0, count_4k * PAGE_SIZE);
        }

        let range = free.range();
        core::mem::forget(free);

        assert!(
            P::NUM_4K_PAGES == 1 || range.start().as_usize() & (P::NUM_4K_PAGES - 1) == 0,
            "AllocatedFrames::alloc: buddy 返回的帧未对齐到 P 边界"
        );

        Ok(Self::from_range(range))
    }
}

#[cfg(test)]
mod tests {
    use memory_types::Frame;

    use crate::alloc::alloc_from_buddy;
    use crate::ensure_test_init;
    use crate::state::AllocatedFrames;

    type Alloc = AllocatedFrames;

    /// 分配单帧后帧计数应为 1，地址应页对齐。
    #[test]
    fn alloc_one_frame() {
        ensure_test_init();
        let frame = Alloc::alloc_one().expect("alloc_one 应成功");
        assert_eq!(frame.count(), 1);
        assert!(frame.start_paddr().is_aligned());
    }

    /// 分配多帧后帧计数应正确。
    #[test]
    fn alloc_multiple_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4) 应成功");
        assert_eq!(frames.count(), 4);
    }

    /// 帧 drop 后应能重新分配（归还到分配器）。
    #[test]
    fn alloc_dealloc_realloc() {
        ensure_test_init();
        {
            let _frame = Alloc::alloc_one().expect("分配");
        }
        // frame 已 drop，帧应归还
        let frame2 = Alloc::alloc_one().expect("重新分配应成功");
        assert!(frame2.start_paddr().is_aligned());
    }

    /// Free → Allocated 显式转换。
    #[test]
    fn free_into_allocated() {
        ensure_test_init();
        let free = alloc_from_buddy(1).expect("buddy 分配");
        let pa = free.start_paddr();
        let allocated = free.into_allocated();
        assert_eq!(allocated.start_paddr(), pa);
        assert_eq!(allocated.count(), 1);
    }

    /// split_at 应正确分割帧范围。
    #[test]
    fn split_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4)");
        let mid = Frame::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        assert_eq!(left.count(), 2);
        assert_eq!(right.count(), 2);
    }

    /// merge 相邻帧应成功。
    #[test]
    fn merge_adjacent_frames() {
        ensure_test_init();
        let frames = Alloc::alloc(4).expect("alloc(4)");
        let mid = Frame::new(frames.start().as_usize() + 2);
        let (left, right) = frames.split_at(mid);
        let merged = left
            .merge(right)
            .unwrap_or_else(|_| panic!("相邻帧 merge 应成功"));
        assert_eq!(merged.count(), 4);
    }

    /// 合并不相邻的帧应失败并归还双方所有权。
    #[test]
    fn merge_non_adjacent_fails() {
        ensure_test_init();
        let a = Alloc::alloc_one().expect("分配 a");
        let b = Alloc::alloc_one().expect("分配 b");
        // 两次独立分配的帧不一定相邻
        let a_start = a.start();
        let b_start = b.start();
        if a_start.as_usize().abs_diff(b_start.as_usize()) > 1 {
            match a.merge(b) {
                Err((returned_a, returned_b)) => {
                    assert_eq!(returned_a.start(), a_start);
                    assert_eq!(returned_b.start(), b_start);
                }
                Ok(_) => panic!("不相邻帧 merge 应失败"),
            }
        }
        // 即使相邻也不出错——测试 merge 本身不 panic
    }
}
