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
/// 仅需指定 `NUM_4K_PAGES`，其余常量自动派生：
/// - `NUM_4K_PAGES_SHIFT` = `log2(NUM_4K_PAGES)`
/// - `SIZE_IN_BYTES` = `NUM_4K_PAGES × PAGE_SIZE`
pub trait PageSize: Copy + Eq + Ord + sealed::Sealed + 'static {
    /// 用于 Display/Debug 输出的简短名称（如 `"4K"`、`"2M"`、`"1G"`）
    const NAME: &'static str;
    /// 一个此大小的页包含多少个 4K 页（必须为 2 的幂）
    const NUM_4K_PAGES: usize;
    /// `log2(NUM_4K_PAGES)`——用于将乘除法替换为位移，避免在无硬件除法器的
    /// 架构（如 RISC-V rv64i）上引入 `__udivdi3` 等软件除法例程。
    const NUM_4K_PAGES_SHIFT: usize = Self::NUM_4K_PAGES.trailing_zeros() as usize;
    /// 字节大小
    const SIZE_IN_BYTES: usize = Self::NUM_4K_PAGES * config::PAGE_SIZE;
}

/// 4 KiB 标准页——最小映射粒度，页表 Level 0 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page4K;
impl PageSize for Page4K {
    const NAME: &'static str = "4K";
    const NUM_4K_PAGES: usize = 1;
}

/// 2 MiB 大页——页表 Level 1 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page2M;
impl PageSize for Page2M {
    const NAME: &'static str = "2M";
    const NUM_4K_PAGES: usize = 512;
}

/// 1 GiB 大页——页表 Level 2 叶节点
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page1G;
impl PageSize for Page1G {
    const NAME: &'static str = "1G";
    const NUM_4K_PAGES: usize = 512 * 512;
}

// 编译期校验：NUM_4K_PAGES 必须为 2 的幂
const _: () = assert!(Page4K::NUM_4K_PAGES.is_power_of_two());
const _: () = assert!(Page2M::NUM_4K_PAGES.is_power_of_two());
const _: () = assert!(Page1G::NUM_4K_PAGES.is_power_of_two());

macro_rules! impl_display {
    ($($ty:ty),*) => { $(
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(Self::NAME)
            }
        }
    )* };
}
impl_display!(Page4K, Page2M, Page1G);

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Page4K {}
    impl Sealed for super::Page2M {}
    impl Sealed for super::Page1G {}
}
