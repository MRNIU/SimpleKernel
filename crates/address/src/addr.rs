//! 物理地址 ([`PhysAddr`]) 与虚拟地址 ([`VirtAddr`]) 类型。

use core::fmt;

use config::PAGE_SIZE;

/// 为地址 newtype 生成对齐辅助方法和 `Display`。
///
/// 无参版本（`is_aligned` / `align_down` / `align_up`）委托给
/// 带参的 `_to` 版本，以 [`PAGE_SIZE`] 为默认粒度。
macro_rules! impl_addr {
    ($name:ident) => {
        impl $name {
            /// 页内偏移（低 [`PAGE_SIZE`] 位）
            #[inline]
            pub const fn page_offset(self) -> usize {
                self.0 & (PAGE_SIZE - 1)
            }

            /// 是否页对齐
            #[inline]
            pub const fn is_aligned(self) -> bool {
                self.is_aligned_to(PAGE_SIZE)
            }

            /// 是否按指定大小对齐（`align` 必须为 2 的幂）
            ///
            /// # Panics
            /// `align` 非 2 的幂时 panic。
            #[inline]
            pub const fn is_aligned_to(self, align: usize) -> bool {
                assert!(align.is_power_of_two(), "align must be a power of two");
                self.0 & (align - 1) == 0
            }

            /// 向下对齐到页边界
            #[inline]
            pub const fn align_down(self) -> Self {
                self.align_down_to(PAGE_SIZE)
            }

            /// 向下对齐到指定边界（`align` 必须为 2 的幂）
            ///
            /// # Panics
            /// `align` 非 2 的幂时 panic。
            #[inline]
            pub const fn align_down_to(self, align: usize) -> Self {
                assert!(align.is_power_of_two(), "align must be a power of two");
                Self(self.0 & !(align - 1))
            }

            /// 向上对齐到页边界；已对齐时保持不变
            ///
            /// # Panics
            /// 地址接近 `usize::MAX` 导致溢出时 panic。
            #[inline]
            pub const fn align_up(self) -> Self {
                self.align_up_to(PAGE_SIZE)
            }

            /// 向上对齐到指定边界（`align` 必须为 2 的幂）
            ///
            /// # Panics
            /// `align` 非 2 的幂，或地址接近 `usize::MAX` 导致溢出时 panic。
            #[inline]
            pub const fn align_up_to(self, align: usize) -> Self {
                assert!(align.is_power_of_two(), "align must be a power of two");
                match self.0.checked_add(align - 1) {
                    Some(v) => Self(v & !(align - 1)),
                    None => panic!("align_up: address overflow"),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "0x{:016X}", self.0)
            }
        }
    };
}

/// 物理地址——标识物理内存或 MMIO 的字节位置。
///
/// 开启分页后不可直接解引用，需通过 `phys_to_virt()` 转换为
/// [`VirtAddr`] 后再访问。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(pub(crate) usize);

crate::impl_usize_newtype!(PhysAddr);
impl_addr!(PhysAddr);

/// 虚拟地址——CPU 可直接访问的指针级地址。
///
/// 提供 `as_ptr` / `as_mut_ptr` 转换为裸指针，以及
/// `From<*const T>` / `From<*mut T>` 反向构造。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtAddr(pub(crate) usize);

crate::impl_usize_newtype!(VirtAddr);
impl_addr!(VirtAddr);

impl VirtAddr {
    /// 转换为原始只读指针
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    /// 转换为原始可变指针
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }
}

impl<T> From<*const T> for VirtAddr {
    #[inline]
    fn from(p: *const T) -> Self {
        Self(p as usize)
    }
}

impl<T> From<*mut T> for VirtAddr {
    #[inline]
    fn from(p: *mut T) -> Self {
        Self(p as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 页对齐：已对齐地址保持不变，未对齐地址 align_down 向下取整、
    /// align_up 向上取整，页末地址（0xFFF）正确归入当前页。
    #[test]
    fn alignment_basic() {
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

        let end_of_page = PhysAddr::new(0x8020_0FFF);
        assert!(!end_of_page.is_aligned());
        assert_eq!(end_of_page.align_down(), PhysAddr::new(0x8020_0000));
        assert_eq!(end_of_page.align_up(), PhysAddr::new(0x8020_1000));
    }

    /// 零地址边界：地址 0 应被视为页对齐，align_up/align_down 均返回自身。
    #[test]
    fn alignment_zero() {
        let zero = PhysAddr::new(0);
        assert!(zero.is_aligned());
        assert_eq!(zero.align_down(), zero);
        assert_eq!(zero.align_up(), zero);
    }

    /// VirtAddr 的对齐方法应与 PhysAddr 行为一致（宏生成的代码路径相同，
    /// 但类型不同，回归测试防止将来为某一类型添加特化时引入差异）。
    #[test]
    fn virt_addr_alignment() {
        let v = VirtAddr::new(0xFFFF_0000_0000_0001);
        assert!(!v.is_aligned());
        assert_eq!(v.align_down(), VirtAddr::new(0xFFFF_0000_0000_0000));
        assert_eq!(v.align_up(), VirtAddr::new(0xFFFF_0000_0000_1000));
    }

    /// 通用对齐：is_aligned_to / align_down_to / align_up_to 支持任意 2 的幂。
    #[test]
    fn generic_alignment() {
        let addr = PhysAddr::new(0x1_2345);

        assert!(addr.is_aligned_to(1));
        assert!(!addr.is_aligned_to(0x1_0000));
        assert!(PhysAddr::new(0x1_0000).is_aligned_to(0x1_0000));

        assert_eq!(addr.align_down_to(0x1000), PhysAddr::new(0x1_2000));
        assert_eq!(addr.align_up_to(0x1000), PhysAddr::new(0x1_3000));

        // 64KB 对齐
        assert_eq!(addr.align_down_to(0x1_0000), PhysAddr::new(0x1_0000));
        assert_eq!(addr.align_up_to(0x1_0000), PhysAddr::new(0x2_0000));
    }

    /// align_up 在地址空间顶部应 panic 而非静默回绕。
    #[test]
    #[should_panic(expected = "align_up: address overflow")]
    fn align_up_overflow_panics() {
        let near_max = VirtAddr::new(usize::MAX - PAGE_SIZE + 2);
        let _ = near_max.align_up();
    }

    /// align_up_to 溢出也应 panic。
    #[test]
    #[should_panic(expected = "align_up: address overflow")]
    fn align_up_to_overflow_panics() {
        let near_max = PhysAddr::new(usize::MAX);
        let _ = near_max.align_up_to(4096);
    }

    /// 地址 ± usize 运算：加偏移得到新地址，减偏移回到原地址。
    #[test]
    fn arithmetic_add_sub_usize() {
        let base = PhysAddr::new(0x8020_0000);
        let a = base + 0x1000;
        assert_eq!(a.as_usize(), 0x8020_1000);
        assert_eq!(a - 0x1000, base);
    }

    /// += 和 -= 运算符。
    #[test]
    fn arithmetic_assign_ops() {
        let mut addr = PhysAddr::new(0x1000);
        addr += 0x500;
        assert_eq!(addr, PhysAddr::new(0x1500));
        addr -= 0x500;
        assert_eq!(addr, PhysAddr::new(0x1000));
    }

    /// 地址 - 地址 返回 usize（字节差值），而非地址类型。
    #[test]
    fn arithmetic_sub_returns_usize() {
        let a = VirtAddr::new(0xFFFF_0000_0000_2000);
        let b = VirtAddr::new(0xFFFF_0000_0000_0000);
        let diff: usize = a - b;
        assert_eq!(diff, 0x2000);
    }

    /// From<usize> 和 Into<usize> 双向转换。
    #[test]
    fn from_usize_roundtrip() {
        let addr: PhysAddr = 0x8020_0000usize.into();
        let val: usize = addr.into();
        assert_eq!(val, 0x8020_0000);
    }

    /// Display 格式化应输出 16 位十六进制，高位补零。
    #[test]
    fn display_format() {
        let addr = PhysAddr::new(0x1000);
        assert_eq!(format!("{addr}"), "0x0000000000001000");
    }

    /// as_ptr / as_mut_ptr 往返。
    #[test]
    fn virt_addr_pointer_conversion() {
        let addr = VirtAddr::new(0xDEAD_0000);
        assert_eq!(addr.as_ptr::<u8>() as usize, 0xDEAD_0000);
        assert_eq!(addr.as_mut_ptr::<u32>() as usize, 0xDEAD_0000);
    }

    /// From<*const T> / From<*mut T> 构造 VirtAddr。
    #[test]
    fn virt_addr_from_pointer() {
        let val: u64 = 0;
        let addr = VirtAddr::from(&val as *const u64);
        assert_eq!(addr.as_usize(), &val as *const u64 as usize);
    }
}
