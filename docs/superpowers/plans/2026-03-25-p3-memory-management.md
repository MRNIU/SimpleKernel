# P3 Memory Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement physical frame allocation, virtual memory (page tables), and heap allocator — producing `PhysAddr`/`VirtAddr` types as foundation for all subsequent phases.

**Architecture:** Identity-mapped kernel (VA == PA) with Sv39 on riscv64 and 4-level 4KB-granule tables on aarch64. Heap uses `buddy_system_allocator::LockedHeap` as `#[global_allocator]` backed by a static BSS array. Frame allocator wraps a second `buddy_system_allocator` instance. Page tables own their nodes via `Vec<FrameTracker>` RAII, ensuring automatic physical memory reclamation on drop.

**Tech Stack:** `buddy_system_allocator` (heap + frame alloc), `bitflags` (page flags), Rust newtype pattern (`PhysAddr`/`VirtAddr`), `riscv`/`aarch64-cpu` crates for register access.

**Key References:**
- Design spec: `docs/rust-rewrite/P3-内存管理.md`
- Infrastructure evolution: `docs/rust-rewrite/基础设施演进.md`
- Ecosystem patterns: `docs/rust-rewrite/生态调研与改进建议.md`

---

## File Structure

```
src/
├── config.rs                  — ADD: PAGE_SIZE, KERNEL_HEAP_SIZE constants
├── main.rs                    — ADD: extern crate alloc, mod memory, mod scope_guard
├── scope_guard.rs             — CREATE: ScopeGuard RAII cleanup guard
├── per_cpu.rs                 — MODIFY: BasicInfo fields u64 → PhysAddr
├── panic.rs                   — MODIFY: PanicEvent.pc u64 → VirtAddr
├── memory/
│   ├── mod.rs                 — CREATE: memory_init(), memory_init_smp(), phys_to_virt()
│   ├── address.rs             — CREATE: PhysAddr, VirtAddr newtypes + arithmetic
│   ├── heap.rs                — CREATE: #[global_allocator] via LockedHeap
│   ├── frame.rs               — CREATE: FrameTracker RAII + frame allocator
│   └── page_table.rs          — CREATE: PageFlags, PageTableEntry, PageTable, MapPage/UnmapPage
├── arch/riscv64/init.rs       — MODIFY: use PhysAddr in BasicInfo construction
└── arch/aarch64/init.rs       — MODIFY: use PhysAddr in BasicInfo construction
```

---

## Task 1: PhysAddr/VirtAddr Address Types

**Files:**
- Create: `src/memory/address.rs`
- Create: `src/memory/mod.rs` (minimal — just module declaration)
- Modify: `src/main.rs` — add `mod memory`

### Step 1.1: Create module skeleton

- [ ] Create `src/memory/mod.rs` with just `pub mod address;`
- [ ] Add `mod memory;` to `src/main.rs` (after `mod logging;`, with `#[cfg(not(test))]` — we will add test support once address types are ready)

### Step 1.2: Implement PhysAddr

- [ ] Create `src/memory/address.rs` with:

```rust
use core::fmt;
use core::ops::{Add, Sub};
use crate::config::PAGE_SIZE;

/// Physical address newtype — cannot be mixed with VirtAddr at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn new(addr: usize) -> Self { Self(addr) }
    pub const fn as_usize(self) -> usize { self.0 }
    pub const fn page_offset(self) -> usize { self.0 & (PAGE_SIZE - 1) }
    pub const fn is_aligned(self) -> bool { self.page_offset() == 0 }
    pub const fn align_down(self) -> Self { Self(self.0 & !(PAGE_SIZE - 1)) }
    pub const fn align_up(self) -> Self {
        Self((self.0 + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
    }
}

impl Add<usize> for PhysAddr {
    type Output = Self;
    fn add(self, rhs: usize) -> Self { Self(self.0 + rhs) }
}

impl Sub<usize> for PhysAddr {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self { Self(self.0 - rhs) }
}

impl Sub<PhysAddr> for PhysAddr {
    type Output = usize;
    fn sub(self, rhs: PhysAddr) -> usize { self.0 - rhs.0 }
}

impl fmt::Display for PhysAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}
```

### Step 1.3: Implement VirtAddr (same structure)

- [ ] Add `VirtAddr` to `address.rs` — same derive, methods, and trait impls as `PhysAddr`.

### Step 1.4: Add PAGE_SIZE to config.rs

- [ ] Add to `src/config.rs`:

```rust
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_BITS: usize = 12;
```

### Step 1.5: Add unit tests for address types

- [ ] Add `#[cfg(test)] mod tests` at the bottom of `address.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phys_addr_alignment() {
        assert!(PhysAddr::new(0x1000).is_aligned());
        assert!(!PhysAddr::new(0x1001).is_aligned());
        assert_eq!(PhysAddr::new(0x1234).align_down(), PhysAddr::new(0x1000));
        assert_eq!(PhysAddr::new(0x1234).align_up(), PhysAddr::new(0x2000));
        assert_eq!(PhysAddr::new(0x1000).align_up(), PhysAddr::new(0x1000));
    }

    #[test]
    fn phys_addr_arithmetic() {
        let a = PhysAddr::new(0x2000);
        assert_eq!((a + 0x100).as_usize(), 0x2100);
        assert_eq!((a - 0x100).as_usize(), 0x1F00);
        assert_eq!(a - PhysAddr::new(0x1000), 0x1000);
    }

    #[test]
    fn phys_addr_display() {
        use core::fmt::Write;
        let mut buf = [0u8; 32];
        let mut cursor = crate::fmt_buf::FmtBuf::new();
        let _ = write!(cursor, "{}", PhysAddr::new(0x80200000));
        assert_eq!(cursor.as_str(), "0x0000000080200000");
    }

    #[test]
    fn virt_addr_alignment() {
        assert!(VirtAddr::new(0x1000).is_aligned());
        assert!(!VirtAddr::new(0x1001).is_aligned());
    }

    #[test]
    fn virt_addr_arithmetic() {
        let a = VirtAddr::new(0x2000);
        assert_eq!((a + 0x100).as_usize(), 0x2100);
        assert_eq!(a - VirtAddr::new(0x1000), 0x1000);
    }
}
```

### Step 1.6: Run tests, verify pass

- [ ] Run: `cargo test -- address`
- [ ] Expected: All 5 address tests pass

### Step 1.7: Commit

- [ ] `git add src/memory/ src/config.rs src/main.rs`
- [ ] `git commit -m "feat(P3): add PhysAddr/VirtAddr newtype address types"`

---

## Task 2: ScopeGuard Cleanup Guard

**Files:**
- Create: `src/scope_guard.rs`
- Modify: `src/main.rs` — add `mod scope_guard`

### Step 2.1: Implement ScopeGuard

- [ ] Create `src/scope_guard.rs`:

```rust
/// RAII cleanup guard — runs cleanup on Drop, dismiss() cancels it.
/// Pattern from Linux kernel Rust — used for init failure rollback.
pub struct ScopeGuard<F: FnOnce()> {
    cleanup: Option<F>,
}

impl<F: FnOnce()> ScopeGuard<F> {
    pub fn new(cleanup: F) -> Self {
        Self { cleanup: Some(cleanup) }
    }

    /// Cancel cleanup (call on success path).
    pub fn dismiss(mut self) {
        self.cleanup = None;
    }
}

impl<F: FnOnce()> Drop for ScopeGuard<F> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}
```

### Step 2.2: Add unit tests

- [ ] Add tests in `scope_guard.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn runs_cleanup_on_drop() {
        let ran = Cell::new(false);
        {
            let _guard = ScopeGuard::new(|| ran.set(true));
        }
        assert!(ran.get());
    }

    #[test]
    fn dismiss_prevents_cleanup() {
        let ran = Cell::new(false);
        {
            let guard = ScopeGuard::new(|| ran.set(true));
            guard.dismiss();
        }
        assert!(!ran.get());
    }
}
```

### Step 2.3: Wire into main.rs, run tests, commit

- [ ] Add `mod scope_guard;` to `src/main.rs`
- [ ] Run: `cargo test -- scope_guard`
- [ ] Expected: 2 tests pass
- [ ] `git commit -m "feat(P3): add ScopeGuard RAII cleanup guard"`

---

## Task 3: Heap Allocator (#[global_allocator])

**Files:**
- Create: `src/memory/heap.rs`
- Modify: `src/memory/mod.rs` — add `pub mod heap`
- Modify: `src/main.rs` — add `extern crate alloc`
- Modify: `src/config.rs` — add `KERNEL_HEAP_SIZE`

### Step 3.1: Add heap size constant

- [ ] Add to `src/config.rs`:

```rust
/// Kernel heap size: 4 MB (backed by static BSS array)
pub const KERNEL_HEAP_SIZE: usize = 4 * 1024 * 1024;
```

### Step 3.2: Implement heap allocator

- [ ] Create `src/memory/heap.rs`:

```rust
use buddy_system_allocator::LockedHeap;
use crate::config::KERNEL_HEAP_SIZE;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

/// BSS-resident heap backing store.
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// Initialize the kernel heap allocator.
///
/// # Safety
/// Must be called exactly once, before any heap allocation.
pub unsafe fn heap_init() {
    // SAFETY: HEAP_SPACE is a static mut only accessed here during single-core init.
    let heap_start = unsafe { HEAP_SPACE.as_ptr() as usize };
    unsafe {
        HEAP_ALLOCATOR.lock().init(heap_start, KERNEL_HEAP_SIZE);
    }
    log::info!(
        "HeapInit: {}MB heap at {:#x}",
        KERNEL_HEAP_SIZE / (1024 * 1024),
        heap_start
    );
}
```

### Step 3.3: Wire into main.rs

- [ ] Add to `src/main.rs` at the top (after `#![...]` attributes):

```rust
#[cfg(not(test))]
extern crate alloc;
```

- [ ] Add `pub mod heap;` to `src/memory/mod.rs`

### Step 3.4: Commit

- [ ] `git commit -m "feat(P3): add kernel heap allocator with buddy_system_allocator"`

---

## Task 4: Frame Allocator with FrameTracker RAII

**Files:**
- Create: `src/memory/frame.rs`
- Modify: `src/memory/mod.rs` — add `pub mod frame`

### Step 4.1: Implement frame allocator and FrameTracker

- [ ] Create `src/memory/frame.rs`:

```rust
use crate::config::PAGE_SIZE;
use crate::memory::address::PhysAddr;
use buddy_system_allocator::LockedHeap;
use crate::sync::SpinLock;
use core::sync::atomic::AtomicBool;

/// Frame allocator wrapping buddy_system_allocator.
/// Operates in units of PAGE_SIZE (4096 bytes).
static FRAME_ALLOCATOR: SpinLock<FrameAllocatorInner> =
    SpinLock::new(FrameAllocatorInner::new(), "frame_alloc");

struct FrameAllocatorInner {
    allocator: buddy_system_allocator::Heap<32>,
    initialized: bool,
}

impl FrameAllocatorInner {
    const fn new() -> Self {
        Self {
            allocator: buddy_system_allocator::Heap::empty(),
            initialized: false,
        }
    }
}

/// Initialize the frame allocator with available physical memory.
///
/// `start` must be page-aligned. The region `[start, start+size)` becomes
/// available for frame allocation.
///
/// # Safety
/// The memory region must be valid, not overlap with kernel/heap, and
/// must be called exactly once.
pub unsafe fn frame_init(start: PhysAddr, size: usize) {
    let mut alloc = FRAME_ALLOCATOR.lock();
    assert!(!alloc.initialized, "frame_init called twice");
    assert!(start.is_aligned(), "frame_init: start not page-aligned");
    unsafe {
        alloc.allocator.init(start.as_usize(), size);
    }
    alloc.initialized = true;
    log::info!(
        "FrameInit: {} MB available from {}",
        size / (1024 * 1024),
        start
    );
}

/// Physical frame RAII guard — automatically returns frame to allocator on Drop.
/// Eliminates "forgot to free_frame" physical memory leaks.
pub struct FrameTracker {
    paddr: PhysAddr,
}

impl FrameTracker {
    /// Allocate a single physical frame (PAGE_SIZE bytes), zeroed.
    pub fn alloc() -> Result<Self, crate::error::ErrorCode> {
        let mut alloc = FRAME_ALLOCATOR.lock();
        if !alloc.initialized {
            return Err(crate::error::ErrorCode::VmAllocationFailed);
        }
        let addr = alloc
            .allocator
            .alloc(core::alloc::Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap())
            .map(|ptr| ptr.as_ptr() as usize)
            .ok_or(crate::error::ErrorCode::OutOfMemory)?;

        // Zero the frame
        unsafe {
            core::ptr::write_bytes(addr as *mut u8, 0, PAGE_SIZE);
        }

        Ok(Self {
            paddr: PhysAddr::new(addr),
        })
    }

    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        let mut alloc = FRAME_ALLOCATOR.lock();
        unsafe {
            alloc.allocator.dealloc(
                core::ptr::NonNull::new_unchecked(self.paddr.as_usize() as *mut u8),
                core::alloc::Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap(),
            );
        }
    }
}
```

### Step 4.2: Add to module, commit

- [ ] Add `pub mod frame;` to `src/memory/mod.rs`
- [ ] `git commit -m "feat(P3): add FrameTracker RAII and physical frame allocator"`

---

## Task 5: Page Table Infrastructure (PageFlags + PageTableEntry + PageTable)

**Files:**
- Create: `src/memory/page_table.rs`
- Modify: `src/memory/mod.rs` — add `pub mod page_table`

### Step 5.1: Implement PageFlags and PageTableEntry

- [ ] Create `src/memory/page_table.rs` with RISC-V Sv39 / AArch64 dual support:

```rust
use bitflags::bitflags;
use crate::config::PAGE_SIZE;
use crate::memory::address::{PhysAddr, VirtAddr};
use crate::memory::frame::FrameTracker;
use crate::error::{ErrorCode, KResult};

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PageFlags: u64 {
        const VALID    = 1 << 0;
        const READ     = 1 << 1;
        const WRITE    = 1 << 2;
        const EXECUTE  = 1 << 3;
        const USER     = 1 << 4;
        const GLOBAL   = 1 << 5;
        const ACCESSED = 1 << 6;
        const DIRTY    = 1 << 7;
    }
}

impl PageFlags {
    /// Kernel read-write data mapping.
    pub const fn kernel_rw() -> Self {
        Self::from_bits_truncate(
            Self::VALID.bits() | Self::READ.bits() | Self::WRITE.bits()
            | Self::ACCESSED.bits() | Self::DIRTY.bits()
        )
    }

    /// Kernel read-execute code mapping.
    pub const fn kernel_rx() -> Self {
        Self::from_bits_truncate(
            Self::VALID.bits() | Self::READ.bits() | Self::EXECUTE.bits()
            | Self::ACCESSED.bits()
        )
    }

    /// Kernel read-only mapping.
    pub const fn kernel_ro() -> Self {
        Self::from_bits_truncate(
            Self::VALID.bits() | Self::READ.bits() | Self::ACCESSED.bits()
        )
    }
}

/// Page table entry — wraps architecture-specific bit layout.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

// ─── RISC-V Sv39 layout ───────────────────────────────────────
// PTE: [63:54] reserved | [53:10] PPN | [9:8] RSW | [7:0] flags
#[cfg(target_arch = "riscv64")]
impl PageTableEntry {
    const PPN_SHIFT: u64 = 10;
    const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00; // bits 53:10
    const FLAGS_MASK: u64 = 0xFF; // bits 7:0

    pub const fn empty() -> Self { Self(0) }

    pub const fn is_valid(self) -> bool { self.0 & PageFlags::VALID.bits() != 0 }

    pub const fn is_leaf(self) -> bool {
        // A leaf PTE has at least one of R/W/X set
        self.0 & (PageFlags::READ.bits() | PageFlags::WRITE.bits() | PageFlags::EXECUTE.bits()) != 0
    }

    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0 & Self::FLAGS_MASK)
    }

    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new(((self.0 & Self::PPN_MASK) >> Self::PPN_SHIFT << 12) as usize)
    }

    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = (paddr.as_usize() as u64 >> 12) << Self::PPN_SHIFT;
        Self(ppn | flags.bits())
    }
}

// ─── AArch64 4KB granule layout ──────────────────────────────
// Table descriptor (L0-L2): [47:12] next-level table addr | [1:0]=0b11
// Page descriptor (L3):     [47:12] output addr | attrs | [1:0]=0b11
#[cfg(target_arch = "aarch64")]
impl PageTableEntry {
    const ADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000; // bits 47:12
    const VALID_BIT: u64 = 1 << 0;
    const TABLE_BIT: u64 = 1 << 1; // 1 = table/page, 0 = block

    // Stage 1 lower attributes for pages
    const ATTR_IDX_NORMAL: u64 = 0 << 2;  // MAIR index 0 = normal memory
    const ATTR_AP_RW: u64 = 0b00 << 6;    // EL1 R/W
    const ATTR_AP_RO: u64 = 0b10 << 6;    // EL1 R/O
    const ATTR_SH_INNER: u64 = 0b11 << 8; // inner shareable
    const ATTR_AF: u64 = 1 << 10;         // access flag
    const ATTR_PXN: u64 = 1 << 53;        // privileged execute-never
    const ATTR_UXN: u64 = 1 << 54;        // unprivileged execute-never

    pub const fn empty() -> Self { Self(0) }

    pub const fn is_valid(self) -> bool { self.0 & Self::VALID_BIT != 0 }

    pub const fn is_leaf(self) -> bool {
        // In L3, all valid entries are leaves. In L0-L2, block entries (bit 1 = 0) are leaves.
        // We use L3 pages only, so valid + table bit = leaf page.
        self.is_valid()
    }

    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & Self::ADDR_MASK) as usize)
    }

    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let mut bits = (paddr.as_usize() as u64 & Self::ADDR_MASK)
            | Self::VALID_BIT
            | Self::TABLE_BIT
            | Self::ATTR_AF
            | Self::ATTR_SH_INNER
            | Self::ATTR_IDX_NORMAL;

        if !flags.contains(PageFlags::WRITE) {
            bits |= Self::ATTR_AP_RO;
        }
        if !flags.contains(PageFlags::EXECUTE) {
            bits |= Self::ATTR_PXN | Self::ATTR_UXN;
        }
        Self(bits)
    }

    pub fn new_table(paddr: PhysAddr) -> Self {
        Self((paddr.as_usize() as u64 & Self::ADDR_MASK)
            | Self::VALID_BIT
            | Self::TABLE_BIT)
    }

    pub fn flags(self) -> PageFlags {
        let mut f = PageFlags::VALID;
        f |= PageFlags::READ; // all valid entries are readable on aarch64
        if self.0 & Self::ATTR_AP_RO == 0 { f |= PageFlags::WRITE; }
        if self.0 & Self::ATTR_PXN == 0 { f |= PageFlags::EXECUTE; }
        f
    }
}

// ─── Host fallback for tests ────────────────────────────────
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
impl PageTableEntry {
    const PPN_SHIFT: u64 = 10;
    const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;
    const FLAGS_MASK: u64 = 0xFF;

    pub const fn empty() -> Self { Self(0) }
    pub const fn is_valid(self) -> bool { self.0 & PageFlags::VALID.bits() != 0 }
    pub const fn is_leaf(self) -> bool {
        self.0 & (PageFlags::READ.bits() | PageFlags::WRITE.bits() | PageFlags::EXECUTE.bits()) != 0
    }
    pub fn flags(self) -> PageFlags {
        PageFlags::from_bits_truncate(self.0 & Self::FLAGS_MASK)
    }
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new(((self.0 & Self::PPN_MASK) >> Self::PPN_SHIFT << 12) as usize)
    }
    pub fn new(paddr: PhysAddr, flags: PageFlags) -> Self {
        let ppn = (paddr.as_usize() as u64 >> 12) << Self::PPN_SHIFT;
        Self(ppn | flags.bits())
    }
}

const ENTRIES_PER_PAGE: usize = PAGE_SIZE / 8; // 512

/// Number of page table levels.
#[cfg(target_arch = "riscv64")]
const PT_LEVELS: usize = 3; // Sv39
#[cfg(target_arch = "aarch64")]
const PT_LEVELS: usize = 4; // 4KB granule, 48-bit VA
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
const PT_LEVELS: usize = 3; // host tests: use Sv39 layout

/// Extract VPN index for the given level from a virtual address.
/// Level 0 is the leaf level, level (PT_LEVELS-1) is the root.
fn vpn_index(va: VirtAddr, level: usize) -> usize {
    (va.as_usize() >> (12 + level * 9)) & 0x1FF
}

/// Page table — owns all page-table node frames via FrameTracker.
/// Dropping the page table automatically reclaims all physical frames.
pub struct PageTable {
    root: FrameTracker,
    /// All allocated page-table node frames (including root).
    frames: alloc::vec::Vec<FrameTracker>,
}

impl PageTable {
    /// Create a new empty page table.
    pub fn new() -> KResult<Self> {
        let root = FrameTracker::alloc()?;
        Ok(Self {
            root,
            frames: alloc::vec::Vec::new(),
        })
    }

    /// Physical address of the root page table.
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.paddr()
    }

    /// Get a mutable reference to the PTE array of a given frame.
    fn pte_array(paddr: PhysAddr) -> &'static mut [PageTableEntry; ENTRIES_PER_PAGE] {
        unsafe { &mut *(paddr.as_usize() as *mut [PageTableEntry; ENTRIES_PER_PAGE]) }
    }

    /// Walk the page table to find or create the leaf PTE for `va`.
    /// Allocates intermediate page table nodes as needed.
    fn find_or_create_pte(&mut self, va: VirtAddr) -> KResult<&'static mut PageTableEntry> {
        let mut paddr = self.root.paddr();
        // Walk from root (highest level) down to level 1
        for level in (1..PT_LEVELS).rev() {
            let idx = vpn_index(va, level);
            let table = Self::pte_array(paddr);
            if !table[idx].is_valid() {
                // Allocate a new page table node
                let frame = FrameTracker::alloc()?;
                #[cfg(any(target_arch = "riscv64", not(any(target_arch = "riscv64", target_arch = "aarch64"))))]
                {
                    table[idx] = PageTableEntry::new(frame.paddr(), PageFlags::VALID);
                }
                #[cfg(target_arch = "aarch64")]
                {
                    table[idx] = PageTableEntry::new_table(frame.paddr());
                }
                self.frames.push(frame);
            }
            paddr = table[idx].paddr();
        }
        let idx = vpn_index(va, 0);
        let table = Self::pte_array(paddr);
        Ok(&mut table[idx])
    }

    /// Find existing leaf PTE for `va`, returns None if not mapped.
    fn find_pte(&self, va: VirtAddr) -> Option<&'static PageTableEntry> {
        let mut paddr = self.root.paddr();
        for level in (1..PT_LEVELS).rev() {
            let idx = vpn_index(va, level);
            let table = Self::pte_array(paddr);
            if !table[idx].is_valid() {
                return None;
            }
            paddr = table[idx].paddr();
        }
        let idx = vpn_index(va, 0);
        let table = Self::pte_array(paddr);
        Some(&table[idx])
    }

    /// Map a single page: va → paddr with given flags.
    pub fn map_page(&mut self, va: VirtAddr, pa: PhysAddr, flags: PageFlags) -> KResult<()> {
        let pte = self.find_or_create_pte(va)?;
        if pte.is_valid() {
            return Err(ErrorCode::VmMapFailed); // already mapped
        }
        *pte = PageTableEntry::new(pa, flags);
        Ok(())
    }

    /// Unmap a single page, returns the previously mapped physical address.
    pub fn unmap_page(&mut self, va: VirtAddr) -> KResult<PhysAddr> {
        let pte = self.find_or_create_pte(va)?;
        if !pte.is_valid() {
            return Err(ErrorCode::VmPageNotMapped);
        }
        let pa = pte.paddr();
        *pte = PageTableEntry::empty();
        Ok(pa)
    }

    /// Look up the mapping for `va`.
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PageFlags)> {
        self.find_pte(va).and_then(|pte| {
            if pte.is_valid() && pte.is_leaf() {
                Some((pte.paddr(), pte.flags()))
            } else {
                None
            }
        })
    }
}

/// Activate this page table (write to satp / TTBR).
///
/// # Safety
/// The page table must contain valid identity mappings for all kernel code/data,
/// otherwise the CPU will fault immediately.
#[cfg(all(target_arch = "riscv64", not(test)))]
pub unsafe fn activate_page_table(pt: &PageTable) {
    let ppn = pt.root_paddr().as_usize() >> 12;
    let satp = (8usize << 60) | ppn; // MODE=8 (Sv39)
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp,
        );
    }
}

#[cfg(all(target_arch = "aarch64", not(test)))]
pub unsafe fn activate_page_table(pt: &PageTable) {
    let ttbr = pt.root_paddr().as_usize() as u64;
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {ttbr}",
            "isb",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            ttbr = in(reg) ttbr,
        );
    }
    // Enable MMU via SCTLR_EL1.M (bit 0)
    let mut sctlr: u64;
    unsafe {
        core::arch::asm!("mrs {sctlr}, sctlr_el1", sctlr = out(reg) sctlr);
        sctlr |= 1; // set M bit
        core::arch::asm!(
            "msr sctlr_el1, {sctlr}",
            "isb",
            sctlr = in(reg) sctlr,
        );
    }
}

#[cfg(any(test, not(any(target_arch = "riscv64", target_arch = "aarch64"))))]
pub unsafe fn activate_page_table(_pt: &PageTable) {
    // No-op on host
}
```

### Step 5.2: Add to module, commit

- [ ] Add `pub mod page_table;` to `src/memory/mod.rs`
- [ ] `git commit -m "feat(P3): add PageTable with Sv39/AArch64 support, map/unmap operations"`

---

## Task 6: Backfill BasicInfo with PhysAddr

**Files:**
- Modify: `src/per_cpu.rs` — change `u64` fields to `PhysAddr`
- Modify: `src/arch/riscv64/init.rs` — update BasicInfo construction
- Modify: `src/arch/aarch64/init.rs` — update BasicInfo construction
- Modify: `src/panic.rs` — update `PanicEvent.pc` to `VirtAddr`
- Modify: `src/main.rs` — update `phase2_smoke_test` elf_addr usage

### Step 6.1: Update BasicInfo fields

- [ ] In `src/per_cpu.rs`, change `BasicInfo`:

```rust
use crate::memory::address::PhysAddr;

pub struct BasicInfo {
    pub physical_memory_addr: PhysAddr,
    pub physical_memory_size: usize,
    pub kernel_addr: PhysAddr,
    pub kernel_size: usize,
    pub elf_addr: PhysAddr,
    pub fdt_addr: PhysAddr,
    pub core_count: usize,
}
```

Update `BasicInfo::new()` to use `PhysAddr::new(0)`.

### Step 6.2: Update arch init files

- [ ] In `src/arch/riscv64/init.rs`, update `BASIC_INFO.call_once(...)`:

```rust
use crate::memory::address::PhysAddr;
// ...
BASIC_INFO.call_once(|| BasicInfo {
    physical_memory_addr: PhysAddr::new(mem_addr as usize),
    physical_memory_size: mem_size,
    kernel_addr: PhysAddr::new(kernel_start as usize),
    kernel_size: (kernel_end - kernel_start) as usize,
    elf_addr: PhysAddr::new(kernel_start as usize),
    fdt_addr: PhysAddr::new(dtb_addr),
    core_count,
});
```

- [ ] Same pattern in `src/arch/aarch64/init.rs`.

### Step 6.3: Update panic.rs PanicEvent.pc

- [ ] Change `pc: u64` to `pc: VirtAddr` in `PanicEvent`, update construction sites.

### Step 6.4: Update phase2_smoke_test elf_addr usage

- [ ] In `src/main.rs`, update the `elf_addr` usage to call `.as_usize() as u64`:

```rust
let elf_addr = per_cpu::BASIC_INFO
    .get()
    .expect("BASIC_INFO not initialized")
    .elf_addr
    .as_usize() as u64;
```

### Step 6.5: Verify compilation and tests

- [ ] Run: `cargo test`
- [ ] Expected: All existing tests pass
- [ ] `git commit -m "feat(P3): backfill BasicInfo fields to PhysAddr, PanicEvent.pc to VirtAddr"`

---

## Task 7: memory_init(), memory_init_smp(), and Integration

**Files:**
- Modify: `src/memory/mod.rs` — add memory_init, memory_init_smp, phys_to_virt, AddressSpace, MapArea
- Modify: `src/arch/riscv64/mod.rs` — call memory_init in bootstrap
- Modify: `src/arch/aarch64/mod.rs` — call memory_init in bootstrap
- Modify: `src/main.rs` — add phase3_smoke_test

### Step 7.1: Implement memory module top-level

- [ ] Update `src/memory/mod.rs`:

```rust
pub mod address;
#[cfg(not(test))]
pub mod frame;
#[cfg(not(test))]
pub mod heap;
#[cfg(not(test))]
pub mod page_table;

#[cfg(not(test))]
use address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use page_table::{PageFlags, PageTable};

/// Identity mapping: phys_to_virt is identity for now.
#[cfg(not(test))]
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize())
}

/// Identity mapping: virt_to_phys is identity for now.
#[cfg(not(test))]
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize())
}

/// Map a range of pages with identity mapping (VA == PA).
#[cfg(not(test))]
fn identity_map_range(
    pt: &mut PageTable,
    start: PhysAddr,
    end: PhysAddr,
    flags: PageFlags,
) -> crate::error::KResult<()> {
    let mut addr = start.align_down();
    let end = end.align_up();
    while addr.as_usize() < end.as_usize() {
        pt.map_page(
            VirtAddr::new(addr.as_usize()),
            addr,
            flags,
        )?;
        addr = addr + crate::config::PAGE_SIZE;
    }
    Ok(())
}

/// Primary memory initialization — called by BSP (bootstrap processor).
///
/// 1. Initialize heap allocator (static BSS region)
/// 2. Initialize frame allocator (physical memory from FDT)
/// 3. Create kernel page table with identity mapping
/// 4. Enable paging
#[cfg(not(test))]
pub fn memory_init() {
    use crate::per_cpu::BASIC_INFO;
    use crate::scope_guard::ScopeGuard;

    // Step 1: Heap — must come first so we can use Vec/Box
    unsafe { heap::heap_init() };

    // Step 2: Frame allocator
    let info = BASIC_INFO.get().expect("BASIC_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    // Allocatable region starts after kernel image (page-aligned)
    let alloc_start = kernel_end.align_up();
    // Reserve some space after kernel for safety (heap is in BSS, already inside kernel image)
    let alloc_size = mem_size - (alloc_start - mem_start);

    unsafe { frame::frame_init(alloc_start, alloc_size) };

    // Step 3: Create kernel page table
    let mut pt = PageTable::new().expect("failed to create kernel page table");

    // Identity-map the entire kernel region (text + rodata + data + bss + heap)
    identity_map_range(
        &mut pt,
        mem_start,
        mem_start + mem_size,
        PageFlags::kernel_rw(),
    )
    .expect("failed to identity-map kernel");

    log::info!(
        "MemoryInit: kernel mapped {}-{}",
        mem_start,
        mem_start + mem_size
    );

    // Step 4: Enable paging
    unsafe { page_table::activate_page_table(&pt) };
    log::info!("MemoryInit: paging enabled");

    // Leak the kernel page table — it must live forever
    core::mem::forget(pt);
}

/// Secondary core memory init — loads kernel page table into satp/TTBR.
/// TODO(P4): called by bootstrap_smp()
#[cfg(not(test))]
#[allow(dead_code)]
pub fn memory_init_smp() {
    // Secondary cores share the BSP's page table.
    // The satp/TTBR value was set by the BSP; on wakeup, firmware
    // or bootstrap_smp() will load the same value.
}

/// Map MMIO region, returns virtual address.
/// TODO(P6): DeviceInit calls this to map device registers.
#[cfg(not(test))]
#[allow(dead_code)]
pub fn map_mmio(_paddr: PhysAddr, _size: usize) -> crate::error::KResult<VirtAddr> {
    // Placeholder — identity mapping means VA == PA
    Ok(VirtAddr::new(_paddr.as_usize()))
}
```

### Step 7.2: Add phase3_smoke_test to main.rs

- [ ] In `src/main.rs`, add:

```rust
#[cfg(not(test))]
pub fn phase3_smoke_test() {
    use alloc::boxed::Box;

    let val = Box::new(42u64);
    log::info!("HeapTest: Box::new(42) = {}", *val);
    assert_eq!(*val, 42);

    log::info!("Phase 3 complete");
}
```

### Step 7.3: Update bootstrap to call memory_init + phase3

- [ ] In `src/arch/riscv64/mod.rs`:

```rust
pub fn bootstrap(argc: i32, argv: *const *const u8) -> ! {
    init::arch_init(argc, argv);
    crate::memory::memory_init();
    crate::phase3_smoke_test();
    loop {
        core::hint::spin_loop();
    }
}
```

- [ ] Same in `src/arch/aarch64/mod.rs`.

### Step 7.4: Remove phase2_smoke_test panic call

- [ ] The old `phase2_smoke_test()` ends with `panic!("test panic")`. Remove the panic call or convert it to a non-fatal check. Update the function to not halt the system. The phase2 test can remain but should not block phase3.

### Step 7.5: Build for target

- [ ] Run: `cargo build --target targets/riscv64-none.json`
- [ ] Expected: Compiles without errors

### Step 7.6: QEMU smoke test

- [ ] Run: `cargo xtask run --arch riscv64`
- [ ] Expected output includes:
  ```
  HeapInit: 4MB heap at 0x...
  FrameInit: ... MB available from ...
  MemoryInit: kernel mapped ...
  MemoryInit: paging enabled
  HeapTest: Box::new(42) = 42
  Phase 3 complete
  ```

### Step 7.7: Commit

- [ ] `git commit -m "feat(P3): implement memory_init with identity-mapped paging and heap"`

---

## Task 8: Unit Tests for Page Table Operations

**Files:**
- Modify: `src/memory/page_table.rs` — add `#[cfg(test)] mod tests`

### Step 8.1: Add page table unit tests

- [ ] Note: Page table unit tests on host are limited since we can't actually allocate frames via the real allocator. Add tests for PTE encoding/decoding:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pte_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let flags = PageFlags::kernel_rw();
        let pte = PageTableEntry::new(pa, flags);
        assert!(pte.is_valid());
        assert_eq!(pte.paddr(), pa);
        assert!(pte.flags().contains(PageFlags::READ));
        assert!(pte.flags().contains(PageFlags::WRITE));
    }

    #[test]
    fn pte_empty_is_invalid() {
        let pte = PageTableEntry::empty();
        assert!(!pte.is_valid());
    }

    #[test]
    fn page_flags_presets() {
        let rw = PageFlags::kernel_rw();
        assert!(rw.contains(PageFlags::VALID | PageFlags::READ | PageFlags::WRITE));
        assert!(!rw.contains(PageFlags::EXECUTE));

        let rx = PageFlags::kernel_rx();
        assert!(rx.contains(PageFlags::READ | PageFlags::EXECUTE));
        assert!(!rx.contains(PageFlags::WRITE));
    }
}
```

### Step 8.2: Run tests, commit

- [ ] Run: `cargo test -- page_table`
- [ ] Expected: All PTE tests pass
- [ ] `git commit -m "test(P3): add page table entry unit tests"`

---

## Exit Criteria Checklist

After all tasks complete, verify:

- [ ] `PhysAddr`/`VirtAddr` unit tests pass (`cargo test -- address`)
- [ ] `Box::new()` works (verified in phase3_smoke_test on QEMU)
- [ ] Paging enabled without page fault (QEMU log shows "paging enabled" followed by continued execution)
- [ ] `MapPage`/`UnmapPage` PTE encoding tests pass (`cargo test -- page_table`)
- [ ] `BasicInfo` fields are `PhysAddr` type (verified by compilation)
- [ ] `ScopeGuard` tests pass (`cargo test -- scope_guard`)
