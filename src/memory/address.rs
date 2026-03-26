/// 物理地址与虚拟地址 newtype 封装
///
/// PhysAddr 和 VirtAddr 是不同的类型，编译时不可混用。
use core::fmt;
use core::ops::{Add, Sub};

use crate::config::PAGE_SIZE;

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
                write!(f, "0x{:016x}", self.0)
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

    #[test]
    fn phys_addr_alignment() {
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

    #[test]
    fn phys_addr_arithmetic() {
        let base = PhysAddr::new(0x8020_0000);

        // Add<usize>
        let a = base + 0x1000;
        assert_eq!(a.as_usize(), 0x8020_1000);

        // Sub<usize>
        let b = a - 0x1000;
        assert_eq!(b, base);

        // Sub<PhysAddr>
        let diff = a - base;
        assert_eq!(diff, 0x1000usize);
    }

    #[test]
    fn phys_addr_display() {
        let addr = PhysAddr::new(0x0000_0000_8020_0000);
        assert_eq!(format!("{}", addr), "0x0000000080200000");
    }

    #[test]
    fn virt_addr_alignment() {
        let aligned = VirtAddr::new(0xFFFF_FFFF_8020_0000);
        assert!(aligned.is_aligned());
        assert_eq!(aligned.align_down(), aligned);
        assert_eq!(aligned.align_up(), aligned);

        let unaligned = VirtAddr::new(0xFFFF_FFFF_8020_0800);
        assert!(!unaligned.is_aligned());
        assert_eq!(unaligned.align_down(), VirtAddr::new(0xFFFF_FFFF_8020_0000));
        assert_eq!(unaligned.align_up(), VirtAddr::new(0xFFFF_FFFF_8020_1000));
    }

    #[test]
    fn virt_addr_arithmetic() {
        let base = VirtAddr::new(0xFFFF_FFFF_8020_0000);

        let a = base + 0x2000;
        assert_eq!(a.as_usize(), 0xFFFF_FFFF_8020_2000);

        let b = a - 0x2000;
        assert_eq!(b, base);

        let diff = a - base;
        assert_eq!(diff, 0x2000usize);
    }
}
