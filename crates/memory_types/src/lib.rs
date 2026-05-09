// Copyright The SimpleKernel Contributors

//! 内核内存基础类型——编译期区分物理/虚拟地址与帧/页号，防止混用。

#![no_std]

mod addr;
mod page_frame;
mod span;

pub use addr::{PhysAddr, VirtAddr};
pub use page_frame::Frame;
pub use span::Span;

#[cfg(any(bare_riscv64, bare_aarch64))]
pub(crate) mod addr_width {
    pub(crate) const PA_BITS: usize = arch::PA_BITS;
    pub(crate) const VA_BITS: usize = arch::VA_BITS;
}

#[cfg(not(any(bare_riscv64, bare_aarch64)))]
pub(crate) mod addr_width {
    pub(crate) const PA_BITS: usize = usize::BITS as usize;
    pub(crate) const VA_BITS: usize = usize::BITS as usize;
}

/// 为地址 newtype 生成通用基础设施。
///
/// 生成内容：`new`（带规范化校验）/ `as_usize` 构造与访问、
/// `From<usize>` 双向转换、`Add`/`Sub`/`AddAssign`/`SubAssign` checked 算术运算、
/// 以及同类型相减返回 `usize` 差值。
///
/// 第二个参数指定校验类型：
/// - `phys`：物理地址校验——高位必须为零
/// - `virt`：虚拟地址校验——高位必须是符号扩展
/// - `none`：不校验（用于页号等内部类型）
macro_rules! impl_usize_newtype {
    ($name:ident, phys) => {
        impl $name {
            /// 从原始 `usize` 构造，校验物理地址在目标地址位宽范围内。
            #[inline]
            pub const fn new(v: usize) -> Self {
                assert!(
                    crate::addr_width::PA_BITS >= 64 || v < (1usize << crate::addr_width::PA_BITS),
                    "PhysAddr::new: addr 超出 PA_BITS 可表示范围"
                );
                Self(v)
            }

            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }
        crate::impl_usize_newtype!(@common $name);
    };
    ($name:ident, virt) => {
        impl $name {
            /// 从原始 `usize` 构造，校验地址按目标虚拟地址位宽规范化
            /// （高位必须是 bit[VA_BITS-1] 的符号扩展）。
            #[inline]
            pub const fn new(v: usize) -> Self {
                let shift = usize::BITS as usize - crate::addr_width::VA_BITS;
                let canonical = ((v as isize) << shift >> shift) as usize;
                assert!(
                    v == canonical,
                    "VirtAddr::new: addr 不是 VA_BITS 符号扩展的规范地址"
                );
                Self(v)
            }

            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }
        crate::impl_usize_newtype!(@common $name);
    };
    ($name:ident, none) => {
        impl $name {
            /// 从原始 `usize` 构造
            #[inline]
            pub const fn new(v: usize) -> Self {
                Self(v)
            }

            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }
        crate::impl_usize_newtype!(@common $name);
    };
    (@common $name:ident) => {
        impl From<usize> for $name {
            #[inline]
            fn from(v: usize) -> Self {
                Self::new(v)
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
                let lhs = self.0;
                Self::new(lhs.checked_add(rhs).unwrap_or_else(|| panic!(
                    "{}::add: {:#x} + {:#x} 加法溢出",
                    stringify!($name), lhs, rhs,
                )))
            }
        }

        impl core::ops::AddAssign<usize> for $name {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                let lhs = self.0;
                *self = Self::new(lhs.checked_add(rhs).unwrap_or_else(|| panic!(
                    "{}::add_assign: {:#x} + {:#x} 加法溢出",
                    stringify!($name), lhs, rhs,
                )));
            }
        }

        impl core::ops::Sub<usize> for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                let lhs = self.0;
                Self::new(lhs.checked_sub(rhs).unwrap_or_else(|| panic!(
                    "{}::sub: {:#x} - {:#x} 减法下溢",
                    stringify!($name), lhs, rhs,
                )))
            }
        }

        impl core::ops::SubAssign<usize> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                let lhs = self.0;
                *self = Self::new(lhs.checked_sub(rhs).unwrap_or_else(|| panic!(
                    "{}::sub_assign: {:#x} - {:#x} 减法下溢",
                    stringify!($name), lhs, rhs,
                )));
            }
        }

        impl core::ops::Sub<$name> for $name {
            type Output = usize;
            #[inline]
            fn sub(self, rhs: $name) -> usize {
                let (l, r) = (self.0, rhs.0);
                l.checked_sub(r).unwrap_or_else(|| panic!(
                    "{}::sub: {:#x} - {:#x} 减法下溢",
                    stringify!($name), l, r,
                ))
            }
        }
    };
}

pub(crate) use impl_usize_newtype;
