//! AArch64 ARMv8 PTE 编码。

use super::{PageFlags, PageTableEntry};
use crate::address::PhysAddr;

const VALID_BIT: u64 = 1 << 0;
const TABLE_BIT: u64 = 1 << 1;
const AF_BIT: u64 = 1 << 10;
const SH_INNER: u64 = 0b11 << 8;
const MAIR_IDX0: u64 = 0b000 << 2;
const AP_RO: u64 = 0b10 << 6;
const AP_RW: u64 = 0b00 << 6;
const PXN_BIT: u64 = 1 << 53;
const UXN_BIT: u64 = 1 << 54;
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

impl PageTableEntry {
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
    #[inline]
    pub fn new_table(paddr: PhysAddr) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | VALID_BIT | TABLE_BIT;
        Self(bits)
    }
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }
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
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & VALID_BIT != 0
    }
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & AF_BIT != 0)
    }
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new_table(paddr)
    }
}
