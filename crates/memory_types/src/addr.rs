//! 物理地址 ([`PhysAddr`]) 与虚拟地址 ([`VirtAddr`]) 类型。

use core::fmt;

use config::{PAGE_SIZE, PHYS_OFFSET};

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
                    Some(v) => Self::new(v & !(align - 1)),
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
/// 开启分页后不可直接解引用，需通过 `.to_virt()` 转换为
/// [`VirtAddr`] 后再访问。
///
/// 构造时校验地址在 [`PA_BITS`](config::PA_BITS) 范围内，
/// 超出范围 panic。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(pub(crate) usize);

crate::impl_usize_newtype!(PhysAddr, phys);
impl_addr!(PhysAddr);

/// 虚拟地址——CPU 可直接访问的指针级地址。
///
/// 构造时校验地址是否规范化（[`VA_BITS`](config::VA_BITS) 位符号扩展），
/// 非规范地址 panic。
///
/// 规范化规则：bits\[63:VA_BITS-1\] 必须是 bit\[VA_BITS-2\] 的符号扩展。
/// 这将地址空间分为低段（全 0 前缀）和高段（全 1 前缀），
/// 中间的 "canonical hole" 为非法地址。
///
/// 提供 `as_ptr` / `as_mut_ptr` 转换为裸指针，以及
/// `From<*const T>` / `From<*mut T>` 反向构造。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtAddr(pub(crate) usize);

crate::impl_usize_newtype!(VirtAddr, virt);
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

    /// 转换为物理地址（线性映射，偏移量 [`PHYS_OFFSET`]）。
    ///
    /// 仅适用于线性映射区域，非线性映射地址须通过页表查询。
    ///
    /// # Panics
    /// 结果不在物理地址有效范围内时 panic。
    #[inline]
    pub fn to_phys(self) -> PhysAddr {
        PhysAddr::new(self.as_usize().wrapping_sub(PHYS_OFFSET))
    }
}

impl<T> From<*const T> for VirtAddr {
    #[inline]
    fn from(p: *const T) -> Self {
        Self::new(p as usize)
    }
}

impl<T> From<*mut T> for VirtAddr {
    #[inline]
    fn from(p: *mut T) -> Self {
        Self::new(p as usize)
    }
}

impl PhysAddr {
    /// 转换为虚拟地址（线性映射，偏移量 [`PHYS_OFFSET`]）。
    ///
    /// 仅适用于线性映射区域，非线性映射地址须通过页表查询。
    ///
    /// # Panics
    /// 结果不在规范虚拟地址范围内时 panic。
    #[inline]
    pub fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.as_usize().wrapping_add(PHYS_OFFSET))
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
    /// 使用规范化的高段地址（Sv39: bits[63:38] 全 1）。
    #[test]
    fn virt_addr_alignment() {
        let v = VirtAddr::new(0xFFFF_FFC0_0000_0001);
        assert!(!v.is_aligned());
        assert_eq!(v.align_down(), VirtAddr::new(0xFFFF_FFC0_0000_0000));
        assert_eq!(v.align_up(), VirtAddr::new(0xFFFF_FFC0_0000_1000));
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
    /// 使用接近 usize::MAX 的规范化高段虚拟地址。
    #[test]
    #[should_panic(expected = "align_up: address overflow")]
    fn align_up_to_overflow_panics() {
        let near_max = VirtAddr::new(usize::MAX - PAGE_SIZE + 2);
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
    /// 使用规范化的高段地址。
    #[test]
    fn arithmetic_sub_returns_usize() {
        let a = VirtAddr::new(0xFFFF_FFC0_0000_2000);
        let b = VirtAddr::new(0xFFFF_FFC0_0000_0000);
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

    /// From<*const T> / From<*mut T> 构造 VirtAddr——使用规范地址范围内的指针。
    #[test]
    fn virt_addr_from_pointer() {
        let canonical: usize = 0x1000;
        let ptr = canonical as *const u64;
        let addr = VirtAddr::from(ptr);
        assert_eq!(addr.as_usize(), canonical);
    }

    /// to_virt / to_phys 互逆：任意物理地址经往返转换后应恢复原值。
    #[test]
    fn phys_virt_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let va = pa.to_virt();
        assert_eq!(va.as_usize(), pa.as_usize().wrapping_add(PHYS_OFFSET));
        assert_eq!(va.to_phys(), pa);
    }

    /// 零地址的物理-虚拟转换。
    #[test]
    fn phys_virt_zero() {
        let pa = PhysAddr::new(0);
        let va = pa.to_virt();
        assert_eq!(va.as_usize(), PHYS_OFFSET);
        assert_eq!(va.to_phys(), pa);
    }

    /// PhysAddr 加法结果超出 PA_BITS 范围应 panic。
    #[test]
    #[should_panic(expected = "PA_BITS")]
    fn phys_add_overflow_pa_bits() {
        let near_max = PhysAddr::new((1usize << config::PA_BITS) - 2);
        let _ = near_max + 4;
    }

    /// VirtAddr 加法结果不满足规范化应 panic。
    #[test]
    #[should_panic(expected = "非规范虚拟地址")]
    fn virt_add_breaks_canonical() {
        let near_boundary = VirtAddr::new((1usize << (config::VA_BITS - 1)) - 2);
        let _ = near_boundary + 4;
    }
}
