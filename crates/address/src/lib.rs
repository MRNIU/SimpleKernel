#![cfg_attr(not(test), no_std)]

/// 物理地址与虚拟地址 newtype 封装
///
/// PhysAddr 和 VirtAddr 是不同的类型，编译时不可混用。
use core::fmt;
use core::ops::{Add, Sub};

use config::PAGE_SIZE;

/// 生成地址 newtype，包含对齐辅助、算术运算符和格式化输出。
macro_rules! define_address {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);

        impl $name {
            /// 从原始 usize 构造地址
            #[inline]
            pub const fn new(addr: usize) -> Self {
                Self(addr)
            }

            /// 返回内部 usize 值
            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }

            /// 页内偏移（低 PAGE_SIZE_BITS 位）
            #[inline]
            pub const fn page_offset(self) -> usize {
                self.0 & (PAGE_SIZE - 1)
            }

            /// 是否页对齐
            #[inline]
            pub const fn is_aligned(self) -> bool {
                self.page_offset() == 0
            }

            /// 向下对齐到页边界
            #[inline]
            pub const fn align_down(self) -> Self {
                Self(self.0 & !(PAGE_SIZE - 1))
            }

            /// 向上对齐到页边界；已对齐时保持不变
            #[inline]
            pub const fn align_up(self) -> Self {
                Self((self.0 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
            }
        }

        impl Add<usize> for $name {
            type Output = Self;
            #[inline]
            fn add(self, rhs: usize) -> Self {
                Self(self.0 + rhs)
            }
        }

        impl Sub<usize> for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                Self(self.0 - rhs)
            }
        }

        /// 两个地址相减，返回字节差值
        impl Sub<$name> for $name {
            type Output = usize;
            #[inline]
            fn sub(self, rhs: $name) -> usize {
                self.0 - rhs.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "0x{:016X}", self.0)
            }
        }
    };
}

define_address!(
    /// 物理地址
    PhysAddr
);

define_address!(
    /// 虚拟地址
    VirtAddr
);

#[cfg(test)]
mod tests {
    use super::*;

    /// 页对齐：已对齐地址保持不变，未对齐地址 align_down 向下取整、
    /// align_up 向上取整，页末地址（0xFFF）正确归入当前页。
    #[test]
    fn alignment_basic() {
        let aligned = PhysAddr::new(0x8020_0000);
        assert!(aligned.is_aligned());
        assert_eq!(aligned.page_offset(), 0);
        assert_eq!(aligned.align_down(), aligned);
        assert_eq!(aligned.align_up(), aligned);

        let unaligned = PhysAddr::new(0x8020_0001);
        assert!(!unaligned.is_aligned());
        assert_eq!(unaligned.page_offset(), 1);
        assert_eq!(unaligned.align_down(), PhysAddr::new(0x8020_0000));
        assert_eq!(unaligned.align_up(), PhysAddr::new(0x8020_1000));

        // 恰好在页末
        let end_of_page = PhysAddr::new(0x8020_0FFF);
        assert!(!end_of_page.is_aligned());
        assert_eq!(end_of_page.align_down(), PhysAddr::new(0x8020_0000));
        assert_eq!(end_of_page.align_up(), PhysAddr::new(0x8020_1000));
    }

    /// 零地址边界：地址 0 应被视为页对齐，align_up/align_down 均返回自身。
    #[test]
    fn alignment_zero() {
        let zero = PhysAddr::new(0);
        assert!(zero.is_aligned());
        assert_eq!(zero.align_down(), zero);
        assert_eq!(zero.align_up(), zero);
    }

    /// 地址 ± usize 运算：加偏移得到新地址，减偏移回到原地址。
    #[test]
    fn arithmetic_add_sub_usize() {
        let base = PhysAddr::new(0x8020_0000);

        let a = base + 0x1000;
        assert_eq!(a.as_usize(), 0x8020_1000);

        let b = a - 0x1000;
        assert_eq!(b, base);
    }

    /// 地址 - 地址 返回 usize（字节差值），而非地址类型，
    /// 验证 Sub<VirtAddr> 的 Output 关联类型为 usize。
    #[test]
    fn arithmetic_sub_returns_usize() {
        let a = VirtAddr::new(0xFFFF_0000_0000_2000);
        let b = VirtAddr::new(0xFFFF_0000_0000_0000);
        let diff: usize = a - b;
        assert_eq!(diff, 0x2000);
    }
}
