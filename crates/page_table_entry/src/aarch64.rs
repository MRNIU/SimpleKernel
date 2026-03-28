//! AArch64 ARMv8 PTE 编码。
//!
//! Stage 1 页描述符格式（4KB/16KB/64KB granule 共用结构，
//! 物理地址掩码随 PAGE_SIZE 变化）：
//! - bit [0]：Valid
//! - bit [1]：Table/Block 类型位（1 = table/page，0 = block）
//! - bits [4:2]：MAIR 索引
//! - bits [7:6]：AP (Access Permissions)
//! - bits [9:8]：SH (Shareability)
//! - bit [10]：AF (Access Flag)
//! - bit [11]：nG (non-Global)
//! - bits [47:12]/[47:14]/[47:16]：Output Address（随 granule 变化）
//! - bit [53]：PXN
//! - bit [54]：UXN/XN

use bitflags::bitflags;

use crate::{PageTableEntry, PteFlagsOps, PteOps};
use address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

/// 输出地址掩码——根据 PAGE_SIZE 自动适配：
/// - 4KB (PAGE_SHIFT=12)：bits [47:12]
/// - 16KB (PAGE_SHIFT=14)：bits [47:14]
/// - 64KB (PAGE_SHIFT=16)：bits [47:16]
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF & !((1u64 << PAGE_SHIFT) - 1);

bitflags! {
    /// AArch64 ARMv8 页表项标志位（硬件原生位位置）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
        /// 有效位
        const VALID     = 1 << 0;
        /// Table/Page 描述符（1 = table/page，0 = block）
        const TABLE     = 1 << 1;
        /// MAIR 索引 0（Normal memory）：bits [4:2] = 0b000。
        /// 零值常量——在 `|` 表达式中无实际作用，仅为文档可读性保留。
        /// Normal memory 即 MAIR 索引位全零时的默认选择。
        const MAIR_IDX0 = 0b000 << 2;
        /// MAIR 索引 1（Device-nGnRnE）：bits [4:2] = 0b001
        const MAIR_IDX1 = 0b001 << 2;
        /// AP[2:1] = 0b01：EL0 可访问（unprivileged access）
        const AP_UNPRIV = 0b01 << 6;
        /// AP[2:1] = 0b10：只读（EL1 read-only，EL0 不可访问）。
        /// 注意 AP 是双位字段 bits [7:6]，`AP_UNPRIV | AP_RO` = 0b11 表示
        /// 内核+用户均只读。
        const AP_RO     = 0b10 << 6;
        /// Inner Shareable：bits [9:8] = 0b11
        const SH_INNER  = 0b11 << 8;
        /// Access Flag
        const AF        = 1 << 10;
        /// non-Global
        const NG        = 1 << 11;
        /// Privileged Execute-Never
        const PXN       = 1 << 53;
        /// Unprivileged Execute-Never / Execute-Never
        const UXN       = 1 << 54;
        /// 软件定义位——bit 55，标记帧所有权（参考 Theseus EXCLUSIVE）。
        ///
        /// ARMv8 bits [58:55] 为软件可用位（IGNORED by hardware）。
        /// EXCLUSIVE = 1：unmap 时帧归还分配器。
        /// EXCLUSIVE = 0：unmap 时不回收帧。
        const EXCLUSIVE = 1 << 55;
    }
}

impl PteFlagsOps for PteFlags {
    /// 内核读写数据映射。
    ///
    /// VALID | TABLE | AF | SH_INNER | PXN | UXN。
    /// MAIR 索引 0（Normal memory）由 bits [4:2] 全零隐式选择。
    #[inline]
    fn kernel_rw() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::PXN | Self::UXN
    }

    /// 内核读-执行映射。
    ///
    /// AP_RO 使 EL1 只读，未设 PXN 允许 EL1 执行，UXN 禁止 EL0 执行。
    /// MAIR 索引 0（Normal memory）由 bits [4:2] 全零隐式选择。
    #[inline]
    fn kernel_rx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::AP_RO | Self::UXN
    }

    /// 内核只读映射。
    ///
    /// MAIR 索引 0（Normal memory）由 bits [4:2] 全零隐式选择。
    #[inline]
    fn kernel_ro() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::AP_RO | Self::PXN | Self::UXN
    }

    /// 内核读写执行映射。
    ///
    /// MAIR 索引 0（Normal memory）由 bits [4:2] 全零隐式选择。
    #[inline]
    fn kernel_rwx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::UXN
    }

    /// 设备 MMIO 映射（Device-nGnRnE，不可缓存，不可执行）。
    ///
    /// 使用 MAIR 索引 1（`MAIR_IDX1`），对应 Device-nGnRnE 属性。
    /// 内核态 MMIO 设备寄存器必须使用此 preset，
    /// 使用 Normal memory 属性会导致 CPU cache 与设备状态不一致。
    #[inline]
    fn kernel_device() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::MAIR_IDX1 | Self::PXN | Self::UXN
    }

    /// 是否具有写权限。
    #[inline]
    fn is_writable(self) -> bool {
        !self.contains(Self::AP_RO)
    }

    /// 将标志位适配为指定层级的叶描述符格式。
    ///
    /// ARMv8 在不同层级使用不同描述符格式：
    /// - Level 0 (page descriptor)：TABLE 位 = 1（bit[1]）
    /// - Level > 0 (block descriptor)：TABLE 位 = 0
    ///
    /// 所有 preset 默认设置 TABLE=1（适用于 Level 0）。
    /// 映射大页时必须调用此方法清除 TABLE 位。
    #[inline]
    fn for_leaf_at_level(self, level: usize) -> Self {
        if level == 0 {
            self
        } else {
            // Block descriptor：清除 TABLE 位
            self.difference(Self::TABLE)
        }
    }

    #[inline]
    fn is_exclusive(self) -> bool {
        self.contains(Self::EXCLUSIVE)
    }

    #[inline]
    fn with_exclusive(self) -> Self {
        self | Self::EXCLUSIVE
    }
}

/// AArch64 专用：构造 table descriptor（指向下一级页表）。
fn new_table(paddr: PhysAddr) -> PageTableEntry {
    let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
        | PteFlags::VALID.bits()
        | PteFlags::TABLE.bits();
    PageTableEntry(bits)
}

impl PteOps for PageTableEntry {
    type Flags = PteFlags;

    /// 从物理地址和标志构造叶/页描述符。
    #[inline]
    fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        Self((paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | flags.bits())
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }

    /// 从 PTE 提取标志位（硬件原生位）。
    #[inline]
    fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & !OUTPUT_ADDR_MASK)
    }

    /// PTE 是否有效。
    #[inline]
    fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

    /// 是否为叶节点（page/block 描述符，非 table 描述符）。
    ///
    /// ARMv8 的叶判断依赖层级：
    /// - Level 0（最低级）：所有有效项都是 page descriptor（叶），TABLE 位 = 1
    /// - Level 1-3：TABLE 位 = 0 表示 block descriptor（叶）
    #[inline]
    fn is_leaf(self, level: usize) -> bool {
        if !self.is_valid() {
            return false;
        }
        if level == 0 {
            true
        } else {
            self.0 & PteFlags::TABLE.bits() == 0
        }
    }

    /// 空 PTE（全零）。
    #[inline]
    fn empty() -> Self {
        Self(0)
    }

    /// 中间节点（table 描述符）。
    #[inline]
    fn new_intermediate(paddr: PhysAddr) -> Self {
        new_table(paddr)
    }
}
