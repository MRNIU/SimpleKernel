//! 页号类型 ([`PhysPageNum`], [`VirtPageNum`]) 及与地址类型的互转。

use core::fmt;

use config::PAGE_SIZE_BITS;

use crate::addr::{PhysAddr, VirtAddr};

/// 生成页号 newtype——在 `impl_usize_newtype!` 基础上，
/// 添加 `start_addr()` 方法、与地址类型的 `From` 互转和 `Display`。
macro_rules! define_page_num {
    ($(#[$meta:meta])* $name:ident, $addr:ident) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub(crate) usize);

        crate::impl_usize_newtype!($name);

        impl $name {
            /// 转换为该页起始地址
            #[inline]
            pub const fn start_addr(self) -> $addr {
                $addr::new(self.0 << PAGE_SIZE_BITS)
            }
        }

        impl From<$addr> for $name {
            /// 地址转页号（向下取整）
            #[inline]
            fn from(addr: $addr) -> Self {
                Self(addr.as_usize() >> PAGE_SIZE_BITS)
            }
        }

        impl From<$name> for $addr {
            /// 页号转起始地址
            #[inline]
            fn from(pn: $name) -> Self {
                pn.start_addr()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}(0x{:X})", stringify!($name), self.0)
            }
        }
    };
}

define_page_num!(
    /// 物理页号——页表操作中的帧索引
    PhysPageNum, PhysAddr
);

define_page_num!(
    /// 虚拟页号——页表操作中的虚拟页索引
    VirtPageNum, VirtAddr
);

// `page_number()` 方法定义在本模块而非 `addr.rs`，
// 因为返回类型 `PhysPageNum` / `VirtPageNum` 在此处定义，
// 放在 `addr.rs` 会造成循环依赖。

impl PhysAddr {
    /// 转换为物理页号（向下取整）
    #[inline]
    pub const fn page_number(self) -> PhysPageNum {
        PhysPageNum::new(self.0 >> PAGE_SIZE_BITS)
    }
}

impl VirtAddr {
    /// 转换为虚拟页号（向下取整）
    #[inline]
    pub const fn page_number(self) -> VirtPageNum {
        VirtPageNum::new(self.0 >> PAGE_SIZE_BITS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PhysAddr → PhysPageNum → PhysAddr 往返一致。
    #[test]
    fn phys_page_num_roundtrip() {
        let addr = PhysAddr::new(0x8020_3000);
        let pn = addr.page_number();
        assert_eq!(pn.as_usize(), 0x8020_3);
        assert_eq!(pn.start_addr(), addr);
    }

    /// 未对齐地址转页号时向下取整。
    #[test]
    fn page_num_truncates() {
        let addr = PhysAddr::new(0x8020_3FFF);
        let pn = addr.page_number();
        assert_eq!(pn.as_usize(), 0x8020_3);
        assert_eq!(pn.start_addr(), PhysAddr::new(0x8020_3000));
    }

    /// From trait 双向转换。
    #[test]
    fn page_num_from_trait() {
        let addr = VirtAddr::new(0x1_0000);
        let pn: VirtPageNum = addr.into();
        assert_eq!(pn.as_usize(), 0x10);
        let back: VirtAddr = pn.into();
        assert_eq!(back, addr);
    }

    /// 零页号的起始地址应为 0。
    #[test]
    fn page_num_zero() {
        let pn = PhysPageNum::new(0);
        assert_eq!(pn.start_addr(), PhysAddr::new(0));
    }

    /// 页号加减偏移及两个页号相减。
    #[test]
    fn page_num_arithmetic() {
        let pn = VirtPageNum::new(0x100);
        assert_eq!((pn + 3).as_usize(), 0x103);
        assert_eq!((pn - 1).as_usize(), 0xFF);
        let diff: usize = VirtPageNum::new(0x105) - pn;
        assert_eq!(diff, 5);
    }

    /// += 和 -= 运算符。
    #[test]
    fn page_num_assign_ops() {
        let mut pn = VirtPageNum::new(10);
        pn += 5;
        assert_eq!(pn.as_usize(), 15);
        pn -= 3;
        assert_eq!(pn.as_usize(), 12);
    }

    /// From<usize> 和 Into<usize> 双向转换。
    #[test]
    fn from_usize_roundtrip() {
        let pn: PhysPageNum = 42usize.into();
        let val: usize = pn.into();
        assert_eq!(val, 42);
    }

    /// Display 格式化。
    #[test]
    fn display_format() {
        let pn = PhysPageNum::new(0x42);
        assert_eq!(format!("{pn}"), "PhysPageNum(0x42)");
    }
}
