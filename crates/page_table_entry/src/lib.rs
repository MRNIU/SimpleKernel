//! 硬件页表项编解码——PTE trait 定义与各架构实现。
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - [`aarch64`] / [`riscv64`]：各架构的 `PageTableEntry` + `PteFlags` 定义
//! - [`PageTableEntry`] / [`PteFlags`]：当前目标架构的类型别名
//!
//! 本 crate 无 `alloc` / `config` 依赖，可在 heap 未初始化的早期启动阶段使用。

#![cfg_attr(not(test), no_std)]

use address::PhysAddr;

// TODO(asid): 用户态映射已设置 non-Global（RISC-V 不设 GLOBAL，AArch64 设 NG），
// 但 ASID 分配、切换及 TLB 维护（sfence.vma rs1=x0,rs2=ASID / TLBI ASIDE1）
// 需在上层页表管理器中实现，确保进程切换时 TLB 不会跨地址空间命中。

pub mod aarch64;
pub mod riscv64;

#[cfg(test)]
mod tests;

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
    /// 用户态读写数据映射（不可执行）。
    fn user_rw() -> Self;
    /// 用户态读-执行映射（不可写）。
    fn user_rx() -> Self;
    /// 用户态只读映射。
    fn user_ro() -> Self;
    /// 用户态读写执行映射。
    fn user_rwx() -> Self;
    /// 是否具有写权限。
    fn is_writable(self) -> bool;
    /// 是否为用户态可访问的映射。
    fn is_user(self) -> bool;
    /// 将标志位适配为指定层级的叶描述符格式。
    fn for_leaf_at_level(self, level: usize) -> Self;
    /// 是否设置了 EXCLUSIVE 软件位。
    fn is_exclusive(self) -> bool;
    /// 返回设置了 EXCLUSIVE 位的新标志。
    fn with_exclusive(self) -> Self;
}

/// 页表项的统一接口——各架构必须实现。
///
/// # Note
///
/// 修改页表项后，调用者**必须**执行对应架构的 TLB 维护操作
/// （RISC-V `sfence.vma` / AArch64 `TLBI` + `DSB` + `ISB`），
/// 否则 CPU 可能继续使用过期的 TLB 缓存条目。
/// TLB 刷新不属于 PTE 编解码的职责，由上层页表管理器负责。
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

#[cfg(target_arch = "aarch64")]
pub use aarch64::{PageTableEntry, PteFlags};
#[cfg(target_arch = "riscv64")]
pub use riscv64::{PageTableEntry, PteFlags};
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub use riscv64::{PageTableEntry, PteFlags};
