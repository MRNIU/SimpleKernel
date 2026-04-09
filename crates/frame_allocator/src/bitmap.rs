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
