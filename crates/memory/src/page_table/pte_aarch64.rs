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

use super::{PageFlags, PageTableEntry};
use crate::address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

const VALID_BIT: u64 = 1 << 0;
const TABLE_BIT: u64 = 1 << 1;
const AF_BIT: u64 = 1 << 10;
const SH_INNER: u64 = 0b11 << 8;
const MAIR_IDX0: u64 = 0b000 << 2;
const AP_RO: u64 = 0b10 << 6;
const AP_RW: u64 = 0b00 << 6;
const PXN_BIT: u64 = 1 << 53;
const UXN_BIT: u64 = 1 << 54;

/// 输出地址掩码——根据 PAGE_SIZE 自动适配：
/// - 4KB (PAGE_SHIFT=12)：bits [47:12]
/// - 16KB (PAGE_SHIFT=14)：bits [47:14]
/// - 64KB (PAGE_SHIFT=16)：bits [47:16]
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF & !((1u64 << PAGE_SHIFT) - 1);

impl PageTableEntry {
    /// 从物理地址和标志构造叶/页描述符。
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let mut bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
            | VALID_BIT
            | TABLE_BIT
            | AF_BIT
            | SH_INNER
            | MAIR_IDX0;
        if flags.contains(PageFlags::WRITE) {
            bits |= AP_RW;
        } else {
            bits |= AP_RO;
        }
        if flags.contains(PageFlags::USER) {
            bits |= 0b01 << 6;
        }
        if !flags.contains(PageFlags::EXECUTE) {
            bits |= PXN_BIT | UXN_BIT;
        }
        if !flags.contains(PageFlags::GLOBAL) {
            bits |= 1 << 11;
        }
        Self(bits)
    }

    /// 构造表描述符（指向下一级页表）。
    #[inline]
    pub fn new_table(paddr: PhysAddr) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | VALID_BIT | TABLE_BIT;
        Self(bits)
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }

    /// 从 PTE 提取架构无关标志。
    pub fn flags(self) -> PageFlags {
        let mut f = PageFlags::empty();
        if self.is_valid() {
            f |= PageFlags::VALID;
        }
        let ap = (self.0 >> 6) & 0b11;
        f |= PageFlags::READ;
        if ap & 0b10 == 0 {
            f |= PageFlags::WRITE;
            f |= PageFlags::DIRTY;
        }
        if ap & 0b01 != 0 {
            f |= PageFlags::USER;
        }
        if self.0 & PXN_BIT == 0 {
            f |= PageFlags::EXECUTE;
        }
        if self.0 & AF_BIT != 0 {
            f |= PageFlags::ACCESSED;
        }
        if self.0 & (1 << 11) == 0 {
            f |= PageFlags::GLOBAL;
        }
        f
    }

    /// PTE 是否有效。
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & VALID_BIT != 0
    }

    /// 是否为叶节点（page/block 描述符，非 table 描述符）。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & AF_BIT != 0)
    }

    /// 空 PTE（全零）。
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }

    /// 中间节点（table 描述符）。
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new_table(paddr)
    }
}
