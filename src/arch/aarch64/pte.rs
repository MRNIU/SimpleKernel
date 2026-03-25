//! AArch64 4KB granule 页表项编码

use crate::memory::address::PhysAddr;
use crate::memory::page_table::{PageFlags, PageTableEntry};

// AArch64 descriptor 常量
const VALID_BIT: u64 = 1 << 0;
const TABLE_BIT: u64 = 1 << 1; // [1:0] = 0b11 → table/page descriptor
const AF_BIT: u64 = 1 << 10; // Access Flag
const SH_INNER: u64 = 0b11 << 8; // Inner Shareable
const MAIR_IDX0: u64 = 0b000 << 2; // MAIR index 0
const AP_RO: u64 = 0b10 << 6; // AP[2:1] = 0b10 → EL1 RO
const AP_RW: u64 = 0b00 << 6; // AP[2:1] = 0b00 → EL1 RW
const PXN_BIT: u64 = 1 << 53; // Privileged Execute-Never
const UXN_BIT: u64 = 1 << 54; // Unprivileged Execute-Never
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

/// 页表层级数（AArch64 4KB granule = 4 级）
pub const PT_LEVELS: usize = 4;

impl PageTableEntry {
    /// 构造 L3 页描述符（leaf）。
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let mut bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
            | VALID_BIT
            | TABLE_BIT // L3 页描述符 bit[1]=1
            | AF_BIT
            | SH_INNER
            | MAIR_IDX0;

        // 访问权限
        if flags.contains(PageFlags::WRITE) {
            bits |= AP_RW;
        } else {
            bits |= AP_RO;
        }

        // 执行权限：未设 EXECUTE 则禁止执行
        if !flags.contains(PageFlags::EXECUTE) {
            bits |= PXN_BIT | UXN_BIT;
        }

        Self(bits)
    }

    /// 构造 table 描述符（非叶，指向下一级页表）。
    #[inline]
    pub fn new_table(paddr: PhysAddr) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | VALID_BIT | TABLE_BIT;
        Self(bits)
    }

    /// 返回描述符中的输出（物理）地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }

    /// 将描述符位解码为 `PageFlags`。
    pub fn flags(self) -> PageFlags {
        let mut f = PageFlags::empty();
        if self.is_valid() {
            f |= PageFlags::VALID;
        }
        // AP[2:1]: RW = 0b00, RO = 0b10
        let ap = (self.0 >> 6) & 0b11;
        f |= PageFlags::READ;
        if ap == 0b00 {
            f |= PageFlags::WRITE;
        }
        if self.0 & PXN_BIT == 0 {
            f |= PageFlags::EXECUTE;
        }
        if self.0 & AF_BIT != 0 {
            f |= PageFlags::ACCESSED;
        }
        f
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & VALID_BIT != 0
    }

    /// L3 叶节点：valid + table-bit + AF（页描述符）。
    /// L0-L2 中 table-bit 表示 table 描述符（非叶）。
    /// 通过 AF 区分：仅页描述符设置 AF。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & AF_BIT != 0)
    }

    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }

    /// 构造中间节点（table 描述符）。
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new_table(paddr)
    }
}
