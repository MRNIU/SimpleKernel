//! 内核地址类型库——编译期区分物理/虚拟地址，防止混用。
//!
//! 提供四种 newtype：
//! - [`PhysAddr`] / [`VirtAddr`]——字节粒度地址，附带对齐辅助方法
//! - [`PhysPageNum`] / [`VirtPageNum`]——页粒度索引，与地址双向转换
//! - [`AddrRange<A>`]——半开区间 `[start, end)`，支持包含/重叠判断
//!
//! 以及物理-虚拟地址转换函数 [`phys_to_virt`] / [`virt_to_phys`]。

#![cfg_attr(not(test), no_std)]

mod addr;
mod page_num;
mod range;

pub use addr::{PhysAddr, VirtAddr, phys_to_virt, virt_to_phys};
pub use page_num::{PhysPageNum, VirtPageNum};
pub use range::{AddrRange, AddrRangeIter};

/// 物理帧范围——`AddrRange<PhysPageNum>` 的便利别名。
pub type FrameRange = AddrRange<PhysPageNum>;
/// 虚拟页范围——`AddrRange<VirtPageNum>` 的便利别名。
pub type PageRange = AddrRange<VirtPageNum>;

/// 为 `#[repr(transparent)] struct Foo(usize)` 生成通用基础设施。
///
/// 生成内容：`new` / `as_usize` 构造与访问、`From<usize>` 双向转换、
/// `Add`/`Sub`/`AddAssign`/`SubAssign` 算术运算（`Sub` 含 debug 下溢检查）、
/// 以及同类型相减返回 `usize` 差值。
///
/// 调用方在此基础上添加类型特有的方法（对齐、指针转换等）和 `Display`。
macro_rules! impl_usize_newtype {
    ($name:ident) => {
        impl $name {
            /// 从原始 `usize` 构造
            #[inline]
            pub const fn new(v: usize) -> Self {
                Self(v)
            }

            /// 返回内部 `usize` 值
            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }

        impl From<usize> for $name {
            #[inline]
            fn from(v: usize) -> Self {
                Self(v)
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(v: $name) -> usize {
                v.0
            }
        }

        impl core::ops::Add<usize> for $name {
            type Output = Self;
            #[inline]
            fn add(self, rhs: usize) -> Self {
                Self(self.0 + rhs)
            }
        }

        impl core::ops::AddAssign<usize> for $name {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                self.0 += rhs;
            }
        }

        impl core::ops::Sub<usize> for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                debug_assert!(self.0 >= rhs, "{}: underflow", stringify!($name));
                Self(self.0 - rhs)
            }
        }

        impl core::ops::SubAssign<usize> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                debug_assert!(self.0 >= rhs, "{}: underflow", stringify!($name));
                self.0 -= rhs;
            }
        }

        impl core::ops::Sub<$name> for $name {
            type Output = usize;
            #[inline]
            fn sub(self, rhs: $name) -> usize {
                debug_assert!(self.0 >= rhs.0, "{}: underflow", stringify!($name));
                self.0 - rhs.0
            }
        }
    };
}

pub(crate) use impl_usize_newtype;
