//! 架构 TLB 刷新原语——直接操作硬件 TLB。
//!
//! [`TlbArch`] 定义 TLB 刷新契约，各架构独立实现。

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod aarch64;
#[cfg(not(target_os = "none"))]
mod host;
#[cfg(all(target_os = "none", target_arch = "riscv64"))]
mod riscv64;

/// TLB 刷新架构契约。
///
/// 所有方法均为关联函数（无 `self`），因为 TLB 操作是全局的、无状态的。
pub(crate) trait TlbArch {
    /// 刷新整个 TLB。
    fn flush_all();

    /// 刷新指定虚拟地址的单条 TLB 表项。
    fn flush_page(vaddr: usize);
}

#[cfg(all(target_os = "none", target_arch = "riscv64"))]
pub(crate) type Arch = riscv64::Riscv64;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub(crate) type Arch = aarch64::Aarch64;

#[cfg(not(target_os = "none"))]
pub(crate) type Arch = host::Host;
