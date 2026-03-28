//! 硬件页表项编解码——PTE trait 定义与各架构实现。
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - [`aarch64`] / [`riscv64`]：各架构的 `PageTableEntry` + `PteFlags` 定义
//! - [`PageTableEntry`] / [`PteFlags`]：当前目标架构的类型别名
//!
//! 本 crate 无 `alloc` 依赖，可在 heap 未初始化的早期启动阶段使用。

#![cfg_attr(not(test), no_std)]

use address::PhysAddr;

pub mod error;

pub mod aarch64;
pub mod riscv64;

#[cfg(test)]
mod tests;

// TODO(user-space): 添加用户态映射 preset（user_rw / user_rx / user_ro 等），
// 需要在各架构的 PteFlags 中同步实现 USER 位和 AP_UNPRIV 位组合。

/// 页表项标志位的统一接口——各架构必须实现。
///
/// 保证 RISC-V 和 AArch64 的 `PteFlags` 提供完全相同的方法集，
/// 避免新增 preset 时某一架构遗漏。
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
    /// 从原始 u64 值构造 PTE。
    fn from_raw(raw: u64) -> Self;
    /// 获取 PTE 的原始 u64 值。
    fn as_raw(self) -> u64;
}

/// 当前目标架构的页表项类型别名。
#[cfg(target_arch = "aarch64")]
pub type PageTableEntry = aarch64::PageTableEntry;
/// 当前目标架构的页表项类型别名。
#[cfg(not(target_arch = "aarch64"))]
pub type PageTableEntry = riscv64::PageTableEntry;

/// 当前目标架构的 PTE 标志位类型别名。
#[cfg(target_arch = "aarch64")]
pub type PteFlags = aarch64::PteFlags;
/// 当前目标架构的 PTE 标志位类型别名。
#[cfg(not(target_arch = "aarch64"))]
pub type PteFlags = riscv64::PteFlags;

/// PTE 大小的位移量——`log2(sizeof(u64))` = 3。
///
/// 两种架构的 PTE 均为 64 位，此常量在所有架构下一致。
pub const PTE_SIZE_SHIFT: usize = core::mem::size_of::<u64>().trailing_zeros() as usize;

/// 编译期断言：两种架构的 PageTableEntry 均实现了 PteOps。
const _: () = {
    const fn _assert<T: PteOps>() {}
    _assert::<riscv64::PageTableEntry>();
    _assert::<aarch64::PageTableEntry>();
};
