//! 内核内存基础类型——编译期区分物理/虚拟地址与帧/页号，防止混用。
//!
//! # 在内存子系统中的定位
//!
//! 本 crate 是整个内存子系统的**基础层**——所有上层 crate 都依赖此处定义的类型。
//! 它不包含任何运行时状态，纯粹通过类型系统在编译期防止地址类别混用。
//!
//! ```text
//! memory / paging / frame_allocator / page_table_entry
//!    │        │           │                │
//!    └────────┴───────────┴────────────────┘
//!                         ▼
//!              memory_types (本 crate)
//!                         │
//!                         ▼
//!                    span (区间)
//! ```
//!
//! # 类型概览
//!
//! ```text
//! PhysAddr ──page_number()──▶ Frame ──Span::new()──▶ Span<Frame>
//!    │                          ▲
//!    │ to_virt()                 │ From<PhysAddr>
//!    ▼                          │
//! VirtAddr ──page_number()──▶ Page  ──Span::new()──▶ Span<Page>
//! ```
//!
//! - [`PhysAddr`] / [`VirtAddr`]——字节粒度地址，附带对齐辅助方法
//! - [`Frame`] / [`Page`]——4K 页粒度标识，与地址双向转换
//! - [`Span<A>`]（re-export from [`span`] crate）——半开区间 `[start, end)`
//!
//! 以及物理-虚拟地址转换方法 [`PhysAddr::to_virt`] / [`VirtAddr::to_phys`]
//! （基于 SAS identity mapping 假设：VA == PA）。
//!
//! # 宏生成的代码
//!
//! 本 crate 使用三个内部宏消除重复代码：
//!
//! | 宏 | 作用范围 | 生成内容 |
//! |---|---------|---------|
//! | `impl_usize_newtype!` | PhysAddr, VirtAddr, Frame, Page | `new`/`as_usize`、`From<usize>` 双向转换、checked 算术运算 |
//! | `impl_addr!` | PhysAddr, VirtAddr | 对齐辅助（`page_offset`/`is_aligned`/`align_down`/`align_up`）、`Display` |
//! | `impl_page_or_frame!` | Frame, Page | `start_addr`、地址/usize 双向转换、算术运算、`Display`/`Debug` |

#![no_std]

mod addr;
mod page_frame;

pub use addr::{PhysAddr, VirtAddr};
pub use page_frame::{Frame, Page};
pub use span::Span;

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
            /// 从原始 `usize` 构造，校验物理地址在有效范围内
            #[inline]
            pub const fn new(v: usize) -> Self {
                assert!(
                    arch::PA_BITS >= 64 || v < (1usize << arch::PA_BITS),
                    "PhysAddr: 地址超出 PA_BITS 有效范围"
                );
                Self(v)
            }

            /// 返回内部 `usize` 值
            #[inline]
            pub const fn as_usize(self) -> usize {
                self.0
            }
        }
        crate::impl_usize_newtype!(@common $name);
    };
    ($name:ident, virt) => {
        impl $name {
            /// 从原始 `usize` 构造，校验虚拟地址规范化
            ///
            /// 规范化规则：将地址视为 VA_BITS 宽的有符号数进行符号扩展，
            /// 扩展后的值必须与原值相等。
            #[inline]
            pub const fn new(v: usize) -> Self {
                let shift = usize::BITS as usize - arch::VA_BITS;
                let canonical = ((v as isize) << shift >> shift) as usize;
                assert!(
                    v == canonical,
                    "VirtAddr: 非规范虚拟地址"
                );
                Self(v)
            }

            /// 返回内部 `usize` 值
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

            /// 返回内部 `usize` 值
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
                Self::new(self.0.checked_add(rhs).expect(concat!(stringify!($name), ": 加法溢出")))
            }
        }

        impl core::ops::AddAssign<usize> for $name {
            #[inline]
            fn add_assign(&mut self, rhs: usize) {
                *self = Self::new(self.0.checked_add(rhs).expect(concat!(stringify!($name), ": 加法溢出")));
            }
        }

        impl core::ops::Sub<usize> for $name {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: usize) -> Self {
                Self::new(self.0.checked_sub(rhs).expect(concat!(stringify!($name), ": 减法下溢")))
            }
        }

        impl core::ops::SubAssign<usize> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: usize) {
                *self = Self::new(self.0.checked_sub(rhs).expect(concat!(stringify!($name), ": 减法下溢")));
            }
        }

        impl core::ops::Sub<$name> for $name {
            type Output = usize;
            #[inline]
            fn sub(self, rhs: $name) -> usize {
                self.0.checked_sub(rhs.0).expect(concat!(stringify!($name), ": 减法下溢"))
            }
        }
    };
}

pub(crate) use impl_usize_newtype;
