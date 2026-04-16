//! 半开区间 `[start, end)` 类型——支持重叠检测。

use core::ops::Sub;

/// 半开区间 `[start, end)`，表示一段连续范围。
///
/// 泛型参数 `A` 可以是任何满足 `Copy + Ord` 约束的类型——
/// 地址、页号、帧号等。
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

    /// 两个范围是否重叠
    #[inline]
    pub fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl<A: Copy + Ord + Sub<A, Output = usize>> Span<A> {
    /// 范围大小（单位取决于 `A`）
    #[inline]
    pub fn size(self) -> usize {
        self.end - self.start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造、size、边界访问
    #[test]
    fn basic() {
        let r = Span::new(0x1000usize, 0x3000);
        assert_eq!(r.size(), 0x2000);
        assert_eq!(r.start(), 0x1000);
        assert_eq!(r.end(), 0x3000);
    }

    /// 重叠检测——相交与相邻
    #[test]
    fn overlaps() {
        let a = Span::new(1usize, 3);
        let b = Span::new(2usize, 4);
        assert!(a.overlaps(b));
        // 相邻但不重叠
        assert!(!a.overlaps(Span::new(3, 4)));
    }

    /// start > end 时 panic
    #[test]
    #[should_panic(expected = "start must be <= end")]
    fn invalid_panics() {
        let _ = Span::new(2usize, 1);
    }
}
