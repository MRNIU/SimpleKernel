//! 连续地址范围 [`AddrRange<A>`]——支持包含判断、重叠检测和大小计算。

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
}
