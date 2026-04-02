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
            #[allow(clippy::suspicious_arithmetic_impl)]
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
            #[allow(clippy::suspicious_op_assign_impl)]
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
            #[allow(clippy::suspicious_arithmetic_impl)]
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
            #[allow(clippy::suspicious_op_assign_impl)]
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
            #[allow(clippy::suspicious_arithmetic_impl)]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// PhysAddr → Frame → PhysAddr 往返一致。
    #[test]
    fn frame_addr_roundtrip() {
        let addr = PhysAddr::new(0x8020_3000);
        let pn = addr.page_number();
        assert_eq!(pn.as_usize(), 0x8020_3);
        assert_eq!(pn.start_addr(), addr);
    }

    /// 未对齐地址转页号时向下取整。
    #[test]
    fn frame_from_unaligned_truncates() {
        let addr = PhysAddr::new(0x8020_3FFF);
        let pn = addr.page_number();
        assert_eq!(pn.as_usize(), 0x8020_3);
        assert_eq!(pn.start_addr(), PhysAddr::new(0x8020_3000));
    }

    /// From trait 双向转换。
    #[test]
    fn page_from_trait() {
        let addr = VirtAddr::new(0x1_0000);
        let pn: Page = addr.into();
        assert_eq!(pn.as_usize(), 0x10);
        let back: VirtAddr = pn.into();
        assert_eq!(back, addr);
    }

    /// 零页号的起始地址应为 0。
    #[test]
    fn frame_zero() {
        let pn: Frame = Frame::new(0);
        assert_eq!(pn.start_addr(), PhysAddr::new(0));
    }

    /// 页号加减偏移及两个页号相减。
    #[test]
    fn page_arithmetic() {
        let pn: Page = Page::new(0x100);
        assert_eq!((pn + 3).as_usize(), 0x103);
        assert_eq!((pn - 1).as_usize(), 0xFF);
        let diff: usize = Page::new(0x105) - pn;
        assert_eq!(diff, 5);
    }

    /// += 和 -= 运算符。
    #[test]
    fn page_assign_ops() {
        let mut pn: Page = Page::new(10);
        pn += 5;
        assert_eq!(pn.as_usize(), 15);
        pn -= 3;
        assert_eq!(pn.as_usize(), 12);
    }

    /// From<usize> 和 Into<usize> 双向转换。
    #[test]
    fn from_usize_roundtrip() {
        let pn: Frame = 42usize.into();
        let val: usize = pn.into();
        assert_eq!(val, 42);
    }

    /// Display 格式化。
    #[test]
    fn display_format() {
        let pn: Frame = Frame::new(0x42);
        assert_eq!(format!("{pn}"), "Frame<4K>(0x42)");
    }

    /// Frame<Page2M> 算术应按 512 缩放。
    #[test]
    fn frame_2m_arithmetic() {
        use crate::page_size::Page2M;
        let f = Frame::<Page2M>::new(0); // 4K 页号 0
        let f2 = f + 1; // 前进 1 个 2M 页
        assert_eq!(f2.as_usize(), 512); // 内部 4K 页号 = 512
        assert_eq!(f2.start_addr(), PhysAddr::new(512 * 4096)); // 2 MiB

        let f3 = f + 3;
        assert_eq!(f3 - f, 3); // 差 3 个 2M 页
    }

    /// Frame<Page2M>::new 对非对齐的 4K 页号应 panic。
    #[test]
    #[should_panic(expected = "未对齐到页大小边界")]
    fn frame_2m_unaligned_panics() {
        use crate::page_size::Page2M;
        let _ = Frame::<Page2M>::new(1); // 1 不是 512 的倍数
    }

    /// Frame<Page1G> 算术应按 512×512 缩放。
    #[test]
    fn frame_1g_arithmetic() {
        use crate::page_size::Page1G;
        let f = Frame::<Page1G>::new(0);
        let f2 = f + 1;
        assert_eq!(f2.as_usize(), 512 * 512);
    }

    /// 从 PhysAddr 转换为 Frame<Page2M> 应向下对齐。
    #[test]
    fn frame_2m_from_addr_aligns_down() {
        use crate::page_size::Page2M;
        let addr = PhysAddr::new(3 * 1024 * 1024); // 3 MiB
        let f: Frame<Page2M> = addr.into();
        // 3 MiB / 4K = 768, 向下对齐到 512 的倍数 = 512
        assert_eq!(f.as_usize(), 512);
        assert_eq!(f.start_addr(), PhysAddr::new(2 * 1024 * 1024)); // 2 MiB
    }
}
