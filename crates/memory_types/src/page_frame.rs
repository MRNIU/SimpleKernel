//! 物理帧 ([`Frame<P>`]) 与虚拟页 ([`Page<P>`]) 类型。
//!
//! 泛型参数 `P: PageSize` 在编译期标记帧/页的粒度（4K/2M/1G），
//! 防止不同粒度的帧/页在算术运算中混用。
//!
//! 内部存储统一以 4K 页号为单位，算术运算按 `P::NUM_4K_PAGES_SHIFT` 位移缩放：
//! - `Frame<Page4K> + 1` → 内部 number 加 1
//! - `Frame<Page2M> + 1` → 内部 number 加 512（跳过一个 2M 页）
//!
//! 默认类型参数为 `Page4K`，因此 `Frame` 等价于 `Frame<Page4K>`。

use core::fmt;
use core::marker::PhantomData;

use config::PAGE_SIZE_BITS;

use crate::addr::{PhysAddr, VirtAddr};
use crate::page_size::{Page4K, PageSize};

/// 物理帧——以 4K 页号为内部存储单位的类型安全帧标识。
///
/// 泛型参数 `P` 标记帧的粒度，算术运算自动按 `P::NUM_4K_PAGES_SHIFT` 位移缩放。
/// 内部 `number` 始终是 4K 页号单位。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame<P: PageSize = Page4K> {
    pub(crate) number: usize,
    _marker: PhantomData<P>,
}

/// 虚拟页——以 4K 页号为内部存储单位的类型安全页标识。
///
/// 泛型参数 `P` 标记页的粒度，算术运算自动按 `P::NUM_4K_PAGES_SHIFT` 位移缩放。
/// 内部 `number` 始终是 4K 页号单位。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Page<P: PageSize = Page4K> {
    pub(crate) number: usize,
    _marker: PhantomData<P>,
}

/// 为 Frame<P> 和 Page<P> 生成通用方法和 trait 实现。
///
/// 两种类型的逻辑完全对称，仅关联的地址类型不同（PhysAddr / VirtAddr）。
macro_rules! impl_page_or_frame {
    ($name:ident, $addr:ident, $display_prefix:expr) => {
        impl<P: PageSize> $name<P> {
            /// 从 4K 页号构造。
            ///
            /// 对于大页（P != Page4K），校验 4K 页号对齐到 P 的边界。
            #[inline]
            pub const fn new(number_4k: usize) -> Self {
                assert!(
                    P::NUM_4K_PAGES == 1 || number_4k & (P::NUM_4K_PAGES - 1) == 0,
                    concat!(stringify!($name), ": 4K 页号未对齐到页大小边界")
                );
                Self {
                    number: number_4k,
                    _marker: PhantomData,
                }
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

        impl<P: PageSize> From<$addr> for $name<P> {
            /// 地址转帧/页号（向下对齐到 P 的边界）
            #[inline]
            fn from(addr: $addr) -> Self {
                let number_4k = addr.as_usize() >> PAGE_SIZE_BITS;
                let aligned = (number_4k >> P::NUM_4K_PAGES_SHIFT) << P::NUM_4K_PAGES_SHIFT;
                Self {
                    number: aligned,
                    _marker: PhantomData,
                }
            }
        }

        impl<P: PageSize> From<$name<P>> for $addr {
            /// 帧/页号转起始地址
            #[inline]
            fn from(pn: $name<P>) -> Self {
                pn.start_addr()
            }
        }

        impl<P: PageSize> From<usize> for $name<P> {
            #[inline]
            fn from(v: usize) -> Self {
                Self::new(v)
            }
        }

        impl<P: PageSize> From<$name<P>> for usize {
            #[inline]
            fn from(v: $name<P>) -> usize {
                v.number
            }
        }

        impl<P: PageSize> core::ops::Add<usize> for $name<P> {
            type Output = Self;
            /// 前进 `rhs` 个 P 大小的页——内部 number 加 `rhs << NUM_4K_PAGES_SHIFT`
            #[inline]
            #[expect(
                clippy::suspicious_arithmetic_impl,
                reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug"
            )]
            fn add(self, rhs: usize) -> Self {
                // NUM_4K_PAGES_SHIFT 是 sealed trait 常量（0/9/18），不可能超过位宽
                let delta = rhs << P::NUM_4K_PAGES_SHIFT;
                Self {
                    number: self
                        .number
                        .checked_add(delta)
                        .expect(concat!(stringify!($name), ": 加法溢出")),
                    _marker: PhantomData,
                }
            }
        }

        impl<P: PageSize> core::ops::AddAssign<usize> for $name<P> {
            #[inline]
            #[expect(
                clippy::suspicious_op_assign_impl,
                reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug"
            )]
            fn add_assign(&mut self, rhs: usize) {
                // NUM_4K_PAGES_SHIFT 是 sealed trait 常量（0/9/18），不可能超过位宽
                let delta = rhs << P::NUM_4K_PAGES_SHIFT;
                self.number = self
                    .number
                    .checked_add(delta)
                    .expect(concat!(stringify!($name), ": 加法溢出"));
            }
        }

        impl<P: PageSize> core::ops::Sub<usize> for $name<P> {
            type Output = Self;
            /// 后退 `rhs` 个 P 大小的页——内部 number 减 `rhs << NUM_4K_PAGES_SHIFT`
            #[inline]
            #[expect(
                clippy::suspicious_arithmetic_impl,
                reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug"
            )]
            fn sub(self, rhs: usize) -> Self {
                // NUM_4K_PAGES_SHIFT 是 sealed trait 常量（0/9/18），不可能超过位宽
                let delta = rhs << P::NUM_4K_PAGES_SHIFT;
                Self {
                    number: self
                        .number
                        .checked_sub(delta)
                        .expect(concat!(stringify!($name), ": 减法下溢")),
                    _marker: PhantomData,
                }
            }
        }

        impl<P: PageSize> core::ops::SubAssign<usize> for $name<P> {
            #[inline]
            #[expect(
                clippy::suspicious_op_assign_impl,
                reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug"
            )]
            fn sub_assign(&mut self, rhs: usize) {
                // NUM_4K_PAGES_SHIFT 是 sealed trait 常量（0/9/18），不可能超过位宽
                let delta = rhs << P::NUM_4K_PAGES_SHIFT;
                self.number = self
                    .number
                    .checked_sub(delta)
                    .expect(concat!(stringify!($name), ": 减法下溢"));
            }
        }

        impl<P: PageSize> core::ops::Sub<$name<P>> for $name<P> {
            type Output = usize;
            /// 两个同类型帧/页号的差——返回 P 大小页的个数
            #[inline]
            #[expect(
                clippy::suspicious_arithmetic_impl,
                reason = "页号差值按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug"
            )]
            fn sub(self, rhs: $name<P>) -> usize {
                let diff = self
                    .number
                    .checked_sub(rhs.number)
                    .expect(concat!(stringify!($name), ": 减法下溢"));
                diff >> P::NUM_4K_PAGES_SHIFT
            }
        }

        impl<P: PageSize> fmt::Display for $name<P> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}<{}>(0x{:X})", $display_prefix, P::NAME, self.number)
            }
        }

        impl<P: PageSize> fmt::Debug for $name<P> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}<{}>(0x{:X})", $display_prefix, P::NAME, self.number)
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
            _marker: PhantomData,
        }
    }
}

impl VirtAddr {
    /// 转换为虚拟页号（向下取整到 4K 边界）
    #[inline]
    pub const fn page_number(self) -> Page {
        Page {
            number: self.0 >> PAGE_SIZE_BITS,
            _marker: PhantomData,
        }
    }
}
