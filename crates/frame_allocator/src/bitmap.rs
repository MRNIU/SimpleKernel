//! Bitmap 帧分配器——外部元数据，不触碰空闲帧内存。
//!
//! 每个 4K 帧对应 1 bit（0=空闲，1=已分配），bitmap 存储在堆上。
//! alloc/dealloc 只操作 bitmap，不读写帧的物理地址——
//! 分配器元数据与帧内容完全解耦，帧内容由所有者管理。
//!
//! 性能：alloc 为 O(n) word 级扫描（`trailing_zeros`），
//! dealloc 为 O(1) 位操作。对教学内核足够，
//! 未来如需 O(log n) 可替换为 buddy+bitmap。

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::backend::FrameAllocBackend;

/// Bitmap 帧分配器。
///
/// `bits[i]` 的第 `j` 位（LSB=0）对应帧号 `base_frame + i*64 + j`。
/// 0 = 空闲，1 = 已分配。
pub(crate) struct BitmapAllocator {
    bits: Vec<u64>,
    base_frame: usize,
    total_frames: usize,
}

impl BitmapAllocator {
    pub(crate) const fn new() -> Self {
        Self {
            bits: Vec::new(),
            base_frame: 0,
            total_frames: 0,
        }
    }

    /// 帧号转 bitmap 内偏移（word 下标, bit 下标）。
    #[inline]
    fn frame_to_index(&self, frame: usize) -> (usize, usize) {
        let offset = frame - self.base_frame;
        (offset / 64, offset % 64)
    }
}

impl FrameAllocBackend for BitmapAllocator {
    fn add_frames(&mut self, start: usize, end: usize) {
        assert!(start < end, "BitmapAllocator::add_frames: start >= end");

        if self.total_frames == 0 {
            // 首次初始化
            self.base_frame = start;
            self.total_frames = end - start;
            let words = (self.total_frames + 63) / 64;
            self.bits = vec![0u64; words];
        } else {
            // 扩展——只支持向高地址扩展
            assert!(
                start >= self.base_frame,
                "BitmapAllocator::add_frames: 不支持向低地址扩展"
            );
            let new_end_offset = end - self.base_frame;
            if new_end_offset > self.total_frames {
                let new_words = (new_end_offset + 63) / 64;
                self.bits.resize(new_words, 0);
                // 标记 [旧 total_frames, start) 为已分配（空洞区域）
                for f in self.total_frames..(start - self.base_frame) {
                    let (w, b) = (f / 64, f % 64);
                    self.bits[w] |= 1u64 << b;
                }
                self.total_frames = new_end_offset;
            }
            // 确保 [start, end) 标记为空闲
            for f in (start - self.base_frame)..(end - self.base_frame) {
                let (w, b) = (f / 64, f % 64);
                self.bits[w] &= !(1u64 << b);
            }
        }
    }

    fn alloc(&mut self, count: usize) -> Option<usize> {
        if count == 0 || count > self.total_frames {
            return None;
        }

        // 单帧快速路径——找第一个含空闲位的 word
        if count == 1 {
            for (wi, word) in self.bits.iter_mut().enumerate() {
                if *word != u64::MAX {
                    let bit = (!*word).trailing_zeros() as usize;
                    let frame_offset = wi * 64 + bit;
                    if frame_offset < self.total_frames {
                        *word |= 1u64 << bit;
                        return Some(self.base_frame + frame_offset);
                    }
                }
            }
            return None;
        }

        // 多帧路径——扫描连续空闲位
        let mut run_start = 0usize;
        let mut run_len = 0usize;

        for offset in 0..self.total_frames {
            let (w, b) = (offset / 64, offset % 64);
            if self.bits[w] & (1u64 << b) == 0 {
                if run_len == 0 {
                    run_start = offset;
                }
                run_len += 1;
                if run_len == count {
                    // 找到——标记为已分配
                    for i in run_start..run_start + count {
                        let (wi, bi) = (i / 64, i % 64);
                        self.bits[wi] |= 1u64 << bi;
                    }
                    return Some(self.base_frame + run_start);
                }
            } else {
                run_len = 0;
            }
        }

        None
    }

    fn dealloc(&mut self, start: usize, count: usize) {
        for i in 0..count {
            let (w, b) = self.frame_to_index(start + i);
            debug_assert!(
                self.bits[w] & (1u64 << b) != 0,
                "BitmapAllocator::dealloc: 帧 {} 未分配（double free?），word={}, bit={}",
                start + i,
                w,
                b
            );
            self.bits[w] &= !(1u64 << b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_allocator(start: usize, end: usize) -> BitmapAllocator {
        let mut a = BitmapAllocator::new();
        a.add_frames(start, end);
        a
    }

    /// add_frames 基本初始化。
    #[test]
    fn add_frames_basic() {
        let a = make_allocator(100, 200);
        assert_eq!(a.base_frame, 100);
        assert_eq!(a.total_frames, 100);
    }

    /// add_frames 扩展——空洞区域标记为已分配。
    #[test]
    fn add_frames_extend_with_gap() {
        let mut a = make_allocator(0, 10);
        a.add_frames(20, 30);
        assert_eq!(a.total_frames, 30);
        // 空洞 [10,20) 应不可分配
        for _ in 0..10 {
            a.alloc(1).expect("前 10 帧应可分配");
        }
        for _ in 0..10 {
            a.alloc(1).expect("后 10 帧应可分配");
        }
        assert!(a.alloc(1).is_none(), "空洞区域不应可分配");
    }

    /// 单帧分配与释放。
    #[test]
    fn alloc_dealloc_single() {
        let mut a = make_allocator(0, 64);
        let f = a.alloc(1).expect("应成功");
        assert_eq!(f, 0);
        a.dealloc(f, 1);
        let f2 = a.alloc(1).expect("释放后应可重新分配");
        assert_eq!(f2, 0);
    }

    /// 多帧连续分配。
    #[test]
    fn alloc_multi_contiguous() {
        let mut a = make_allocator(0, 128);
        let f = a.alloc(4).expect("应分配 4 帧");
        assert_eq!(f, 0);
        let f2 = a.alloc(4).expect("应分配后续 4 帧");
        assert_eq!(f2, 4);
    }

    /// 碎片化后多帧分配——需要找到连续空闲区域。
    #[test]
    fn alloc_multi_fragmented() {
        let mut a = make_allocator(0, 10);
        let f0 = a.alloc(1).expect("帧 0");
        let f1 = a.alloc(1).expect("帧 1");
        let _f2 = a.alloc(1).expect("帧 2");
        let f3 = a.alloc(1).expect("帧 3");
        // 释放 0,1,3 制造碎片：空闲=[0,1], [3], [4..9]
        a.dealloc(f0, 1);
        a.dealloc(f1, 1);
        a.dealloc(f3, 1);
        // 请求 3 个连续帧——[0,1] 不够，[3] 不够，应从 [4..9] 分配
        let f = a.alloc(3).expect("应找到连续 3 帧");
        assert!(f >= 4, "应跳过碎片区域，实际分配起始: {f}");
    }

    /// OOM——分配耗尽。
    #[test]
    fn alloc_oom() {
        let mut a = make_allocator(0, 3);
        a.alloc(2).expect("前 2 帧");
        a.alloc(1).expect("第 3 帧");
        assert!(a.alloc(1).is_none(), "应 OOM");
    }

    /// alloc(0) 返回 None。
    #[test]
    fn alloc_zero() {
        let mut a = make_allocator(0, 10);
        assert!(a.alloc(0).is_none());
    }

    /// 请求超过总帧数。
    #[test]
    fn alloc_exceeds_total() {
        let mut a = make_allocator(0, 5);
        assert!(a.alloc(6).is_none());
    }

    /// 跨 word 边界的多帧分配（确保不限于单个 u64）。
    #[test]
    fn alloc_cross_word_boundary() {
        let mut a = make_allocator(0, 128);
        // 占满前 62 帧，留 [62,63] 空闲（word 0 末尾）+ [64..] 空闲（word 1）
        for i in 0..62 {
            let f = a.alloc(1).expect("逐帧分配");
            assert_eq!(f, i);
        }
        // 请求 4 帧连续——应跨 word 边界 [62,63,64,65]
        let f = a.alloc(4).expect("跨 word 边界分配");
        assert_eq!(f, 62);
    }

    /// dealloc 后帧可重新分配。
    #[test]
    fn dealloc_reuse() {
        let mut a = make_allocator(0, 2);
        let f = a.alloc(2).expect("分配 2 帧");
        assert!(a.alloc(1).is_none());
        a.dealloc(f, 2);
        let f2 = a.alloc(2).expect("释放后应可重新分配");
        assert_eq!(f2, f);
    }

    /// double free 在 debug 模式下应 panic。
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "未分配")]
    fn dealloc_double_free_panics() {
        let mut a = make_allocator(0, 10);
        let f = a.alloc(1).expect("分配");
        a.dealloc(f, 1);
        a.dealloc(f, 1);
    }
}
