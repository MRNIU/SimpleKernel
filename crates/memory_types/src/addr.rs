// Copyright The SimpleKernel Contributors

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
            /// 页内偏移（低 `PAGE_SIZE_BITS` 位）。
            #[inline]
            pub const fn page_offset(self) -> usize {
                self.0 & (PAGE_SIZE - 1)
            }

            #[inline]
            pub const fn is_aligned(self) -> bool {
                self.is_aligned_to(PAGE_SIZE)
            }

            /// # Panics
            /// `align` 非 2 的幂时 panic。
            #[inline]
            pub const fn is_aligned_to(self, align: usize) -> bool {
                assert!(
                    align.is_power_of_two(),
                    concat!(
                        stringify!($name),
                        "::is_aligned_to: align 必须是 2 的幂（非零）"
                    )
                );
                self.0 & (align - 1) == 0
            }

            #[inline]
            pub const fn align_down(self) -> Self {
                self.align_down_to(PAGE_SIZE)
            }

            /// # Panics
            ///
            /// `align` 非 2 的幂时 panic。对 [`VirtAddr`]，对齐结果若落入
            /// canonical hole，也会在重新构造地址时 panic。
            #[inline]
            pub const fn align_down_to(self, align: usize) -> Self {
                assert!(
                    align.is_power_of_two(),
                    concat!(
                        stringify!($name),
                        "::align_down_to: align 必须是 2 的幂（非零）"
                    )
                );
                Self::new(self.0 & !(align - 1))
            }

            /// 向上对齐到页边界；已对齐时保持不变。
            ///
            /// # Panics
            /// 地址接近 `usize::MAX` 导致溢出时 panic。
            #[inline]
            pub const fn align_up(self) -> Self {
                self.align_up_to(PAGE_SIZE)
            }

            /// # Panics
            /// `align` 非 2 的幂，或地址接近 `usize::MAX` 导致溢出时 panic。
            #[inline]
            pub const fn align_up_to(self, align: usize) -> Self {
                assert!(
                    align.is_power_of_two(),
                    concat!(
                        stringify!($name),
                        "::align_up_to: align 必须是 2 的幂（非零）"
                    )
                );
                match self.0.checked_add(align - 1) {
                    Some(v) => Self::new(v & !(align - 1)),
                    None => panic!(concat!(
                        stringify!($name),
                        "::align_up_to: self + (align - 1) 溢出（地址接近 usize::MAX）"
                    )),
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
/// 构造时校验地址在 [`PA_BITS`](arch::PA_BITS) 范围内，
/// 超出范围 panic。
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(pub(crate) usize);

crate::impl_usize_newtype!(PhysAddr, phys);
impl_addr!(PhysAddr);

/// 虚拟地址——CPU 可直接访问的指针级地址。
///
/// 构造时校验地址是否规范化（[`VA_BITS`](arch::VA_BITS) 位符号扩展），
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
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /// 转换为物理地址（SAS identity mapping，VA == PA）。
    ///
    /// 仅适用于内核 SAS 全量映射区域，非 identity mapping 地址须通过页表查询。
    ///
    /// # Panics
    /// 结果超出 `arch::PA_BITS` 范围时 panic。
    #[inline]
    pub fn to_phys(self) -> PhysAddr {
        PhysAddr::new(self.as_usize())
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
    /// 转换为虚拟地址（SAS identity mapping，VA == PA）。
    ///
    /// 仅适用于内核 SAS 全量映射区域，非 identity mapping 地址须通过页表查询。
    ///
    /// # Panics
    /// 结果不是 `arch::VA_BITS` 规范地址时 panic。
    #[inline]
    pub fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.as_usize())
    }
}
