// Copyright The SimpleKernel Contributors

//! 物理帧 ([`Frame`]) 类型。
//!
//! 固定 4K 页大小，无泛型参数。内部存储以 4K 页号为单位。

use core::fmt;

use config::PAGE_SIZE_BITS;

use crate::addr::PhysAddr;

/// 当前架构物理地址位宽可表示的 4K 帧数量上限。
const MAX_FRAME_COUNT: usize = 1usize << (crate::addr_width::PA_BITS - PAGE_SIZE_BITS);

/// 物理帧——4K 页号的类型安全 newtype。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame {
    pub(crate) number: usize,
}

impl Frame {
    /// 从 4K 页号构造物理帧。
    ///
    /// # Panics
    ///
    /// `number_4k` 大于等于当前架构物理地址位宽可表示的 4K 帧数量时 panic。
    #[inline]
    pub const fn new(number_4k: usize) -> Self {
        assert!(
            number_4k < MAX_FRAME_COUNT,
            "Frame::new: 页号必须小于 1 << (PA_BITS - PAGE_SIZE_BITS)"
        );
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

/// 将帧号向高地址移动指定页数。
///
/// # Panics
///
/// 帧号加法溢出，或结果大于等于当前架构物理地址位宽可表示的 4K 帧数量时 panic。
impl core::ops::Add<usize> for Frame {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self {
        let lhs = self.number;
        let number = lhs
            .checked_add(rhs)
            .unwrap_or_else(|| panic!("Frame::add: {lhs} + {rhs} 加法溢出"));
        assert!(
            number < MAX_FRAME_COUNT,
            "Frame::add: {lhs} + {rhs} 超出 PA_BITS 可表示范围"
        );
        Self { number }
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
