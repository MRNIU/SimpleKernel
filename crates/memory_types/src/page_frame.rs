//! 物理帧 ([`Frame`]) 与虚拟页 ([`Page`]) 类型。
//!
//! 固定 4K 页大小，无泛型参数。内部存储以 4K 页号为单位。

use core::fmt;

use config::PAGE_SIZE_BITS;

use crate::addr::{PhysAddr, VirtAddr};

/// 物理帧——以 4K 页号为内部存储单位的类型安全帧标识。
///
/// 内部 `number` 是 4K 页号单位。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame {
    pub(crate) number: usize,
}

/// 虚拟页——以 4K 页号为内部存储单位的类型安全页标识。
///
/// 内部 `number` 是 4K 页号单位。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page {
    pub(crate) number: usize,
}

/// 为 Frame 和 Page 生成通用方法和 trait 实现。
///
/// 两种类型的逻辑完全对称，仅关联的地址类型不同（PhysAddr / VirtAddr）。
macro_rules! impl_page_or_frame {
    ($name:ident, $addr:ident, $display_prefix:expr) => {
        impl $name {
            /// 从 4K 页号构造。
            #[inline]
            pub const fn new(number_4k: usize) -> Self {
                Self { number: number_4k }
            }

            /// 返回内部 4K 页号
            #[inline]
            pub const fn as_usize(self) -> usize {
                self.number
            }

            /// 转换为起始地址
            #[inline]
            pub const fn start_addr(self) -> $addr {
                $addr::new(self.number << PAGE_SIZE_BITS)
            }
        }

        impl From<$addr> for $name {
            /// 地址转帧/页号（向下对齐到 4K 边界）
            #[inline]
            fn from(addr: $addr) -> Self {
                let number_4k = addr.as_usize() >> PAGE_SIZE_BITS;
                Self { number: number_4k }
            }
        }

        impl From<$name> for $addr {
            /// 帧/页号转起始地址
            #[inline]
            fn from(pn: $name) -> Self {
                pn.start_addr()
            }
        }

        impl From<usize> for $name {
            #[inline]
            fn from(v: usize) -> Self {
                Self::new(v)
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(v: $name) -> usize {
                v.number
            }
        }

        impl core::ops::Add<usize> for $name {
            type Output = Self;
            /// 前进 `rhs` 个 4K 页
            #[inline]
            fn add(self, rhs: usize) -> Self {
                Self {
                    number: self
                        .number
                        .checked_add(rhs)
                        .expect(concat!(stringify!($name), ": 加法溢出")),
                }
            }
        }

        impl core::ops::AddAssign<usize> for $name {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                self.number = self
                    .number
                    .checked_add(rhs)
                    .expect(concat!(stringify!($name), ": 加法溢出"));
            }
        }

        impl core::ops::Sub<usize> for $name {
            type Output = Self;
            /// 后退 `rhs` 个 4K 页
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                Self {
                    number: self
                        .number
                        .checked_sub(rhs)
                        .expect(concat!(stringify!($name), ": 减法下溢")),
                }
            }
        }

        impl core::ops::SubAssign<usize> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                self.number = self
                    .number
                    .checked_sub(rhs)
                    .expect(concat!(stringify!($name), ": 减法下溢"));
            }
        }

        impl core::ops::Sub<$name> for $name {
            type Output = usize;
            /// 两个同类型帧/页号的差——返回 4K 页的个数
            #[inline]
            fn sub(self, rhs: $name) -> usize {
                self.number
                    .checked_sub(rhs.number)
                    .expect(concat!(stringify!($name), ": 减法下溢"))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}(0x{:X})", $display_prefix, self.number)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}(0x{:X})", $display_prefix, self.number)
            }
        }
    };
}

impl_page_or_frame!(Frame, PhysAddr, "Frame");
impl_page_or_frame!(Page, VirtAddr, "Page");

// `page_number()` 方法定义在本模块而非 `addr.rs`，
// 因为返回类型 `Frame` / `Page` 在此处定义，
// 放在 `addr.rs` 会造成循环依赖。

impl PhysAddr {
    /// 转换为物理页号（向下取整到 4K 边界）
    #[inline]
    pub const fn page_number(self) -> Frame {
        Frame {
            number: self.0 >> PAGE_SIZE_BITS,
        }
    }
}

impl VirtAddr {
    /// 转换为虚拟页号（向下取整到 4K 边界）
    #[inline]
    pub const fn page_number(self) -> Page {
        Page {
            number: self.0 >> PAGE_SIZE_BITS,
        }
    }
}
