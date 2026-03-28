//! 连续地址范围 [`AddrRange<A>`]——支持包含判断、重叠检测、分割、合并和迭代。

use core::ops::Sub;

/// 半开区间 `[start, end)`，表示一段连续的地址空间。
///
/// 泛型参数 `A` 可以是 [`PhysAddr`](crate::PhysAddr)、
/// [`VirtAddr`](crate::VirtAddr) 或任何满足约束的地址类型。
///
/// # Panics
/// 构造时 `start > end` 会 panic。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddrRange<A> {
    start: A,
    end: A,
}

impl<A: Copy + Ord> AddrRange<A> {
    /// 构造地址范围 `[start, end)`
    ///
    /// # Panics
    /// `start > end` 时 panic。
    #[inline]
    pub fn new(start: A, end: A) -> Self {
        assert!(start <= end, "AddrRange: start must be <= end");
        Self { start, end }
    }

    /// 范围起始地址
    #[inline]
    pub fn start(self) -> A {
        self.start
    }

    /// 范围结束地址（不含）
    #[inline]
    pub fn end(self) -> A {
        self.end
    }

    /// 地址是否在范围内
    #[inline]
    pub fn contains(self, addr: A) -> bool {
        self.start <= addr && addr < self.end
    }

    /// 两个范围是否重叠
    #[inline]
    pub fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// 是否为空范围
    #[inline]
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

impl<A: Copy + Ord + Sub<A, Output = usize>> AddrRange<A> {
    /// 范围大小（字节数或页数，取决于 `A` 的单位）
    #[inline]
    pub fn size(self) -> usize {
        self.end - self.start
    }
}

impl<A: Copy + Ord + core::ops::Add<usize, Output = A>> AddrRange<A> {
    /// 在 `mid` 处分割为 `[start, mid)` 和 `[mid, end)`。
    ///
    /// # Panics
    /// `mid` 不在 `[start, end]` 范围内时 panic。
    #[inline]
    pub fn split_at(self, mid: A) -> (Self, Self) {
        assert!(
            self.start <= mid && mid <= self.end,
            "AddrRange::split_at: mid out of range"
        );
        (Self::new(self.start, mid), Self::new(mid, self.end))
    }

    /// 是否与 `other` 首尾相接（`self.end == other.start`）。
    #[inline]
    pub fn contiguous_with(self, other: Self) -> bool {
        self.end == other.start
    }

    /// 合并两个首尾相接的范围；不相接时返回 `None`。
    #[inline]
    pub fn merge(self, other: Self) -> Option<Self> {
        if self.contiguous_with(other) {
            Some(Self::new(self.start, other.end))
        } else if other.contiguous_with(self) {
            Some(Self::new(other.start, self.end))
        } else {
            None
        }
    }

    /// 逐元素迭代（步长为 1）。
    #[inline]
    pub fn iter(self) -> AddrRangeIter<A> {
        AddrRangeIter {
            current: self.start,
            end: self.end,
        }
    }
}

/// [`AddrRange`] 的逐元素迭代器。
pub struct AddrRangeIter<A> {
    current: A,
    end: A,
}

impl<A: Copy + Ord + core::ops::Add<usize, Output = A>> Iterator for AddrRangeIter<A> {
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let item = self.current;
            self.current = self.current + 1;
            Some(item)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PhysAddr, VirtAddr};

    /// 构造、size、contains、overlaps。
    #[test]
    fn basic() {
        let r = AddrRange::new(PhysAddr::new(0x1000), PhysAddr::new(0x3000));
        assert_eq!(r.start(), PhysAddr::new(0x1000));
        assert_eq!(r.end(), PhysAddr::new(0x3000));
        assert_eq!(r.size(), 0x2000);
        assert!(!r.is_empty());

        assert!(r.contains(PhysAddr::new(0x1000)));
        assert!(r.contains(PhysAddr::new(0x2FFF)));
        assert!(!r.contains(PhysAddr::new(0x3000)));
        assert!(!r.contains(PhysAddr::new(0x0FFF)));
    }

    /// 空范围：start == end。
    #[test]
    fn empty() {
        let r = AddrRange::new(PhysAddr::new(0x1000), PhysAddr::new(0x1000));
        assert!(r.is_empty());
        assert_eq!(r.size(), 0);
        assert!(!r.contains(PhysAddr::new(0x1000)));
    }

    /// 重叠检测：相交为 true，相邻为 false。
    #[test]
    fn overlaps() {
        let a = AddrRange::new(VirtAddr::new(0x1000), VirtAddr::new(0x3000));
        let b = AddrRange::new(VirtAddr::new(0x2000), VirtAddr::new(0x4000));
        let c = AddrRange::new(VirtAddr::new(0x3000), VirtAddr::new(0x4000));
        let d = AddrRange::new(VirtAddr::new(0x0000), VirtAddr::new(0x1000));

        assert!(a.overlaps(b));
        assert!(!a.overlaps(c));
        assert!(!a.overlaps(d));
    }

    /// 构造时 start > end 应 panic。
    #[test]
    #[should_panic(expected = "start must be <= end")]
    fn invalid_panics() {
        let _ = AddrRange::new(PhysAddr::new(0x2000), PhysAddr::new(0x1000));
    }

    /// split_at 将范围一分为二。
    #[test]
    fn split_at_middle() {
        use crate::PhysPageNum;
        let r = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(4));
        let (left, right) = r.split_at(PhysPageNum::new(2));
        assert_eq!(left.size(), 2);
        assert_eq!(right.size(), 2);
        assert_eq!(left.end(), right.start());
    }

    /// split_at 边界：在 start 或 end 处分割产生空范围。
    #[test]
    fn split_at_boundary() {
        use crate::PhysPageNum;
        let r = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(4));
        let (left, right) = r.split_at(PhysPageNum::new(0));
        assert!(left.is_empty());
        assert_eq!(right.size(), 4);

        let (left, right) = r.split_at(PhysPageNum::new(4));
        assert_eq!(left.size(), 4);
        assert!(right.is_empty());
    }

    /// merge 合并两个相邻范围。
    #[test]
    fn merge_contiguous() {
        use crate::PhysPageNum;
        let a = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(2));
        let b = AddrRange::new(PhysPageNum::new(2), PhysPageNum::new(5));
        let merged = a.merge(b).expect("应能合并");
        assert_eq!(merged.start(), PhysPageNum::new(0));
        assert_eq!(merged.end(), PhysPageNum::new(5));
    }

    /// merge 不相邻范围返回 None。
    #[test]
    fn merge_non_contiguous() {
        use crate::PhysPageNum;
        let a = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(2));
        let b = AddrRange::new(PhysPageNum::new(3), PhysPageNum::new(5));
        assert!(a.merge(b).is_none());
    }

    /// iter 逐元素迭代。
    #[test]
    fn iter_pages() {
        use crate::PhysPageNum;
        let r = AddrRange::new(PhysPageNum::new(10), PhysPageNum::new(13));
        let pages: Vec<_> = r.iter().collect();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0], PhysPageNum::new(10));
        assert_eq!(pages[2], PhysPageNum::new(12));
    }

    /// 空范围迭代应产生 0 个元素。
    #[test]
    fn iter_empty() {
        use crate::PhysPageNum;
        let r = AddrRange::new(PhysPageNum::new(5), PhysPageNum::new(5));
        assert_eq!(r.iter().count(), 0);
    }

    /// split_at 越界应 panic。
    #[test]
    #[should_panic(expected = "mid out of range")]
    fn split_at_out_of_range() {
        use crate::PhysPageNum;
        let r = AddrRange::new(PhysPageNum::new(2), PhysPageNum::new(4));
        let _ = r.split_at(PhysPageNum::new(5));
    }

    /// merge 反序（b 在 a 前面）也能合并。
    #[test]
    fn merge_reversed_order() {
        use crate::PhysPageNum;
        let a = AddrRange::new(PhysPageNum::new(3), PhysPageNum::new(5));
        let b = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(3));
        let merged = a.merge(b).expect("反序也应能合并");
        assert_eq!(merged.start(), PhysPageNum::new(0));
        assert_eq!(merged.end(), PhysPageNum::new(5));
    }

    /// contiguous_with 判断首尾相接。
    #[test]
    fn contiguous_with_check() {
        use crate::PhysPageNum;
        let a = AddrRange::new(PhysPageNum::new(0), PhysPageNum::new(3));
        let b = AddrRange::new(PhysPageNum::new(3), PhysPageNum::new(5));
        let c = AddrRange::new(PhysPageNum::new(4), PhysPageNum::new(6));
        assert!(a.contiguous_with(b));
        assert!(!a.contiguous_with(c));
        assert!(!b.contiguous_with(a));
    }
}
