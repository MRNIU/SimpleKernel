//! AArch64 ARMv8 PTE 编码。
//!
//! Stage 1 页描述符格式（4KB granule）：
//! - bit [0]：Valid
//! - bit [1]：Table/Block 类型位（1 = table/page，0 = block）
//! - bits [4:2]：MAIR 索引
//! - bits [7:6]：AP (Access Permissions)
//! - bits [9:8]：SH (Shareability)
//! - bit [10]：AF (Access Flag)
//! - bit [11]：nG (non-Global)
//! - bits [47:12]：Output Address
//! - bit [53]：PXN
//! - bit [54]：UXN/XN
//!
//! [Arm ARM §D8.3](https://developer.arm.com/documentation/ddi0487/latest)

use bitflags::bitflags;

use crate::{PteFlagsOps, PteOps};
use memory_types::PhysAddr;

/// 输出地址掩码：bits [47:12]（4KB granule）
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

/// AArch64 页表项（64 位）。
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("PageTableEntry")
            .field(&format_args!("{:#018x}", self.0))
            .finish()
    }
}

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
    }
}

/// 标志位掩码——所有已定义标志位的并集，用于从 PTE 中精确提取标志。
const FLAGS_MASK: u64 = PteFlags::all().bits();

impl PteFlagsOps for PteFlags {
    #[inline]
    fn kernel_rw() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::PXN | Self::UXN
    }

    #[inline]
    fn kernel_rx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::AP_RO | Self::UXN
    }

    #[inline]
    fn kernel_ro() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::AP_RO | Self::PXN | Self::UXN
    }

    /// 内核读写执行映射。
    ///
    /// UXN = 1 阻止 EL0 执行此页——即使内核允许 RWX，用户态仍不可执行，
    /// 防止特权提升后利用内核映射执行代码。
    #[inline]
    fn kernel_rwx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::UXN
    }

    /// 设备 MMIO 映射（Device-nGnRnE，不可缓存、不可执行）。
    ///
    /// 未设置 `SH_INNER`：Arm ARM §D8.5.10 规定 Device 内存类型下
    /// shareability 字段被硬件忽略，省略以避免误导。
    #[inline]
    fn kernel_device() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::MAIR_IDX1 | Self::PXN | Self::UXN
    }

    /// 用户态读写数据映射（不可执行）。
    ///
    /// - `AP_UNPRIV`：允许 EL0 访问
    /// - `NG`：non-Global，TLB 条目绑定 ASID（per-process）
    /// - `PXN | UXN`：数据页不可执行（W^X）
    #[inline]
    fn user_rw() -> Self {
        Self::VALID
            | Self::TABLE
            | Self::AF
            | Self::SH_INNER
            | Self::AP_UNPRIV
            | Self::NG
            | Self::PXN
            | Self::UXN
    }

    /// 用户态读-执行映射（不可写）。
    ///
    /// - `AP_UNPRIV | AP_RO`：EL0 + EL1 均只读
    /// - `PXN`：禁止内核执行用户代码页
    /// - 不设 `UXN`：允许用户态执行
    #[inline]
    fn user_rx() -> Self {
        Self::VALID
            | Self::TABLE
            | Self::AF
            | Self::SH_INNER
            | Self::AP_UNPRIV
            | Self::AP_RO
            | Self::NG
            | Self::PXN
    }

    /// 用户态只读映射。
    #[inline]
    fn user_ro() -> Self {
        Self::VALID
            | Self::TABLE
            | Self::AF
            | Self::SH_INNER
            | Self::AP_UNPRIV
            | Self::AP_RO
            | Self::NG
            | Self::PXN
            | Self::UXN
    }

    /// 用户态读写执行映射。
    ///
    /// - `PXN`：禁止内核执行
    /// - 不设 `UXN`：允许用户态执行
    /// - 不设 `AP_RO`：允许写入
    #[inline]
    fn user_rwx() -> Self {
        Self::VALID
            | Self::TABLE
            | Self::AF
            | Self::SH_INNER
            | Self::AP_UNPRIV
            | Self::NG
            | Self::PXN
    }

    /// AArch64 中 Valid 的页面始终可读——没有独立的 "读" 控制位。
    #[inline]
    fn is_readable(self) -> bool {
        self.contains(Self::VALID)
    }

    /// AArch64 的写权限由 AP_RO 位反向控制——AP_RO=0 表示可写。
    #[inline]
    fn is_writable(self) -> bool {
        !self.contains(Self::AP_RO)
    }

    /// AArch64 的执行权限由 XN 位反向控制。
    /// 内核页看 PXN，用户页看 UXN。这里取保守策略：任一 XN 位未设置即视为可执行。
    #[inline]
    fn is_executable(self) -> bool {
        !self.contains(Self::PXN) || !self.contains(Self::UXN)
    }

    #[inline]
    fn is_user(self) -> bool {
        self.contains(Self::AP_UNPRIV)
    }

    #[inline]
    fn is_accessed(self) -> bool {
        self.contains(Self::AF)
    }

    /// AArch64 没有硬件 Dirty 位（在不启用 DBM 时）。
    /// 这里用 "可写 + 已访问" 近似判断。
    #[inline]
    fn is_dirty(self) -> bool {
        self.is_writable() && self.is_accessed()
    }

    #[inline]
    fn with_writable(self, w: bool) -> Self {
        if w {
            self.difference(Self::AP_RO)
        } else {
            self | Self::AP_RO
        }
    }

    /// 设置或清除执行权限——同时操作 PXN 和 UXN。
    #[inline]
    fn with_executable(self, x: bool) -> Self {
        if x {
            self.difference(Self::PXN | Self::UXN)
        } else {
            self | Self::PXN | Self::UXN
        }
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
            self.difference(Self::TABLE)
        }
    }
}

impl PteOps for PageTableEntry {
    type Flags = PteFlags;

    #[inline]
    fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        Self((paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | flags.bits())
    }

    #[inline]
    fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }

    #[inline]
    fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & FLAGS_MASK)
    }

    #[inline]
    fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

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

    #[inline]
    fn empty() -> Self {
        Self(0)
    }

    #[inline]
    fn new_intermediate(paddr: PhysAddr) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
            | PteFlags::VALID.bits()
            | PteFlags::TABLE.bits();
        Self(bits)
    }

    #[inline]
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    #[inline]
    fn as_raw(self) -> u64 {
        self.0
    }
}
