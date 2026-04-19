//! 物理帧 ([`Frame`]) 类型。
//!
//! 固定 4K 页大小，无泛型参数。内部存储以 4K 页号为单位。

use core::fmt;

use config::PAGE_SIZE_BITS;

use crate::addr::PhysAddr;

/// 物理帧——4K 页号的类型安全 newtype。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame {
    pub(crate) number: usize,
}

impl Frame {
    #[inline]
    pub const fn new(number_4k: usize) -> Self {
        Self { number: number_4k }
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.number
    }

    /// 转换为起始物理地址。
    #[inline]
    pub const fn start_addr(self) -> PhysAddr {
        PhysAddr::new(self.number << PAGE_SIZE_BITS)
    }
}

impl core::ops::Add<usize> for Frame {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self {
        let lhs = self.number;
        Self {
            number: lhs
                .checked_add(rhs)
                .unwrap_or_else(|| panic!("Frame::add: {lhs} + {rhs} 加法溢出")),
        }
    }
}

/// 两个帧号相减得到 4K 页的个数。
impl core::ops::Sub<Frame> for Frame {
    type Output = usize;
    #[inline]
    fn sub(self, rhs: Frame) -> usize {
        let (l, r) = (self.number, rhs.number);
        l.checked_sub(r)
            .unwrap_or_else(|| panic!("Frame::sub: {l} - {r} 减法下溢"))
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Frame(0x{:X})", self.number)
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Frame(0x{:X})", self.number)
    }
}

impl PhysAddr {
    /// 转换为物理页号（向下取整到 4K 边界）。
    #[inline]
    pub const fn page_number(self) -> Frame {
        Frame {
            number: self.0 >> PAGE_SIZE_BITS,
        }
    }
}
