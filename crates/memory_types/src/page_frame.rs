//! 物理帧 ([`Frame`]) 类型。
//!
//! 固定 4K 页大小，无泛型参数。内部存储以 4K 页号为单位。

use core::fmt;

use config::PAGE_SIZE_BITS;

use crate::addr::PhysAddr;

/// 物理帧——以 4K 页号为内部存储单位的类型安全帧标识。
///
/// 内部 `number` 是 4K 页号单位。
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

impl From<PhysAddr> for Frame {
    /// 地址转帧号，低位被截断（向下对齐到 4K 边界）。
    #[inline]
    fn from(addr: PhysAddr) -> Self {
        Self {
            number: addr.as_usize() >> PAGE_SIZE_BITS,
        }
    }
}

impl From<Frame> for PhysAddr {
    #[inline]
    fn from(frame: Frame) -> Self {
        frame.start_addr()
    }
}

impl From<usize> for Frame {
    #[inline]
    fn from(v: usize) -> Self {
        Self::new(v)
    }
}

impl From<Frame> for usize {
    #[inline]
    fn from(v: Frame) -> usize {
        v.number
    }
}

impl core::ops::Add<usize> for Frame {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self {
        Self {
            number: self.number.checked_add(rhs).expect("Frame: 加法溢出"),
        }
    }
}

impl core::ops::AddAssign<usize> for Frame {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.number = self.number.checked_add(rhs).expect("Frame: 加法溢出");
    }
}

impl core::ops::Sub<usize> for Frame {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: usize) -> Self {
        Self {
            number: self.number.checked_sub(rhs).expect("Frame: 减法下溢"),
        }
    }
}

impl core::ops::SubAssign<usize> for Frame {
    #[inline]
    fn sub_assign(&mut self, rhs: usize) {
        self.number = self.number.checked_sub(rhs).expect("Frame: 减法下溢");
    }
}

/// 两个帧号相减得到 4K 页的个数。
impl core::ops::Sub<Frame> for Frame {
    type Output = usize;
    #[inline]
    fn sub(self, rhs: Frame) -> usize {
        self.number
            .checked_sub(rhs.number)
            .expect("Frame: 减法下溢")
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
