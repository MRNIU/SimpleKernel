//! 硬件页表项编解码——PTE trait 定义与各架构实现。
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - [`PteFlags`]：各架构在 `aarch64.rs` / `riscv64.rs` 中定义硬件原生标志位
//! - [`PageTableEntry`]：64 位 PTE 值类型
//!
//! 本 crate 无 `alloc` 依赖，可在 heap 未初始化的早期启动阶段使用。

#![cfg_attr(not(test), no_std)]

use address::PhysAddr;

pub mod error;

#[cfg(test)]
mod tests;

#[cfg(any(target_arch = "aarch64", feature = "test-aarch64"))]
mod aarch64;
#[cfg(not(any(target_arch = "aarch64", feature = "test-aarch64")))]
mod riscv64;

#[cfg(any(target_arch = "aarch64", feature = "test-aarch64"))]
pub use aarch64::PteFlags;
#[cfg(not(any(target_arch = "aarch64", feature = "test-aarch64")))]
pub use riscv64::PteFlags;

/// 页表项标志位的统一接口——各架构必须实现。
///
/// 保证 RISC-V 和 AArch64 的 `PteFlags` 提供完全相同的方法集，
/// 避免新增 preset 时某一架构遗漏。
// TODO(user-space): 添加用户态映射 preset（user_rw / user_rx / user_ro 等），
// 需要在各架构的 PteFlags 中同步实现 USER 位和 AP_UNPRIV 位组合。
pub trait PteFlagsOps: Copy + core::fmt::Debug {
    /// 内核读写数据映射。
    fn kernel_rw() -> Self;
    /// 内核读-执行映射。
    fn kernel_rx() -> Self;
    /// 内核只读映射。
    fn kernel_ro() -> Self;
    /// 内核读写执行映射。
    fn kernel_rwx() -> Self;
    /// 设备 MMIO 映射（不可缓存、不可执行）。
    fn kernel_device() -> Self;
    /// 是否具有写权限。
    fn is_writable(self) -> bool;
    /// 将标志位适配为指定层级的叶描述符格式。
    fn for_leaf_at_level(self, level: usize) -> Self;
    /// 是否设置了 EXCLUSIVE 软件位。
    fn is_exclusive(self) -> bool;
    /// 返回设置了 EXCLUSIVE 位的新标志。
    fn with_exclusive(self) -> Self;
}

/// 页表项的统一接口——各架构必须实现。
pub trait PteOps: Copy + core::fmt::Debug {
    /// 对应架构的标志位类型
    type Flags: PteFlagsOps;
    /// 从物理地址和标志构造叶 PTE。
    fn new(paddr: PhysAddr, flags: Self::Flags) -> Self;
    /// 从 PTE 提取物理地址。
    fn paddr(self) -> PhysAddr;
    /// 从 PTE 提取标志位。
    fn flags(self) -> Self::Flags;
    /// PTE 是否有效。
    fn is_valid(self) -> bool;
    /// 是否为叶节点。
    fn is_leaf(self, level: usize) -> bool;
    /// 空 PTE（全零）。
    fn empty() -> Self;
    /// 中间节点 PTE（指向下一级页表）。
    fn new_intermediate(paddr: PhysAddr) -> Self;
}

/// 单个硬件页表项（64 位）。
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);

/// PTE 大小的位移量——`log2(sizeof(PageTableEntry))`。
pub const PTE_SIZE_SHIFT: usize = core::mem::size_of::<PageTableEntry>().trailing_zeros() as usize;

/// 编译期断言：当前架构的 PageTableEntry 实现了 PteOps。
const _: () = {
    const fn _assert<T: PteOps>() {}
    _assert::<PageTableEntry>();
};
