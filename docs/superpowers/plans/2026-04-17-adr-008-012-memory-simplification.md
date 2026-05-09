<!-- Copyright The SimpleKernel Contributors -->

# ADR-008~012 内存子系统简化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute ADR-008 through ADR-012, simplifying the memory subsystem: delete FrameState typestate, remove CLAIMED bit, replace BTreeMap with Vec in PageTable, introduce FlagsConflict error, and make hot-path PTE updates lock-free.

**Architecture:** Five sequential ADRs that progressively simplify the memory subsystem. Each ADR builds on the previous: 008 simplifies frame_allocator, 009 removes CLAIMED from PTE, 010 simplifies PageTable internals, 011 refines create_pte semantics and deletes MMIO tracking, 012 restructures PageTable locking for SMP scalability.

**Tech Stack:** Rust nightly (`no_std`), `AtomicU64` (Acquire/Release), `bitflags`, `buddy_system_allocator`, custom `SpinLock`

---

## File Structure

### Files to Delete
- `crates/frame_allocator/src/state.rs` (ADR-008)
- `crates/frame_allocator/src/transitions.rs` (ADR-008)
- `tests/paging-test/src/double_claim_panic.rs` (ADR-009)

### Files to Create
- `crates/frame_allocator/src/frames.rs` — New `AllocatedFrames` struct definition (ADR-008)

### Files to Modify
- `crates/frame_allocator/src/lib.rs` — Remove feature gate, update modules/exports (ADR-008)
- `crates/frame_allocator/src/alloc.rs` — `alloc_from_backend` returns `AllocatedFrames` directly (ADR-008)
- `crates/paging/src/lib.rs` — Add zero-clearing to `alloc_node_frame`; later remove outer SpinLock (ADR-008, 012)
- `crates/page_table_entry/src/lib.rs` — Remove `is_claimed`/`with_claimed` from trait (ADR-009)
- `crates/page_table_entry/src/riscv64.rs` — Remove `CLAIMED` constant and methods (ADR-009)
- `crates/page_table_entry/src/aarch64.rs` — Remove `CLAIMED` constant and methods (ADR-009)
- `crates/paging/src/mapping.rs` — Remove `claim_pages`, simplify `OwnedPages` (ADR-009)
- `crates/paging/src/table.rs` — Remove ref_count, BTreeMap→Vec, rename+refine semantics, lock restructure (ADR-010, 011, 012)
- `crates/paging/src/error.rs` — Add `FlagsConflict` variant (ADR-011)
- `crates/paging/src/mmio.rs` — Remove `.lock()` calls (ADR-012)
- `crates/memory/src/lib.rs` — Delete MMIO_REGIONS + check_mmio_overlap, add RAM validation (ADR-011)
- `crates/memory/src/error.rs` — Remove MmioIdentical/MmioOverlap, add FlagsConflict mapping (ADR-011)
- `crates/memory/src/init.rs` — Remove `.lock()` calls (ADR-012)
- `src/boot.rs` — Remove `.lock()` on page table access (ADR-012)
- `tests/paging-test/Cargo.toml` — Remove double-claim-panic bin entry (ADR-009)
- `tests/paging-test/src/mapping.rs` — Remove CLAIMED assertions (ADR-009)
- `tests/paging-test/src/table.rs` — Adapt `test_set_page_flags_updates_flags_same_pa`, remove `mut` (ADR-011, 012)
- `tests/paging-test/src/conflict_panic.rs` — No change needed (should pass after ADR-011)
- `tests/frame-test/src/alloc.rs` — No change needed (uses `AllocatedFrames` public API only)

---

### Task 1: ADR-008 — Delete FrameState typestate, create plain AllocatedFrames

**Files:**
- Create: `crates/frame_allocator/src/frames.rs`
- Delete: `crates/frame_allocator/src/state.rs`
- Delete: `crates/frame_allocator/src/transitions.rs`
- Modify: `crates/frame_allocator/src/lib.rs`
- Modify: `crates/frame_allocator/src/alloc.rs`
- Modify: `crates/paging/src/lib.rs:73-79`

- [ ] **Step 1: Create `frames.rs` with plain `AllocatedFrames` struct**

```rust
// crates/frame_allocator/src/frames.rs
//! 已分配帧类型——RAII 物理帧所有权。

use memory_types::PhysAddr;

use crate::FrameSpan;
use crate::alloc::dealloc_to_backend;

/// 已分配的连续物理帧——持有所有权，Drop 时归还分配器。
///
/// 不可 Clone、不可 Copy。帧内容**未初始化**——调用方按需初始化。
///
/// SAS 全量映射下帧始终可通过 identity mapping 访问（`PA.to_virt()`）。
pub struct AllocatedFrames {
    pub(crate) range: FrameSpan,
}

impl core::fmt::Debug for AllocatedFrames {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AllocatedFrames({}-{})", self.range.start(), self.range.end())
    }
}

impl AllocatedFrames {
    /// 分配 `count` 个连续 4K 物理帧。**内容未初始化**，调用方负责初始化。
    ///
    /// 内部路径：buddy allocator → `AllocatedFrames`。
    /// buddy 内部将帧数向上取整到 2 的幂次，
    /// 实际分配的帧数可能多于请求数，但 `FrameSpan` 仅跟踪请求的帧。
    ///
    /// # Errors
    ///
    /// 分配器未初始化返回 `AllocationFailed`，帧耗尽返回 `OutOfMemory`。
    pub fn alloc(count: usize) -> Result<Self, crate::FrameAllocError> {
        crate::alloc::alloc_from_backend(count)
    }

    /// 分配一个 4K 物理帧。**内容未初始化**。
    pub fn alloc_one() -> Result<Self, crate::FrameAllocError> {
        Self::alloc(1)
    }

    /// 范围内 4K 帧的数量。
    #[inline]
    pub fn count(&self) -> usize {
        self.range.size()
    }

    /// 起始物理地址。
    #[inline]
    pub fn start_paddr(&self) -> PhysAddr {
        self.range.start().start_addr()
    }

    /// 从 FrameSpan 构造（crate 内部使用）。
    #[inline]
    pub(crate) fn from_range(range: FrameSpan) -> Self {
        Self { range }
    }
}

impl Drop for AllocatedFrames {
    fn drop(&mut self) {
        dealloc_to_backend(self.range);
    }
}
```

- [ ] **Step 2: Update `alloc.rs` — `alloc_from_backend` returns `AllocatedFrames` directly, remove `FreeFrames` import**

Change `alloc_from_backend` to return `AllocatedFrames` instead of `FreeFrames`. Remove import of `FreeFrames`. The function body changes from creating a `FreeFrames` to creating an `AllocatedFrames` directly.

In `crates/frame_allocator/src/alloc.rs`:
- Replace `use crate::state::{AllocatedFrames, FreeFrames};` with `use crate::frames::AllocatedFrames;`
- Change return type of `alloc_from_backend` from `Result<FreeFrames, FrameAllocError>` to `Result<AllocatedFrames, FrameAllocError>`
- Change `Ok(FreeFrames::from_range(...))` to `Ok(AllocatedFrames::from_range(...))`
- In `init()`, change `use crate::state::AllocatedFrames;` reference — but init already constructs `AllocatedFrames::from_range` directly, which still works.

```rust
// alloc_from_backend signature and body (line 140-156):
pub(crate) fn alloc_from_backend(count: usize) -> Result<AllocatedFrames, FrameAllocError> {
    if count == 0 {
        return Err(FrameAllocError::AllocationFailed);
    }
    let mut alloc = FRAME_ALLOCATOR.lock();
    if !alloc.initialized {
        return Err(FrameAllocError::AllocationFailed);
    }
    let frame_num = alloc
        .allocator
        .alloc(count)
        .ok_or(FrameAllocError::OutOfMemory)?;
    let start = Frame::new(frame_num);
    Ok(AllocatedFrames::from_range(FrameSpan::new(start, start + count)))
}
```

- [ ] **Step 3: Update `lib.rs` — remove feature gate, delete old modules, add new module**

In `crates/frame_allocator/src/lib.rs`:
- Remove `#![feature(adt_const_params)]`
- Remove `mod state;` and `mod transitions;`
- Add `mod frames;`
- Change `pub use state::{AllocatedFrames, FrameState, Frames};` to `pub use frames::AllocatedFrames;`
- Update module doc comment to remove "2-State Typestate" section

```rust
//! 物理帧分配器——RAII 帧所有权 + buddy 后端。
//!
//! # 在内存子系统中的定位
//!
//! 本 crate 是内存子系统的**资源层**——管理物理帧的分配和回收，
//! 不感知页表的存在。帧的权限管理由上层 `paging::OwnedPages` 负责。
//!
//! ```text
//! paging::OwnedPages (权限管理)
//!    │
//!    ▼ 持有 AllocatedFrames 字段
//! frame_allocator (本 crate: 帧所有权追踪)
//!    │
//!    ▼ alloc/dealloc
//! buddy_system_allocator (后端)
//! ```
//!
//! # 分配器接口
//!
//! ```text
//! buddy pool ──alloc()──▶ AllocatedFrames ──Drop──▶ buddy pool
//! ```
//!
//! `AllocatedFrames` 是唯一的公开类型——持有帧所有权，Drop 时自动归还。
//! 帧内容**未初始化**，调用方按需初始化（页表节点需清零，栈/DMA 各有策略）。
//!
//! 帧的所有权通过 Rust move 语义在编译期追踪，无需引用计数或 PTE 标记位。
//!
//! # 典型用法
//!
//! ```rust,ignore
//! let frames = AllocatedFrames::alloc(4)?;  // 4 连续帧，内容未初始化
//! // frames 通过 identity mapping 可直接访问
//! let ptr: *mut u8 = frames.start_paddr().to_virt().as_mut_ptr();
//! // Drop 时自动归还 buddy
//! ```

#![no_std]

mod alloc;
mod error;
mod frames;

pub use alloc::init;
pub use error::FrameAllocError;
pub use frames::AllocatedFrames;

/// 物理帧范围——`frame_allocator` 内部使用的便利别名。
pub(crate) type FrameSpan = memory_types::Span<memory_types::Frame>;
```

- [ ] **Step 4: Delete `state.rs` and `transitions.rs`**

Delete files:
- `crates/frame_allocator/src/state.rs`
- `crates/frame_allocator/src/transitions.rs`

- [ ] **Step 5: Add zero-clearing to `alloc_node_frame` in paging**

In `crates/paging/src/lib.rs:73-79`, add `write_bytes` after allocation:

```rust
/// 分配一个零初始化的页表节点帧。
///
/// 页表节点要求全零初态（无效 PTE = 0），由本函数负责清零。
/// 分配器本身不清零（ADR-008: 机制与策略分离）。
fn alloc_node_frame() -> Result<frame_allocator::AllocatedFrames, error::PagingError> {
    let frame = frame_allocator::AllocatedFrames::alloc_one().map_err(|e| {
        log::warn!("页表节点帧分配失败: {:?}", e);
        error::PagingError::AllocationFailed
    })?;
    // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚分配，无其他引用
    unsafe {
        core::ptr::write_bytes(
            frame.start_paddr().to_virt().as_mut_ptr::<u8>(),
            0,
            config::PAGE_SIZE,
        );
    }
    Ok(frame)
}
```

- [ ] **Step 6: Update DMA HAL comment**

In `src/device/hal.rs:36-37`, the comment says "AllocatedFrames::alloc 已保证内容清零" — this is no longer true. Update:

```rust
    /// 分配 DMA 缓冲区——调用帧分配器获取物理连续页。
    ///
    /// 帧分配器不清零（ADR-008），DMA 缓冲区需要清零以防信息泄漏。
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (u64, NonNull<u8>) {
        let frames = AllocatedFrames::alloc(pages).expect("DMA 帧分配失败");
        let paddr = frames.start_paddr();
        let vaddr = paddr.to_virt();

        // 清零 DMA 缓冲区——防止旧数据泄漏到设备
        // SAFETY: identity mapping 下 VA == PA，帧刚分配无其他引用
        unsafe {
            core::ptr::write_bytes(vaddr.as_mut_ptr::<u8>(), 0, pages * config::PAGE_SIZE);
        }

        // SAFETY: to_virt 保证返回有效的虚拟地址（identity mapping 下 VA == PA）
        let ptr = NonNull::new(vaddr.as_mut_ptr::<u8>()).expect("DMA vaddr 为空");

        // 将帧存入追踪表，阻止 Drop 回收
        DMA_TRACKER.lock().insert(paddr.as_usize() as u64, frames);

        (paddr.as_usize() as u64, ptr)
    }
```

- [ ] **Step 7: Build and test**

Run: `cargo fmt && cargo clippy -- -D warnings`
Run: `cargo test -p frame_allocator` (if any host tests exist)
Run: `cargo xtask build --arch riscv64`
Expected: Compiles without errors.

- [ ] **Step 8: Commit**

```bash
git add -A crates/frame_allocator/ crates/paging/src/lib.rs src/device/hal.rs
git commit --signoff -m "refactor(frame_allocator): 删除 FrameState typestate + 分配器不清零 (ADR-008)

- 删除 state.rs、transitions.rs，移除 #![feature(adt_const_params)]
- 新建 frames.rs: 纯 struct AllocatedFrames + RAII Drop
- alloc_from_backend 直接返回 AllocatedFrames（无 FreeFrames 中间状态）
- 分配器不再强制清零——调用方按需初始化:
  - alloc_node_frame(): 页表节点清零
  - dma_alloc(): DMA 缓冲区清零
- 与 OwnedPages（RAII，无 typestate）概念对称"
```

---

### Task 2: ADR-009 — Delete CLAIMED software bit, keep page poison

**Files:**
- Delete: `tests/paging-test/src/double_claim_panic.rs`
- Modify: `crates/page_table_entry/src/lib.rs:95-98`
- Modify: `crates/page_table_entry/src/riscv64.rs:49-51,175-186`
- Modify: `crates/page_table_entry/src/aarch64.rs:69-71,230-242`
- Modify: `crates/paging/src/mapping.rs` (delete `claim_pages`, simplify `OwnedPages`)
- Modify: `tests/paging-test/Cargo.toml:21-24`
- Modify: `tests/paging-test/src/mapping.rs` (remove CLAIMED assertions)

- [ ] **Step 1: Remove `is_claimed`/`with_claimed` from `PteFlagsOps` trait**

In `crates/page_table_entry/src/lib.rs`, delete lines 95-98:

```rust
    // DELETE these 4 lines:
    // fn is_claimed(self) -> bool;
    // fn with_claimed(self, claimed: bool) -> Self;
```

- [ ] **Step 2: Remove CLAIMED from RISC-V PteFlags**

In `crates/page_table_entry/src/riscv64.rs`:
- Delete `const CLAIMED = 1 << 8;` (line 51)
- Delete `is_claimed` impl (lines 175-177)
- Delete `with_claimed` impl (lines 179-186)

- [ ] **Step 3: Remove CLAIMED from AArch64 PteFlags**

In `crates/page_table_entry/src/aarch64.rs`:
- Delete `const CLAIMED = 1 << 55;` (lines 69-71)
- Delete `is_claimed` impl (lines 231-233)
- Delete `with_claimed` impl (lines 235-242)

- [ ] **Step 4: Rewrite `mapping.rs` — delete `claim_pages`, simplify `OwnedPages`**

Replace the entire `crates/paging/src/mapping.rs` with:

```rust
//! 仿射类型所有权——move-only 的物理帧所有权 + 权限管理。
//!
//! [`OwnedPages`] 持有物理帧的独占所有权，VA 通过 identity mapping
//! 从 PA 推导（VA == PA）。
//!
//! SAS 架构下所有物理内存始终有背景 identity mapping（kernel_rw），
//! `OwnedPages` 管理的是**所有权和权限覆盖层**。
//!
//! Rust 所有权系统在编译期保证唯一所有权（`AllocatedFrames` 不可 Clone/Copy），
//! Drop 时写入 poison 填充用于 use-after-free 调试。

use config::PAGE_SIZE;
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// 仿射类型帧所有权——持有物理帧的独占所有权和当前权限。
///
/// 不可 Clone、不可 Copy。Drop 时恢复 PTE 为默认权限（kernel_rw）、
/// 写入 poison 填充、并回收帧。
///
/// SAS 全量映射下，所有物理内存始终有 identity mapping（背景层）。
/// `OwnedPages` 不创建/删除 PTE，而是管理权限覆盖：
/// - `new`：更新权限
/// - `set_flags`：修改权限
/// - `drop`：恢复 kernel_rw → poison → 归还帧
pub struct OwnedPages {
    frames: AllocatedFrames,
    /// 用户可见的权限标志。
    flags: PteFlags,
}

impl OwnedPages {
    /// 声明物理帧所有权——更新 PTE 权限。
    ///
    /// SAS 全量映射下，PTE 已由 boot 背景映射建立（kernel_rw）。
    /// 此方法接管帧所有权并更新 PTE flags。
    ///
    /// # Panics
    ///
    /// - 页数为 0
    /// - PTE 不存在（背景映射未建立）
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "OwnedPages::new: 页数不能为 0");

        let va_start = frames.start_paddr().to_virt();
        batch_update_flags(va_start, page_count, flags);

        Self { frames, flags }
    }

    /// 返回起始虚拟地址（从 PA 推导）。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.frames.start_paddr().to_virt()
    }

    /// 返回区域总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.frames.count() * PAGE_SIZE
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frames.count()
    }

    /// 返回用户可见的权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 修改权限——遍历 PTE 更新标志位，刷新 TLB。
    pub fn set_flags(&mut self, new_flags: PteFlags) {
        batch_update_flags(self.vaddr(), self.page_count(), new_flags);
        self.flags = new_flags;
    }
}

/// 批量更新 PTE 权限并刷新 TLB。
///
/// `set_flags` / `Drop` / `new` 共用此逻辑。
fn batch_update_flags(va_start: VirtAddr, page_count: usize, flags: PteFlags) {
    let pt = crate::kernel_page_table();
    let mut guard = pt.lock();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        guard
            .update_flags(va, flags)
            .expect("batch_update_flags: update_flags 失败");
    }
    drop(guard);
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}

impl Drop for OwnedPages {
    fn drop(&mut self) {
        let va_start = self.vaddr();
        let page_count = self.page_count();

        // 恢复 kernel_rw + flush TLB
        batch_update_flags(va_start, page_count, PteFlags::kernel_rw());

        // 写 poison——此时 PTE 已恢复 kernel_rw 且 TLB 已刷新
        // SAFETY: va_start identity-mapped，帧仍由 self.frames 持有（buddy 尚未回收）
        unsafe {
            core::ptr::write_bytes(
                va_start.as_mut_ptr::<u8>(),
                config::FREED_PAGE_POISON,
                page_count * PAGE_SIZE,
            );
        }

        // AllocatedFrames 自然 drop → buddy 回收
    }
}

impl core::fmt::Debug for OwnedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "OwnedPages({}, {} pages, {:?})",
            self.vaddr(),
            self.page_count(),
            self.flags,
        )
    }
}
```

- [ ] **Step 5: Delete `double_claim_panic.rs` test and remove its Cargo.toml entry**

Delete file: `tests/paging-test/src/double_claim_panic.rs`

In `tests/paging-test/Cargo.toml`, delete the `[[bin]]` block for `double-claim-panic` (lines 21-24):

```toml
# DELETE:
# [[bin]]
# name = "double-claim-panic"
# path = "src/double_claim_panic.rs"
# test = false
```

- [ ] **Step 6: Update mapping tests — remove CLAIMED assertions**

In `tests/paging-test/src/mapping.rs`:

`test_new_basic` (line 48): delete `assert!(flags.is_claimed(), ...)` line.

`test_new_changes_flags` (line 90): delete `assert!(flags.is_claimed(), ...)` line.

`test_drop_restores_default_flags` (lines 106, 113): delete both `assert!(...is_claimed()...)` lines.

`test_set_flags_changes_flags` (line 133): delete `assert!(flags.is_claimed(), ...)` line.

- [ ] **Step 7: Build and test**

Run: `cargo fmt && cargo clippy -- -D warnings`
Run: `cargo xtask build --arch riscv64`
Expected: Compiles without errors.

- [ ] **Step 8: Commit**

```bash
git add -A crates/page_table_entry/ crates/paging/src/mapping.rs tests/paging-test/
git commit --signoff -m "refactor(paging): 删除 CLAIMED 软件位，保留 page poison (ADR-009)

- PteFlagsOps: 删除 is_claimed/with_claimed 方法
- PteFlags: 删除 CLAIMED 常量（RISC-V RSW[0] / AArch64 bit 55）
- mapping.rs: 删除 claim_pages 函数，OwnedPages::new 直接走 batch_update_flags
- 删除 double_claim_panic 测试（CLAIMED 机制不再存在）
- 保留 OwnedPages::Drop 的 poison write_bytes（被动调试信号）
- 释放两架构各一个 PTE 软件位供未来 COW/swap 使用"
```

---

### Task 3: ADR-010 — PageTable BTreeMap → Vec, remove ref_count

**Files:**
- Modify: `crates/paging/src/table.rs`

- [ ] **Step 1: Refactor PageTable internals**

In `crates/paging/src/table.rs`:

1. Replace `use alloc::collections::BTreeMap;` with `use alloc::vec::Vec;`
2. Delete `struct NodeEntry` (lines 52-61)
3. Remove `root_ref_count` field from `PageTable`
4. Change `nodes` type from `BTreeMap<PhysAddr, NodeEntry>` to `Vec<AllocatedFrames>`
5. Delete `ref_count_mut` method (lines 96-107)
6. Delete `inc_ref` method (lines 109-113)
7. Update `PageTable::create()`: remove `root_ref_count: 0`, change `nodes: BTreeMap::new()` to `nodes: Vec::new()`
8. Update `walk_create`: replace `self.nodes.insert(frame_paddr, NodeEntry { ... })` with `self.nodes.push(frame)`; remove `self.inc_ref(paddr)` call
9. Update `set_page_flags`: remove `self.inc_ref(frame_paddr)` call (line 198)

The updated `table.rs` (key changes only):

```rust
// Replace BTreeMap import:
use alloc::vec::Vec;

// Delete struct NodeEntry entirely.

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
pub struct PageTable {
    /// 持有根帧所有权——通过 `.start_paddr()` 获取物理地址。
    root: AllocatedFrames,
    /// 中间页表节点——仅持有所有权，drop 时自动释放。
    nodes: Vec<AllocatedFrames>,
}

impl PageTable {
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            nodes: Vec::new(),
        })
    }

    // Delete ref_count_mut and inc_ref entirely.

    fn walk_create(&mut self, va: VirtAddr) -> Result<(PhysAddr, usize), PagingError> {
        let mut paddr = self.root.start_paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let mut table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = crate::alloc_node_frame()?;
                let frame_paddr = frame.start_paddr();
                self.nodes.push(frame);
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                panic!(
                    "walk_create: VA {} 在 level {} 遇到非预期的大页叶 PTE（页表损坏）",
                    va, level
                );
            } else {
                paddr = pte.paddr();
            }
        }

        let idx = vpn_index(va, 0);
        Ok((paddr, idx))
    }

    // set_page_flags: remove the `self.inc_ref(frame_paddr);` line (was line 198)
}
```

- [ ] **Step 2: Update module doc comment**

In `crates/paging/src/table.rs` line 1, update:
```rust
//! 多级页表——walk / set_page_flags 逻辑。
```

- [ ] **Step 3: Build and test**

Run: `cargo fmt && cargo clippy -- -D warnings`
Run: `cargo xtask build --arch riscv64`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/paging/src/table.rs
git commit --signoff -m "refactor(paging): BTreeMap → Vec + 删除 ref_count 死代码 (ADR-010)

- 删除 NodeEntry struct、root_ref_count、ref_count_mut、inc_ref
- nodes 字段: BTreeMap<PhysAddr, NodeEntry> → Vec<AllocatedFrames>
- walk_create: insert → push (O(1) 摊还)
- 净减 ~30 行，数据结构与实际职责（持有所有权）对齐"
```

---

### Task 4: ADR-011 — FlagsConflict + rename set_page_flags → create_pte + delete MMIO_REGIONS

**Files:**
- Modify: `crates/paging/src/error.rs`
- Modify: `crates/paging/src/table.rs` (rename + FlagsConflict)
- Modify: `crates/memory/src/lib.rs` (delete MMIO_REGIONS, add RAM validation)
- Modify: `crates/memory/src/error.rs` (remove MmioIdentical/MmioOverlap)
- Modify: `tests/paging-test/src/table.rs` (adapt test)

- [ ] **Step 1: Add `FlagsConflict` to `PagingError`**

Replace `crates/paging/src/error.rs`:

```rust
//! 分页子系统错误定义。

use core::fmt;

/// 分页操作错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 页表节点帧分配失败
    AllocationFailed,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 已有 PTE 的 flags 与请求冲突（同 PA + 不同 flags）
    FlagsConflict,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::FlagsConflict => write!(f, "PTE flags conflict (same PA, different flags)"),
        }
    }
}

impl core::error::Error for PagingError {}
```

- [ ] **Step 2: Rename `set_page_flags` → `create_pte`, add FlagsConflict logic; rename `update_flags` → `update_pte`**

In `crates/paging/src/table.rs`:

Rename `set_page_flags` → `create_pte` and change "同 PA 不同 flags" from silent update to `Err(FlagsConflict)`:

```rust
    /// 创建单个虚拟页（4KB）的 PTE。
    ///
    /// SAS 全量映射下 PTE 始终存在。此方法的语义：
    /// - 若该 VA 无 PTE → 创建 Level 0 叶 PTE，返回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同且 flags 相同 → 幂等，返回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同但 flags 不同 → Err(FlagsConflict)；
    ///   显式修改 flags 请使用 [`update_pte`](Self::update_pte)
    /// - 若该 VA 已有 PTE 但 PA 不同 → panic（内核 bug）
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新。**
    ///
    /// # Panics
    ///
    /// - walk 路径上遇到非预期的大页叶 PTE（页表损坏）
    /// - VA 已映射到不同的 PA（内核 bug）
    pub fn create_pte(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PagingError> {
        let leaf_flags = flags.for_leaf_at_level(0);
        let (frame_paddr, idx) = self.walk_create(va)?;
        // SAFETY: frame_paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(frame_paddr) };
        let current = table.read(idx);
        if current.is_valid() {
            if current.paddr() != pa {
                panic!(
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    current.paddr(),
                    pa
                );
            }
            if current.flags() != leaf_flags {
                return Err(PagingError::FlagsConflict);
            }
            return Ok(());
        }
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        Ok(())
    }

    /// 修改已映射页的权限标志位，保留物理地址不变。
    ///
    /// 单次页表遍历完成查找和更新，避免双重 walk 开销。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    pub fn update_pte(
        &mut self,
        va: VirtAddr,
        new_flags: PteFlags,
    ) -> Result<PteFlags, PagingError> {
        let (pte, paddr, idx, leaf_level) =
            self.walk_to_leaf(va).ok_or(PagingError::PageNotMapped)?;

        let old_flags = pte.flags();
        let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
        // SAFETY: paddr 指向叶 PTE 所在的帧
        let mut table = unsafe { Table::from_paddr(paddr) };
        table.write(idx, PageTableEntry::new(pte.paddr(), leaf_flags));

        Ok(old_flags)
    }
```

Update `identity_map_range` to handle `FlagsConflict`:

```rust
    pub fn identity_map_range(&mut self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
        );

        while addr.as_usize() < end_aligned.as_usize() {
            let va = VirtAddr::new(addr.as_usize());
            match self.create_pte(va, addr, flags) {
                Ok(()) => {}
                Err(PagingError::FlagsConflict) => panic!(
                    "identity_map_range: VA {} flags 冲突——已有 PTE 的权限与请求不同",
                    va
                ),
                Err(e) => panic!("identity_map_range: 设置 {va} 权限失败: {e}"),
            }
            addr += config::PAGE_SIZE;
        }
    }
```

- [ ] **Step 3: Update `mapping.rs` to use `update_pte`**

In `crates/paging/src/mapping.rs`, the `batch_update_flags` function calls `guard.update_flags(...)`. Rename to `guard.update_pte(...)`:

```rust
fn batch_update_flags(va_start: VirtAddr, page_count: usize, flags: PteFlags) {
    let pt = crate::kernel_page_table();
    let mut guard = pt.lock();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        guard
            .update_pte(va, flags)
            .expect("batch_update_flags: update_pte 失败");
    }
    drop(guard);
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}
```

- [ ] **Step 4: Delete MMIO_REGIONS from memory crate, add RAM validation**

Replace `crates/memory/src/lib.rs`:

```rust
//! 内核内存管理门面——统一编排子系统初始化、提供 MMIO 映射接口。
//!
//! # 架构总览
//!
//! 内存子系统由多个 crate 分三层协同工作：
//!
//! ```text
//! ┌──────────���──────────────────────────────────────────┐
//! │  策略层 — memory (本 crate)                          │
//! │  init() · init_smp() · map_mmio()                    │
//! ├─────────────────────────────────────────────────────┤
//! │  机制层 — paging                                     │
//! │  PageTable · OwnedPages · MmioRegion                 │
//! ├───────────────┬──────────────────┬──────────────────┤
//! │ frame_allocator│ page_table_entry  │ tlb             │
//! │ 物理帧分配     │ PTE 编解码        │ TLB 刷新        │
//! │ AllocatedFrames│ PteFlagsOps/PteOps│ TlbFlushGuard   │
//! ├───────────────┴──────────────────┴──────────────────┤
//! │  memory_types — PhysAddr · VirtAddr · Frame · Span   │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # SAS 全量映射模型
//!
//! SAS 架构下所有物理内存在 boot 时 identity-map 为 `kernel_rw`（背景层），
//! 运行时只调整权限（覆盖层），永远不创建或删除 PTE。
//!
//! # 典型使用方式
//!
//! ```rust,ignore
//! use memory::frame::AllocatedFrames;
//! use paging::{OwnedPages, PteFlags, PteFlagsOps};
//!
//! // 分配帧 + 设置只读权限
//! let frames = AllocatedFrames::alloc(4)?;
//! let mapping = OwnedPages::new(frames, PteFlags::kernel_ro());
//! // ... 使用 mapping.vaddr() 访问内存 ...
//! drop(mapping); // 恢复 kernel_rw + 归还帧
//!
//! // MMIO 映射
//! let va = memory::map_mmio(PhysAddr::new(0x1000_0000), 0x1000)?;
//! ```
//!
//! # 初始化顺序
//!
//! 1. `heap::init()` — 启用堆分配（buddy 内部需要 BTreeSet）
//! 2. `frame_allocator::init()` — 空闲帧入 buddy，内核段帧预留
//! 3. `PageTable::create()` + `identity_map_range()` — 背景层
//! 4. `OwnedPages::new()` × 3 + `mem::forget()` — 覆盖层（永久持有）
//!
//! 详见 `docs/design/memory-subsystem-v2.md`。

#![no_std]

extern crate alloc;

/// 错误类型。
pub mod error;
/// 物理帧分配器（re-export `frame_allocator` crate）。
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
pub use heap_crate as heap;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
pub mod init;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

pub use globals::{MEMORY_INFO, MemoryInfo};

pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map，返回 `paddr` 对应的虚拟地址。
///
/// 内部按页对齐建立映射，但返回值精确对应调用方请求的 `paddr`（类似 Linux `ioremap`）。
/// MMIO 映射永久存在（MmioRegion 不 unmap）。
///
/// MMIO 重叠检测由 PageTable 的 PTE 承担（单一真相源）：
/// - 同 PA + 同 flags → 幂等，`create_pte` 返回 Ok
/// - 同 PA + 不同 flags → `create_pte` 返回 `FlagsConflict`，`identity_map_range` panic
///
/// # Panics
///
/// `paddr` 落在 RAM 范围内时 panic——拒绝将 RAM 映射为 Device 内存。
///
/// # Errors
///
/// 页表映射失败时返回错误。
pub fn map_mmio(
    paddr: memory_types::PhysAddr,
    size: usize,
) -> Result<memory_types::VirtAddr, error::MemoryError> {
    // paddr RAM 校验——拒绝将 RAM 重映射为 Device 内存
    let info = MEMORY_INFO
        .get()
        .expect("map_mmio: MEMORY_INFO not initialized");
    let ram_start = info.physical_memory_addr.as_usize();
    let ram_end = ram_start + info.physical_memory_size;
    assert!(
        paddr.as_usize() + size <= ram_start || paddr.as_usize() >= ram_end,
        "map_mmio: paddr {} + {:#x} 落在 RAM 范围 [{:#x}, {:#x})——拒绝映射为 Device 内存",
        paddr,
        size,
        ram_start,
        ram_end
    );

    paging::mmio::MmioRegion::map(paddr, size)?;

    // 返回 paddr 对应的精确虚拟地址（identity mapping: VA == PA）
    Ok(paddr.to_virt())
}
```

- [ ] **Step 5: Update `MemoryError` — remove MMIO variants, add FlagsConflict mapping**

Replace `crates/memory/src/error.rs`:

```rust
//! 内存子系统错误定义。

use core::fmt;

/// 内存子系统错误类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// 帧分配器未初始化时尝试分配
    AllocationFailed,
    /// 物理帧耗尽
    OutOfMemory,
    /// 页表映射失败（如重复映射或 flags 冲突）
    MapFailed,
    /// 目标虚拟页未映射
    PageNotMapped,
    /// 全局内核页表未初始化
    InvalidPageTable,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for MemoryError {}

impl From<frame_allocator::FrameAllocError> for MemoryError {
    fn from(e: frame_allocator::FrameAllocError) -> Self {
        match e {
            frame_allocator::FrameAllocError::AllocationFailed => Self::AllocationFailed,
            frame_allocator::FrameAllocError::OutOfMemory => Self::OutOfMemory,
        }
    }
}

impl From<paging::error::PagingError> for MemoryError {
    fn from(e: paging::error::PagingError) -> Self {
        use paging::error::PagingError;
        match e {
            PagingError::AllocationFailed => Self::AllocationFailed,
            PagingError::PageNotMapped => Self::PageNotMapped,
            PagingError::FlagsConflict => Self::MapFailed,
        }
    }
}
```

- [ ] **Step 6: Update `table.rs` tests — adapt for rename + FlagsConflict**

In `tests/paging-test/src/table.rs`:

1. All calls to `pt.set_page_flags(...)` → `pt.create_pte(...)`
2. All calls to `pt.update_flags(...)` → `pt.update_pte(...)`
3. `test_set_page_flags_updates_flags_same_pa` now expects `Err(FlagsConflict)`:

```rust
/// 同 VA + 同 PA + 不同 flags → FlagsConflict 错误（显式修改请用 update_pte）。
fn test_create_pte_flags_conflict() {
    let mut pt = PageTable::create().expect("创建测试页表失败");
    let va = VirtAddr::new(0x1000);
    let pa = PhysAddr::new(0x8020_0000);

    pt.create_pte(va, pa, PteFlags::kernel_rw())
        .expect("首次 create 应成功");
    let err = pt
        .create_pte(va, pa, PteFlags::kernel_ro())
        .expect_err("不同 flags 应返回 FlagsConflict");
    assert_eq!(err, PagingError::FlagsConflict);
}
```

Also rename all test function names: `test_set_page_flags_*` → `test_create_pte_*`, `test_update_flags_*` → `test_update_pte_*`.

Update the runner and log messages accordingly.

- [ ] **Step 7: Update `mmio.rs` to use `create_pte` naming if applicable**

In `crates/paging/src/mmio.rs:38`, `identity_map_range` is called (which calls `create_pte` internally), no direct rename needed here. The code is fine as-is.

- [ ] **Step 8: Build and test**

Run: `cargo fmt && cargo clippy -- -D warnings`
Run: `cargo xtask build --arch riscv64`
Expected: Compiles without errors.

- [ ] **Step 9: Commit**

```bash
git add crates/paging/ crates/memory/src/lib.rs crates/memory/src/error.rs tests/paging-test/
git commit --signoff -m "refactor(paging): create_pte FlagsConflict + 删除 MMIO_REGIONS (ADR-011)

- PagingError 新增 FlagsConflict 变体
- set_page_flags → create_pte: 同 PA 不同 flags 返回 Err(FlagsConflict)
- update_flags → update_pte: 仅 rename，语义不变
- identity_map_range: FlagsConflict → panic（配置错误）
- memory::map_mmio: 删除 MMIO_REGIONS BTreeMap + check_mmio_overlap
  重叠检测统一至 PageTable PTE（单一真相源）
- memory::map_mmio: 新增 paddr RAM 校验（拒绝 RAM → Device 重映射）
- MemoryError: 删除 MmioIdentical/MmioOverlap 变体"
```

---

### Task 5: ADR-012 — PageTable lock restructure + hot-path lock-free + ordering upgrade

**Files:**
- Modify: `crates/paging/src/table.rs` (major restructure)
- Modify: `crates/paging/src/lib.rs` (KERNEL_PAGE_TABLE type change)
- Modify: `crates/paging/src/mapping.rs` (remove `.lock()`)
- Modify: `crates/paging/src/mmio.rs` (remove `.lock()`)
- Modify: `crates/memory/src/init.rs` (remove `.lock()`)
- Modify: `src/boot.rs` (remove `.lock()`)
- Modify: `tests/paging-test/src/table.rs` (remove `mut`, remove `.lock()`)
- Modify: `tests/paging-test/src/mapping.rs` (remove `.lock()`)
- Modify: `tests/paging-test/src/conflict_panic.rs` (remove `mut`)

- [ ] **Step 1: Upgrade `Table::read/write` ordering + add `swap`**

In `crates/paging/src/table.rs`, update `Table`:

```rust
struct Table {
    base: *mut AtomicU64,
}

impl Table {
    #[inline]
    unsafe fn from_paddr(paddr: PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut AtomicU64,
        }
    }

    #[inline]
    fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let val = unsafe { (*self.base.add(index)).load(Ordering::Acquire) };
        PageTableEntry::from_raw(val)
    }

    #[inline]
    fn write(&self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Release) };
    }

    /// 原子交换 PTE，返回旧值。用于无锁 update_pte。
    #[inline]
    fn swap(&self, index: usize, pte: PageTableEntry) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        let old = unsafe { (*self.base.add(index)).swap(pte.as_raw(), Ordering::AcqRel) };
        PageTableEntry::from_raw(old)
    }
}
```

Note: `write` method signature changes from `&mut self` to `&self` — atomic stores don't need exclusive access.

- [ ] **Step 2: Restructure `PageTable` — internal lock on `nodes` only**

Rewrite `PageTable` with lock moved inside:

```rust
/// 多级页表——无锁 hot path + 内部锁保护中间节点分配。
///
/// SAS 全量映射下 PTE 只建不删——中间节点一旦建立永久有效，
/// 无 dangling 引用风险。这让无锁 walk 和原子 PTE 更新成为可能。
///
/// 锁粒度：
/// - `root`：创建后永不变动，无需同步保护
/// - `nodes`：仅 `walk_create`（建 PTE）路径需要互斥
/// - 单个 PTE：`AtomicU64` per-entry，`Acquire/Release` ordering
pub struct PageTable {
    /// 根帧——create 后永不变动，无需同步保护。
    root: AllocatedFrames,
    /// 中间节点容器——仅 `create_pte` 的慢路径（建新 PTE）需要锁。
    nodes: sync_crate::SpinLock<Vec<AllocatedFrames>>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PagingError> {
        let root = crate::alloc_node_frame()?;
        Ok(Self {
            root,
            nodes: sync_crate::SpinLock::new(
                Vec::new(),
                "pt_nodes",
                sync_crate::lock_level::KERNEL_PT,
            ),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root.start_paddr()
    }

    /// 无锁只读 walk——SAS 下 PTE 只建不删，walker 不会读到悬挂。
    fn walk_to_leaf(&self, va: VirtAddr) -> Option<(PageTableEntry, PhysAddr, usize, usize)> {
        let mut paddr = self.root.start_paddr();

        for level in (0..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧（节点只增不删）
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx); // Acquire
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, paddr, idx, level));
            }
            paddr = pte.paddr();
        }

        None
    }

    /// 只改 flags，不分配中间节点——无锁。
    ///
    /// **调用方必须在此操作后执行 TLB 刷新。**
    pub fn update_pte(
        &self,
        va: VirtAddr,
        new_flags: PteFlags,
    ) -> Result<PteFlags, PagingError> {
        let (pte, paddr, idx, leaf_level) =
            self.walk_to_leaf(va).ok_or(PagingError::PageNotMapped)?;

        let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
        let new_pte = PageTableEntry::new(pte.paddr(), leaf_flags);
        // SAFETY: paddr 指向叶 PTE 所在的帧——由 self.root 或 self.nodes 持有
        let table = unsafe { Table::from_paddr(paddr) };
        let old = table.swap(idx, new_pte);

        Ok(PageTableEntry::from_raw(old.as_raw()).flags())
    }

    /// 查询虚拟地址的映射信息——无锁。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, _, _, level) = self.walk_to_leaf(va)?;
        let page_size = crate::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 创建单个虚拟页（4KB）的 PTE。
    ///
    /// 快速路径（PTE 已存在）无锁；慢路径（需分配中间节点）取锁。
    ///
    /// SAS 全量映射下 PTE 始终存在。此方法的语义：
    /// - 若该 VA 无 PTE → 创建 Level 0 叶 PTE，返回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同且 flags 相同 → 幂等，���回 Ok
    /// - 若该 VA 已有 PTE 且 PA 相同但 flags 不同 → Err(FlagsConflict)；
    ///   显式修改 flags 请使用 [`update_pte`](Self::update_pte)
    /// - 若该 VA 已有 PTE 但 PA 不同 → panic（内核 bug）
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新。**
    pub fn create_pte(
        &self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PagingError> {
        let leaf_flags = flags.for_leaf_at_level(0);

        // 快速路径：先无锁检查 PTE 是否已存在
        if let Some((pte, _, _, level)) = self.walk_to_leaf(va) {
            if pte.paddr() != pa {
                panic!(
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    pte.paddr(),
                    pa
                );
            }
            let existing_leaf_flags = flags.for_leaf_at_level(level);
            if pte.flags() == existing_leaf_flags {
                return Ok(()); // 幂等
            }
            return Err(PagingError::FlagsConflict);
        }

        // 慢路径：PTE 不存在，需要分配中间节点——取锁
        let mut nodes = self.nodes.lock();
        self.walk_create_and_write(&mut nodes, va, pa, leaf_flags)
    }

    /// 持锁建立新 PTE——walk 到 Level 0 并按需分配中间节点，写入叶 PTE。
    ///
    /// TOCTOU re-check：快速路径到取锁之间，另一核可能已建立该 PTE。
    fn walk_create_and_write(
        &self,
        nodes: &mut Vec<AllocatedFrames>,
        va: VirtAddr,
        pa: PhysAddr,
        leaf_flags: PteFlags,
    ) -> Result<(), PagingError> {
        let mut paddr = self.root.start_paddr();

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = crate::alloc_node_frame()?;
                let frame_paddr = frame.start_paddr();
                nodes.push(frame);
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                panic!(
                    "walk_create: VA {} 在 level {} 遇到非预期的大页叶 PTE（页表损坏）",
                    va, level
                );
            } else {
                paddr = pte.paddr();
            }
        }

        let idx = vpn_index(va, 0);
        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let current = table.read(idx);

        // TOCTOU re-check：另一核可能在我们取锁期间已建立该 PTE
        if current.is_valid() {
            if current.paddr() != pa {
                panic!(
                    "create_pte: VA {} 已指向 PA {}，试图改为 PA {}（不同 PA 是内核 bug）",
                    va,
                    current.paddr(),
                    pa
                );
            }
            if current.flags() == leaf_flags {
                return Ok(()); // 幂等
            }
            return Err(PagingError::FlagsConflict);
        }

        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        Ok(())
    }

    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA），仅使用 4KB 页。
    ///
    /// **调用方必须在此操作后执行架构相关的 TLB 刷新。**
    ///
    /// # Panics
    ///
    /// `start >= end` 或映射冲突时 panic。
    pub fn identity_map_range(&self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        assert!(
            addr.as_usize() < end_aligned.as_usize(),
            "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
        );

        while addr.as_usize() < end_aligned.as_usize() {
            let va = VirtAddr::new(addr.as_usize());
            match self.create_pte(va, addr, flags) {
                Ok(()) => {}
                Err(PagingError::FlagsConflict) => panic!(
                    "identity_map_range: VA {} flags 冲突——已有 PTE 的权限与请求不同",
                    va
                ),
                Err(e) => panic!("identity_map_range: 设置 {va} 权限失败: {e}"),
            }
            addr += config::PAGE_SIZE;
        }
    }
}
```

- [ ] **Step 3: Update `paging/src/lib.rs` — remove outer SpinLock**

```rust
/// 全局内核页表——SAS 架构下只有一张页表。
///
/// 内部锁保护中间节点分配；hot-path（update_pte, get_mapping）无锁。
static KERNEL_PAGE_TABLE: spin::Once<PageTable> = spin::Once::new();

/// 初始化全局内核页表——消费 `PageTable` 的所有权，写入静态存储。
///
/// 仅在启动时调用一次。
pub fn init_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| pt);
}

/// 获取全局内核页表引用。
///
/// 未初始化时 panic。
pub fn kernel_page_table() -> &'static PageTable {
    KERNEL_PAGE_TABLE
        .get()
        .expect("kernel page table not initialized")
}
```

- [ ] **Step 4: Update `mapping.rs` — remove `.lock()` calls**

```rust
fn batch_update_flags(va_start: VirtAddr, page_count: usize, flags: PteFlags) {
    let pt = crate::kernel_page_table();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        pt.update_pte(va, flags)
            .expect("batch_update_flags: update_pte 失败");
    }
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}
```

- [ ] **Step 5: Update `mmio.rs` — remove `.lock()`**

In `crates/paging/src/mmio.rs:36-39`:

```rust
    pub fn map(paddr: PhysAddr, size: usize) -> Result<Self, PagingError> {
        let pa_aligned = paddr.align_down();
        let end_aligned = (paddr + size).align_up();
        let mapped_size = end_aligned.as_usize() - pa_aligned.as_usize();
        let va = memory_types::VirtAddr::new(pa_aligned.as_usize());

        let pt = crate::kernel_page_table();
        pt.identity_map_range(pa_aligned, end_aligned, PteFlags::kernel_device());

        Ok(Self {
            base: va,
            size: mapped_size,
        })
    }
```

- [ ] **Step 6: Update `memory/src/init.rs` — remove `.lock()` calls**

```rust
pub fn init() {
    // ... (heap init, frame allocator init unchanged) ...

    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    // 背景层：identity-map 全部物理内存
    {
        let mem_end = mem_start + mem_size;
        paging::kernel_page_table().identity_map_range(mem_start, mem_end, PteFlags::kernel_rw());
    }

    // ... (overlay layer unchanged) ...
}

pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let pt = paging::kernel_page_table();
    activate(pt);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
```

- [ ] **Step 7: Update `src/boot.rs` — remove `.lock()`**

Line 46-48:

```rust
    {
        let pt = paging::kernel_page_table();
        unsafe { Arch::activate_page_table(pt) };
    }
```

Note: `Arch::activate_page_table` signature might need `&PageTable` instead of `&SpinLockGuard<PageTable>`. Check if it takes `&PageTable` or auto-derefs through the guard. If it takes a generic `impl Deref<Target=PageTable>`, it should work. If it specifically takes `&PageTable`, no change needed since we now pass `&PageTable` directly.

- [ ] **Step 8: Update all tests — remove `mut`, remove `.lock()`**

In `tests/paging-test/src/table.rs`:
- All `let mut pt = PageTable::create()` → `let pt = PageTable::create()`
- All `pt.create_pte(...)` already takes `&self` now
- All `pt.update_pte(...)` already takes `&self` now

In `tests/paging-test/src/mapping.rs`:
- All `paging::kernel_page_table().lock()` → `paging::kernel_page_table()`
- All `guard.get_mapping(...)` → `pt.get_mapping(...)` (rename variable)
- All `guard.update_pte(...)` → `pt.update_pte(...)`

In `tests/paging-test/src/conflict_panic.rs`:
- `let mut pt = ...` → `let pt = ...`
- `pt.create_pte(...)` (already `&self`)

- [ ] **Step 9: Build and test**

Run: `cargo fmt && cargo clippy -- -D warnings`
Run: `cargo xtask build --arch riscv64`
Run: `cargo xtask build --arch aarch64`
Expected: Compiles without errors on both architectures.

- [ ] **Step 10: Run full QEMU test suite**

Run: `cargo xtask test --arch riscv64`
Expected: All tests pass, including:
- `basic` — page table parameter validation
- `table` — core page table operations with `create_pte`/`update_pte`
- `mapping` — OwnedPages RAII behavior (no CLAIMED checks)
- `conflict-panic` — FlagsConflict triggers panic in identity_map_range
- `equal-range-panic` — empty range validation
- `reversed-range-panic` — reversed range validation
- `frame-alloc-test` — frame allocator lifecycle

- [ ] **Step 11: Commit**

```bash
git add crates/paging/ crates/memory/src/init.rs src/boot.rs tests/paging-test/
git commit --signoff -m "refactor(paging): PageTable 无锁 hot-path + ordering 升级 (ADR-012)

- PageTable 内部锁: SpinLock<PageTable> → PageTable { nodes: SpinLock<Vec> }
- kernel_page_table() 返回 &PageTable（不再需要 .lock()）
- update_pte / get_mapping / walk_to_leaf: 完全无锁（&self）
- create_pte: 快速路径无锁，慢路径（分配中间节点）取内部锁
- Table::read/write: Relaxed → Acquire/Release
- Table::swap: 新增 AcqRel 原子交换（update_pte 使用）
- 所有方法 &mut self → &self
- 所有调用方移除 .lock() 调用"
```

---

### Task 6: Update documentation

**Files:**
- Modify: `crates/frame_allocator/Cargo.toml`
- Modify: `docs/design/memory-subsystem-v2.md` (if exists, update relevant sections)
- Modify: `CLAUDE.md` (update CODE MAP if needed)

- [ ] **Step 1: Update `frame_allocator/Cargo.toml` description**

```toml
description = "Physical frame allocator with RAII ownership tracking"
```

- [ ] **Step 2: Update CLAUDE.md code map entries if needed**

In `CLAUDE.md`, update relevant descriptions:
- `frame_allocator` entry: "typestate 生命周期追踪" → "RAII 所有权追踪"
- Add note about `create_pte`/`update_pte` naming
- Update `kernel_page_table()` return type description

- [ ] **Step 3: Commit**

```bash
git add crates/frame_allocator/Cargo.toml CLAUDE.md
git commit --signoff -m "docs: 更新内存子系统文档 (ADR-008~012)"
```
