//! 页大小类型——`Frame<P>` 和 `Page<P>` 的泛型约束。
//!
//! [`PageSize`] 是 sealed trait，当前仅允许 [`Page4K`]。
//! ADR-006 移除了 `Page2M`/`Page1G` 大页类型——SAS + QEMU 下大页无可观测收益，
//! 且引入 page splitting、多级映射等不必要的复杂性。

use core::fmt;

/// 页大小——sealed trait，当前仅允许 [`Page4K`]。
///
/// 保留此 trait 是因为 `Frame<P>` 和 `Page<P>` 仍以 `P: PageSize` 为泛型约束。
pub trait PageSize: Copy + Eq + Ord + sealed::Sealed + 'static {
    /// 用于 Display/Debug 输出的简短名称（如 `"4K"`）
    const NAME: &'static str;
    /// 一个此大小的页包含多少个 4K 页（`Page4K` 固定为 1）
    const NUM_4K_PAGES: usize;
    /// `log2(NUM_4K_PAGES)`——用于将乘除法替换为位移，避免在无硬件除法器的
    /// 架构（如 RISC-V rv64i）上引入 `__udivdi3` 等软件除法例程。
    const NUM_4K_PAGES_SHIFT: usize = Self::NUM_4K_PAGES.trailing_zeros() as usize;
    /// 字节大小
    const SIZE_IN_BYTES: usize = Self::NUM_4K_PAGES * config::PAGE_SIZE;
}

/// 4 KiB 标准页——页表 Level 0 叶节点，唯一支持的页大小。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page4K;
impl PageSize for Page4K {
    const NAME: &'static str = "4K";
    const NUM_4K_PAGES: usize = 1;
}

// 编译期校验：NUM_4K_PAGES 必须为 2 的幂
const _: () = assert!(Page4K::NUM_4K_PAGES.is_power_of_two());

impl fmt::Display for Page4K {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::NAME)
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Page4K {}
}
