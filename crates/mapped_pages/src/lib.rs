//! 仿射类型映射——move-only 的 VA→PA 映射所有权。
//!
//! # 核心安全保证
//!
//! 1. **编译期 use-after-unmap 防护**：`as_type::<T>()` 返回的引用
//!    生命周期绑定到 `&self`，编译器阻止在 `MappedPages` drop 后继续使用。
//! 2. **RAII 自动清理**：drop 时自动 unmap 并根据 EXCLUSIVE 位释放物理帧。
//! 3. **EXCLUSIVE 位追踪帧所有权**：帧的所有权信息编码在 PTE 中（而非软件枚举），
//!    即使 `MappedPages` 对象丢失，遍历页表也能恢复所有权信息。
//! 4. **显式页表绑定**：每个 `MappedPages` 持有其所属页表的 `Arc` 引用，
//!    Drop 时通过该引用 unmap，不依赖全局状态。
//!    用户进程退出时其 `Arc` 引用计数归零，页表自动释放。

#![cfg_attr(not(test), no_std)]

#[cfg(any(test, feature = "test-support", target_os = "none"))]
extern crate alloc;

pub mod error;
#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mapping;
#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mmio;

#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub use mapping::MappedPages;
#[cfg(any(test, feature = "test-support"))]
pub use mapping::test_pt;
