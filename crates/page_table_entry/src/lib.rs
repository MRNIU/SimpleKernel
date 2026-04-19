//! 硬件页表项编解码——PTE trait 定义与各架构实现。
//!
//! 通过 [`PteFlagsOps`] + [`PteOps`] trait 屏蔽架构差异，上层只使用
//! [`PageTableEntry`] / [`PteFlags`] 类型别名（条件编译选择具体实现）。
//!
//! 本 crate 无 `alloc` 依赖（`config` 仅提供编译期常量），
//! 可在 heap 未初始化的早期启动阶段使用。

#![no_std]

use memory_types::PhysAddr;

pub mod aarch64;
pub mod riscv64;

/// 页表项标志位的统一接口——各架构必须实现。
///
/// SAS 架构下只需要内核权限工厂——没有用户态，不做页回收（无需 A/D 位查询），
/// 权限修改通过 `PageTable::update_range_flags` 整包替换（无需 builder）。
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

    /// 将标志位适配为指定层级的叶描述符格式。
    fn for_leaf_at_level(self, level: usize) -> Self;
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
    /// 中间节点 PTE（指向下一级页表）。
    fn new_intermediate(paddr: PhysAddr) -> Self;
    /// 从原始 u64 值构造 PTE。
    fn from_raw(raw: u64) -> Self;
    /// 获取 PTE 的原始 u64 值。
    fn as_raw(self) -> u64;
}

#[cfg(bare_aarch64)]
pub use aarch64::{PageTableEntry, PteFlags};
#[cfg(bare_riscv64)]
pub use riscv64::{PageTableEntry, PteFlags};
