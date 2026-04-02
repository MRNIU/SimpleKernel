//! 通用半开区间 [`Span<A>`]——支持包含判断、重叠检测、分割、合并和迭代。
//!
//! 零依赖的纯泛型数据结构，可用于地址、页号、帧号等任何 `Copy + Ord` 类型。

#![cfg_attr(not(test), no_std)]

use core::ops::Sub;

/// 半开区间 `[start, end)`，表示一段连续范围。
///
/// 泛型参数 `A` 可以是任何满足 `Copy + Ord` 约束的类型——
/// 地址、页号、帧号、整数等。
///
/// # Panics
/// 构造时 `start > end` 会 panic。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span<A> {
    start: A,
    end: A,
}

impl<A: Copy + Ord> Span<A> {
    /// 构造半开区间 `[start, end)`
    ///
    /// # Panics
    /// `start > end` 时 panic。
    #[inline]
    pub fn new(start: A, end: A) -> Self {
        assert!(start <= end, "Span: start must be <= end");
        Self { start, end }
    }

    /// 范围起始
    #[inline]
    pub fn start(self) -> A {
        self.start
    }

    /// 范围结束（不含）
    #[inline]
    pub fn end(self) -> A {
        self.end
    }

    /// 元素是否在范围内
    #[inline]
    pub fn contains(self, val: A) -> bool {
        self.start <= val && val < self.end
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

impl<A: Copy + Ord + Sub<A, Output = usize>> Span<A> {
    /// 范围大小（单位取决于 `A`）
    #[inline]
    pub fn size(self) -> usize {
        self.end - self.start
    }
}

impl<A: Copy + Ord + core::ops::Add<usize, Output = A>> Span<A> {
    /// 在 `mid` 处分割为 `[start, mid)` 和 `[mid, end)`。
    ///
    /// # Panics
    /// `mid` 不在 `[start, end]` 范围内时 panic。
    #[inline]
    pub fn split_at(self, mid: A) -> (Self, Self) {
        assert!(
            self.start <= mid && mid <= self.end,
            "Span::split_at: mid out of range"
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
    pub fn iter(self) -> SpanIter<A> {
        SpanIter {
            current: self.start,
            end: self.end,
        }
    }
}

/// [`Span`] 的逐元素迭代器。
pub struct SpanIter<A> {
    current: A,
    end: A,
}

impl<A: Copy + Ord + core::ops::Add<usize, Output = A>> Iterator for SpanIter<A> {
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

    /// 测试用 newtype（span crate 零依赖）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Val(usize);
    impl core::ops::Add<usize> for Val {
        type Output = Self;
        fn add(self, rhs: usize) -> Self {
            Self(self.0 + rhs)
        }
    }
    impl core::ops::Sub<Val> for Val {
        type Output = usize;
        fn sub(self, rhs: Val) -> usize {
            self.0 - rhs.0
        }
    }

    #[test]
    fn basic() {
        let r = Span::new(Val(0x1000), Val(0x3000));
        assert_eq!(r.size(), 0x2000);
        assert!(r.contains(Val(0x1000)));
        assert!(!r.contains(Val(0x3000)));
    }

    #[test]
    fn empty() {
        let r = Span::new(Val(1), Val(1));
        assert!(r.is_empty());
    }

    #[test]
    fn overlaps() {
        let a = Span::new(Val(1), Val(3));
        let b = Span::new(Val(2), Val(4));
        assert!(a.overlaps(b));
        assert!(!a.overlaps(Span::new(Val(3), Val(4))));
    }

    #[test]
    #[should_panic(expected = "start must be <= end")]
    fn invalid_panics() {
        let _ = Span::new(Val(2), Val(1));
    }

    #[test]
    fn split() {
        let (l, r) = Span::new(Val(0), Val(4)).split_at(Val(2));
        assert_eq!(l.size(), 2);
        assert_eq!(r.size(), 2);
    }

    #[test]
    fn merge_ok() {
        let m = Span::new(Val(0), Val(2))
            .merge(Span::new(Val(2), Val(5)))
            .unwrap();
        assert_eq!(m.size(), 5);
    }

    #[test]
    fn merge_fail() {
        assert!(
            Span::new(Val(0), Val(2))
                .merge(Span::new(Val(3), Val(5)))
                .is_none()
        );
    }

    #[test]
    fn iter_elements() {
        assert_eq!(Span::new(Val(10), Val(13)).iter().count(), 3);
    }

    #[test]
    fn contiguous() {
        let a = Span::new(Val(0), Val(3));
        let b = Span::new(Val(3), Val(5));
        assert!(a.contiguous_with(b));
        assert!(!b.contiguous_with(a));
    }
}
