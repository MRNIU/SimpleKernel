//! 硬件页表项编解码——PTE trait 定义与各架构实现。
//!
//! # 在内存子系统中的定位
//!
//! 本 crate 是内存子系统的**资源层**——提供单个 PTE 的 bit-level 编解码，
//! 不感知页表结构（多级遍历由 `paging` crate 负责）。
//!
//! ```text
//! paging::PageTable (遍历 + 读写)
//!    │
//!    ▼ 调用 PteOps 方法编解码
//! page_table_entry (本 crate: 单个 PTE)
//!    │
//!    ▼
//! memory_types::PhysAddr (地址类型)
//! ```
//!
//! # 架构抽象方式
//!
//! 通过 trait + 条件编译实现跨架构：
//! - [`PteFlagsOps`]：权限工厂接口
//! - [`PteOps`]：PTE 编解码统一接口
//! - `#[cfg(bare_riscv64)]` / `#[cfg(bare_aarch64)]`：选择具体实现
//!
//! 上层代码只使用 [`PageTableEntry`] / [`PteFlags`] 类型别名，无需关心架构差异。
//!
//! # 类型列表
//!
//! - [`PteFlagsOps`] / [`PteOps`]：统一 trait 接口，各架构必须实现
//! - [`aarch64`] / [`riscv64`]：各架构的 `PageTableEntry` + `PteFlags` 定义
//! - [`PageTableEntry`] / [`PteFlags`]：当前目标架构的类型别名（条件编译导出）
//!
//! 本 crate 无 `alloc` / `config` 依赖，可在 heap 未初始化的早期启动阶段使用。

#![no_std]

use memory_types::PhysAddr;

pub mod aarch64;
pub mod riscv64;

/// 页表项标志位的统一接口——各架构必须实现。
///
/// SAS 架构下只需要内核权限工厂——没有用户态，不做页回收（无需 A/D 位查询），
/// 权限修改通过 `OwnedPages::set_flags` 整包替换（无需 builder）。
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
