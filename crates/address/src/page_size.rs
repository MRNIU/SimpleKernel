//! 页大小类型——编译期区分 4K / 2M / 1G 页，防止不同粒度的页/帧混用。
//!
//! [`PageSize`] 是 sealed trait，仅允许本模块预定义的三种尺寸。
//! [`Frame<P>`](crate::Frame) 和 [`Page<P>`](crate::Page) 以此为泛型参数，
//! 在类型层面追踪页的粒度。
//!
//! 所有类型内部统一以 4K 页号为存储单位，`P::NUM_4K_PAGES` 用于算术缩放。

use core::fmt;

/// 页大小——sealed trait，仅允许 [`Page4K`] / [`Page2M`] / [`Page1G`]。
///
/// 各实现提供两个关联常量，用于在 4K 基准单位和实际页大小之间转换：
/// - `NUM_4K_PAGES`：一个 P 大小的页等于多少个 4K 页
/// - `SIZE_IN_BYTES`：页的字节大小
pub trait PageSize: Copy + Eq + Ord + sealed::Sealed + 'static {
    /// 一个此大小的页包含多少个 4K 页
    const NUM_4K_PAGES: usize;
    /// 字节大小
    const SIZE_IN_BYTES: usize;
}

/// 4 KiB 标准页——最小映射粒度，页表 Level 0 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page4K;
impl PageSize for Page4K {
    const NUM_4K_PAGES: usize = 1;
    const SIZE_IN_BYTES: usize = config::PAGE_SIZE;
}
impl fmt::Display for Page4K {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "4K")
    }
}

/// 2 MiB 大页——页表 Level 1 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page2M;
impl PageSize for Page2M {
    const NUM_4K_PAGES: usize = 512;
    const SIZE_IN_BYTES: usize = 512 * config::PAGE_SIZE;
}
impl fmt::Display for Page2M {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "2M")
    }
}

/// 1 GiB 大页——页表 Level 2 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page1G;
impl PageSize for Page1G {
    const NUM_4K_PAGES: usize = 512 * 512;
    const SIZE_IN_BYTES: usize = 512 * 512 * config::PAGE_SIZE;
}
impl fmt::Display for Page1G {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "1G")
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Page4K {}
    impl Sealed for super::Page2M {}
    impl Sealed for super::Page1G {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 各页大小的常量应正确。
    #[test]
    fn page_size_constants() {
        assert_eq!(Page4K::SIZE_IN_BYTES, 4096);
        assert_eq!(Page4K::NUM_4K_PAGES, 1);

        assert_eq!(Page2M::SIZE_IN_BYTES, 2 * 1024 * 1024);
        assert_eq!(Page2M::NUM_4K_PAGES, 512);

        assert_eq!(Page1G::SIZE_IN_BYTES, 1024 * 1024 * 1024);
        assert_eq!(Page1G::NUM_4K_PAGES, 512 * 512);
    }

    /// NUM_4K_PAGES × 4K = SIZE_IN_BYTES 应成立。
    #[test]
    fn num_4k_times_4k_equals_size() {
        assert_eq!(Page4K::NUM_4K_PAGES * 4096, Page4K::SIZE_IN_BYTES);
        assert_eq!(Page2M::NUM_4K_PAGES * 4096, Page2M::SIZE_IN_BYTES);
        assert_eq!(Page1G::NUM_4K_PAGES * 4096, Page1G::SIZE_IN_BYTES);
    }
}
