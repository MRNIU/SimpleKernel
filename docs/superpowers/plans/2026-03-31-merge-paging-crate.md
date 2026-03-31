# 合并 page_table + mapped_pages → paging crate 重构计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `page_table` 和 `mapped_pages` 合并为单一 `paging` crate，通过 `pub(crate)` 封死 PageTable 的写操作，让 `MappedPages` 成为唯一的映射入口，编译期强制仿射安全。

**Architecture:** `page_table_entry`（纯 PTE 编解码）不动。`page_table` 的全部代码移入新 `paging` crate，PageTable 的 mutating 方法降为 `pub(crate)`。`MappedPages::map_identity` 内部直接调 `identity_map_range`，删除逐页循环和 `unsafe new_borrowed`。`MmioRegion` 内部直接调 `pub(crate)` 方法。`memory` crate 去掉对 `page_table` 的直接依赖，改为只依赖 `paging`。

**Tech Stack:** Rust nightly, `#![no_std]`, Cargo workspace crates

**变更范围：**

| 操作 | 路径 |
|------|------|
| 新建 | `crates/paging/Cargo.toml` |
| 新建 | `crates/paging/src/lib.rs` |
| 移入 | `crates/paging/src/table.rs`（来自 `page_table/src/table.rs`） |
| 移入 | `crates/paging/src/error.rs`（合并两个 crate 的 error） |
| 移入 | `crates/paging/src/mapping.rs`（来自 `mapped_pages/src/mapping.rs`） |
| 移入 | `crates/paging/src/mmio.rs`（来自 `mapped_pages/src/mmio.rs`） |
| 删除 | `crates/page_table/` 整个目录 |
| 删除 | `crates/mapped_pages/` 整个目录 |
| 修改 | `Cargo.toml`（workspace members） |
| 修改 | `crates/memory/Cargo.toml` |
| 修改 | `crates/memory/src/lib.rs` |
| 修改 | `crates/memory/src/node_frame.rs` |
| 修改 | `crates/memory/src/init.rs` |
| 修改 | `crates/memory/src/vma.rs` |
| 修改 | `crates/memory/src/error.rs` |
| 修改 | `src/arch/aarch64/mod.rs` |
| 修改 | 根 `Cargo.toml` dependencies |

---

### Task 1: 创建 paging crate 骨架

**Files:**
- Create: `crates/paging/Cargo.toml`
- Create: `crates/paging/src/lib.rs`

- [ ] **Step 1: 创建 `crates/paging/Cargo.toml`**

```toml
[package]
name = "paging"
version.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Page table + affine-type mapping ownership (MappedPages, MmioRegion)"
edition.workspace = true

[features]
default = []
## 导出 HeapNodeFrame + test_pt 供下游 crate 测试使用。
test-support = ["frame_allocator/test-support"]

[dependencies]
config = { path = "../config" }
address = { path = "../address" }
page_table_entry = { path = "../page_table_entry" }
frame_allocator = { path = "../frame_allocator" }
sync_crate = { path = "../sync", package = "sync" }
tlb = { path = "../tlb" }
zerocopy.workspace = true
heapless.workspace = true

[dev-dependencies]
frame_allocator = { path = "../frame_allocator", features = ["test-support"] }
```

- [ ] **Step 2: 创建最小 `crates/paging/src/lib.rs`**

先建一个能编译的空壳，后续 Task 逐步填充：

```rust
//! 分页子系统——页表、仿射类型映射、MMIO。
//!
//! # 设计
//!
//! `PageTable` 的写操作（`map_page`、`unmap_page`、`identity_map_range`）
//! 为 `pub(crate)`，外部只能通过 [`MappedPages`] 或 [`MmioRegion`] 建立映射。
//! 编译期强制仿射安全——持有 `MappedPages` 即证明映射有效，Drop 时自动清理。

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod error;
```

- [ ] **Step 3: 在 workspace 中注册 paging crate**

修改根 `Cargo.toml` 的 `workspace.members`，将 `"crates/page_table"` 和 `"crates/mapped_pages"` 替换为 `"crates/paging"`：

```toml
# 删除:
#    "crates/page_table",
#    "crates/mapped_pages",
# 添加:
    "crates/paging",
```

同时在根 `[dependencies]` 中将 `page_table = { path = "crates/page_table" }` 替换为 `paging = { path = "crates/paging" }`。

- [ ] **Step 4: 验证 crate 骨架编译**

Run: `cargo check -p paging 2>&1 | head -20`
Expected: 编译成功（error 模块待填充，可能有 warning）

- [ ] **Step 5: Commit**

```bash
git add crates/paging/ Cargo.toml
git commit --signoff -m "$(cat <<'EOF'
feat(paging): 创建 paging crate 骨架

合并 page_table + mapped_pages 的第一步：建立新 crate 结构，
注册到 workspace。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: 合并 error 模块

**Files:**
- Create: `crates/paging/src/error.rs`

两个 crate 各有一个 error 模块，合并为统一的 error。`PageTableError` 是内部错误（页表层），`PagingError` 是对外错误（包含帧分配失败）。

- [ ] **Step 1: 创建合并后的 `crates/paging/src/error.rs`**

```rust
//! 分页子系统错误定义。

use core::fmt;

/// 页表内部错误——`pub(crate)`，外部不可见。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageTableError {
    /// 节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
    /// walk 路径上遇到大页
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 无效地址范围（start >= end）
    InvalidRange,
}

impl fmt::Display for PageTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::AlreadyMapped => write!(f, "virtual address already mapped"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::InvalidRange => write!(f, "invalid address range"),
        }
    }
}

/// 分页操作错误——对外公开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// 节点帧分配失败
    AllocationFailed,
    /// 目标 VA 已被映射
    AlreadyMapped,
    /// walk 路径上遇到大页冲突
    HugePageConflict,
    /// 目标 VA 未映射
    PageNotMapped,
    /// 无效地址范围
    InvalidRange,
    /// 物理帧分配失败
    FrameAllocFailed,
}

impl fmt::Display for PagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => write!(f, "page table node frame allocation failed"),
            Self::AlreadyMapped => write!(f, "virtual address already mapped"),
            Self::HugePageConflict => write!(f, "huge page conflict in walk path"),
            Self::PageNotMapped => write!(f, "virtual address not mapped"),
            Self::InvalidRange => write!(f, "invalid address range"),
            Self::FrameAllocFailed => write!(f, "physical frame allocation failed"),
        }
    }
}

impl core::error::Error for PagingError {}

impl From<PageTableError> for PagingError {
    fn from(e: PageTableError) -> Self {
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped => Self::AlreadyMapped,
            PageTableError::HugePageConflict => Self::HugePageConflict,
            PageTableError::PageNotMapped => Self::PageNotMapped,
            PageTableError::InvalidRange => Self::InvalidRange,
        }
    }
}

impl From<frame_allocator::FrameAllocError> for PagingError {
    fn from(_: frame_allocator::FrameAllocError) -> Self {
        Self::FrameAllocFailed
    }
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p paging 2>&1 | head -20`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/error.rs
git commit --signoff -m "$(cat <<'EOF'
feat(paging): 合并 error 模块

PageTableError 降为 pub(crate)（内部），PagingError 对外公开。
统一两个旧 crate 的错误类型。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: 移入 PageTable 核心（lib.rs 常量/trait + table.rs）

**Files:**
- Modify: `crates/paging/src/lib.rs`
- Create: `crates/paging/src/table.rs`

将 `page_table` crate 的全部代码移入 `paging`，**同时将 PageTable 的 mutating 方法降为 `pub(crate)`**。

- [ ] **Step 1: 更新 `crates/paging/src/lib.rs`**

将原 `page_table/src/lib.rs` 中的常量、trait、HeapNodeFrame 搬入，调整 error 引用：

```rust
//! 分页子系统——页表、仿射类型映射、MMIO。
//!
//! # 设计
//!
//! `PageTable` 的写操作（`map_page`、`unmap_page`、`identity_map_range`）
//! 为 `pub(crate)`，外部只能通过 [`MappedPages`] 或 [`MmioRegion`] 建立映射。
//! 编译期强制仿射安全——持有 `MappedPages` 即证明映射有效，Drop 时自动清理。
//!
//! PTE 编解码由 [`page_table_entry`] crate 提供。

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use address::PhysAddr;
use core::sync::atomic::{AtomicU64, Ordering};

pub mod error;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

/// PTE 大小的位移量——`log2(sizeof(u64))` = 3。
///
/// 两种架构的 PTE 均为 64 位，此常量在所有架构下一致。
pub const PTE_SIZE_SHIFT: usize = core::mem::size_of::<u64>().trailing_zeros() as usize;

pub mod table;
pub use table::PageTable;

/// 页表节点帧的统一接口。
///
/// 消费方通过实现此 trait 向页表注入帧分配能力：
/// - 裸机：`impl NodeFrameOps for NodeFrame`（在 `memory` crate 中）
/// - 测试：`impl NodeFrameOps for HeapNodeFrame`（本 crate 内置）
pub trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, error::PageTableError>;
    /// 获取帧的物理地址（裸机 identity mapping）或堆地址（测试）。
    fn paddr(&self) -> PhysAddr;
}

/// 测试用页表节点帧——从堆分配，模拟物理帧。
///
/// 通过 `test-support` feature 或 `cfg(test)` 启用。
#[cfg(any(test, feature = "test-support"))]
pub struct HeapNodeFrame {
    ptr: *mut u8,
    layout: core::alloc::Layout,
}

// SAFETY: HeapNodeFrame 独占其分配的内存（*mut u8 阻止了 auto-Send），
// 可安全跨线程传递。
#[cfg(any(test, feature = "test-support"))]
unsafe impl Send for HeapNodeFrame {}

#[cfg(any(test, feature = "test-support"))]
impl Drop for HeapNodeFrame {
    fn drop(&mut self) {
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
        unsafe { alloc::alloc::dealloc(self.ptr, self.layout) };
    }
}

#[cfg(any(test, feature = "test-support"))]
impl NodeFrameOps for HeapNodeFrame {
    fn alloc() -> Result<Self, error::PageTableError> {
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("HeapNodeFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err(error::PageTableError::AllocationFailed);
        }
        Ok(Self { ptr, layout })
    }
    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
    }
}

/// 每张页表中的条目数（PAGE_SIZE / sizeof(PTE)）。
///
/// 64 位架构中 PTE 均为 8 字节，4KB 页对应 512 条目。
pub const ENTRIES_PER_TABLE: usize = config::PAGE_SIZE / core::mem::size_of::<u64>();

/// 单级索引位宽（log2(ENTRIES_PER_TABLE)）。
pub(crate) const INDEX_BITS: usize = config::PAGE_SIZE_BITS - PTE_SIZE_SHIFT;

/// 层级参数。
#[derive(Clone, Copy)]
pub struct LevelInfo {
    /// 该级 VPN 在虚拟地址中的起始位位置
    pub shift: usize,
    /// 索引掩码
    pub index_mask: usize,
}

/// 最大页表层级数（Sv57 五级）。
const MAX_LEVELS: usize = 5;

/// 编译期计算各级层级参数。
const fn compute_level_info() -> [LevelInfo; MAX_LEVELS] {
    let mask = ENTRIES_PER_TABLE - 1;
    let mut info = [LevelInfo {
        shift: 0,
        index_mask: mask,
    }; MAX_LEVELS];
    info[0].shift = config::PAGE_SIZE_BITS;
    let mut i = 1;
    while i < MAX_LEVELS {
        info[i].shift = info[i - 1].shift + INDEX_BITS;
        i += 1;
    }
    info
}

pub const LEVEL_INFO: [LevelInfo; MAX_LEVELS] = compute_level_info();

/// 页表节点——封装 PTE 数组的原子访问。
///
/// 使用 `AtomicU64` 保证 SMP 下单个 PTE 读写不会 torn read/write。
/// 外层 `SpinLock` 负责更高层的互斥，此处仅保证单次访问的原子性。
pub(crate) struct Table {
    base: *mut AtomicU64,
}

impl Table {
    /// 从物理地址构造页表节点。
    ///
    /// # Safety
    /// - `paddr` 必须指向有效、页对齐的帧
    #[inline]
    pub(crate) unsafe fn from_paddr(paddr: address::PhysAddr) -> Self {
        Self {
            base: paddr.as_usize() as *mut AtomicU64,
        }
    }

    #[inline]
    pub(crate) fn read(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查。
        // Relaxed 即可——外层 SpinLock 提供必要的 memory barrier。
        let val = unsafe { (*self.base.add(index)).load(Ordering::Relaxed) };
        PageTableEntry::from_raw(val)
    }

    #[inline]
    pub(crate) fn write(&mut self, index: usize, pte: PageTableEntry) {
        debug_assert!(index < ENTRIES_PER_TABLE, "PTE index out of bounds");
        // SAFETY: base 指向有效帧，index 经 debug_assert 检查
        unsafe { (*self.base.add(index)).store(pte.as_raw(), Ordering::Relaxed) };
    }
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
#[inline]
pub(crate) fn vpn_index(va: address::VirtAddr, level: usize) -> usize {
    let info = &LEVEL_INFO[level];
    (va.as_usize() >> info.shift) & info.index_mask
}

/// 返回第 `level` 级映射的页大小（字节）。
#[inline]
pub const fn page_size_at_level(level: usize) -> usize {
    1usize << LEVEL_INFO[level].shift
}

#[cfg(test)]
mod tests {
    use super::*;
    use address::VirtAddr;

    #[test]
    fn level0_params_match_page_size() {
        assert_eq!(LEVEL_INFO[0].shift, config::PAGE_SIZE_BITS);
        assert_eq!(LEVEL_INFO[0].index_mask, ENTRIES_PER_TABLE - 1);
    }

    #[test]
    fn level_shifts_are_chained() {
        for i in 1..LEVEL_INFO.len() {
            assert_eq!(
                LEVEL_INFO[i].shift,
                LEVEL_INFO[i - 1].shift + INDEX_BITS,
                "level {} shift 不正确",
                i
            );
        }
    }

    #[test]
    fn page_size_at_level_values() {
        assert_eq!(page_size_at_level(0), config::PAGE_SIZE);
        assert_eq!(page_size_at_level(1), ENTRIES_PER_TABLE * config::PAGE_SIZE);
        assert_eq!(
            page_size_at_level(2),
            ENTRIES_PER_TABLE * ENTRIES_PER_TABLE * config::PAGE_SIZE
        );
    }

    #[test]
    fn vpn_index_extracts_correct_bits() {
        let va = VirtAddr::new(0x1000);
        assert_eq!(vpn_index(va, 0), 1);
        assert_eq!(vpn_index(va, 1), 0);
        assert_eq!(vpn_index(va, 2), 0);

        let va_high = VirtAddr::new(0x4000_0000);
        assert_eq!(vpn_index(va_high, 0), 0);
        assert_eq!(vpn_index(va_high, 1), 0);
        assert_eq!(vpn_index(va_high, 2), 1);
    }
}
```

**注意**：`NodeFrameOps::alloc()` 的返回类型仍使用内部 `PageTableError`，因为这是 trait 实现方需要构造的类型。`PageTableError` 虽然 `pub(crate)`，但通过 trait 定义中的引用对 implementor 可见（Rust 允许在 pub trait 的关联类型/签名中使用 restricted-visibility 类型——**不对，这不行**）。

**修正**：`NodeFrameOps::alloc()` 返回类型需要是 pub 的。有两个方案：
- 方案 A：让 `PageTableError` 保持 `pub`，但 PageTable 的方法用 `pub(crate)` 限制——类型公开，方法不公开
- 方案 B：`NodeFrameOps::alloc()` 返回 `Result<Self, PagingError>`

方案 A 更简单且不改 trait 语义，采用方案 A。将上面 `error.rs` 中 `PageTableError` 改回 `pub`：

```rust
/// 页表操作错误。
///
/// 注意：此类型 `pub` 是因为 [`NodeFrameOps::alloc`] 签名需要引用它。
/// PageTable 的 mutating 方法本身是 `pub(crate)` 的，外部无法直接调用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableError {
    // ... 同上
}
```

- [ ] **Step 2: 创建 `crates/paging/src/table.rs`**

从 `page_table/src/table.rs` 复制，**关键修改：所有 mutating 方法改为 `pub(crate)`**：

```rust
//! 多级页表——walk / map / unmap 逻辑。

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::error::PageTableError;
use crate::{NodeFrameOps, PageTableEntry, PteFlags, PteFlagsOps, PteOps, Table, vpn_index};
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
///
/// 写操作（`map_page`、`unmap_page` 等）为 `pub(crate)`——
/// 外部必须通过 [`MappedPages`](crate::MappedPages) 建立映射。
pub struct PageTable<F: NodeFrameOps> {
    root_paddr: PhysAddr,
    #[expect(dead_code, reason = "仅用于持有所有权，通过 root_paddr 访问")]
    root: F,
    frames: BTreeMap<PhysAddr, F>,
    ref_counts: BTreeMap<PhysAddr, u16>,
}

impl<F: NodeFrameOps> PageTable<F> {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, PageTableError> {
        let root = F::alloc()?;
        let root_paddr = root.paddr();
        let mut ref_counts = BTreeMap::new();
        ref_counts.insert(root_paddr, 0);
        Ok(Self {
            root_paddr,
            root,
            frames: BTreeMap::new(),
            ref_counts,
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    #[inline]
    fn inc_ref(&mut self, paddr: PhysAddr) {
        *self
            .ref_counts
            .get_mut(&paddr)
            .expect("ref_counts: 帧未注册") += 1;
    }

    #[inline]
    fn dec_ref(&mut self, paddr: PhysAddr) -> u16 {
        let count = self
            .ref_counts
            .get_mut(&paddr)
            .expect("ref_counts: 帧未注册");
        *count -= 1;
        *count
    }

    fn walk_create(
        &mut self,
        va: VirtAddr,
        target_level: usize,
    ) -> Result<(PhysAddr, usize), PageTableError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = F::alloc()?;
                let frame_paddr = frame.paddr();
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.insert(frame_paddr, frame);
                self.ref_counts.insert(frame_paddr, 0);
                self.inc_ref(paddr);
                paddr = frame_paddr;
            } else if pte.is_leaf(level) {
                return Err(PageTableError::HugePageConflict);
            } else {
                paddr = pte.paddr();
            }
        }

        let idx = vpn_index(va, target_level);
        Ok((paddr, idx))
    }

    // ── pub(crate) mutating methods ──

    /// 映射单个虚拟页到物理帧（Level 0，4KB）。
    pub(crate) fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PageTableError> {
        self.map_at_level(va, pa, flags, 0)
    }

    /// 在指定层级映射虚拟地址到物理地址。
    pub(crate) fn map_at_level(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
        level: usize,
    ) -> Result<(), PageTableError> {
        let page_size = crate::page_size_at_level(level);
        debug_assert!(
            va.as_usize().is_multiple_of(page_size),
            "map_at_level: VA {:#x} 未按 level {} 页大小 ({:#x}) 对齐",
            va.as_usize(),
            level,
            page_size
        );
        debug_assert!(
            pa.as_usize().is_multiple_of(page_size),
            "map_at_level: PA {:#x} 未按 level {} 页大小 ({:#x}) 对齐",
            pa.as_usize(),
            level,
            page_size
        );

        let (frame_paddr, idx) = self.walk_create(va, level)?;
        // SAFETY: frame_paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(frame_paddr) };
        let current = table.read(idx);
        if current.is_valid() {
            return Err(PageTableError::AlreadyMapped);
        }
        let leaf_flags = flags.for_leaf_at_level(level);
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
        self.inc_ref(frame_paddr);
        Ok(())
    }

    /// 取消映射单个虚拟页（4KB），返回其原始物理地址。
    pub(crate) fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, PageTableError> {
        self.unmap_page_with_flags(va).map(|(pa, _)| pa)
    }

    /// 取消映射并返回原始物理地址和 PTE 标志。
    pub(crate) fn unmap_page_with_flags(
        &mut self,
        va: VirtAddr,
    ) -> Result<(PhysAddr, PteFlags), PageTableError> {
        self.unmap_at_level_with_flags(va, 0)
    }

    /// 在指定层级取消映射，返回原始物理地址。
    pub(crate) fn unmap_at_level(
        &mut self,
        va: VirtAddr,
        level: usize,
    ) -> Result<PhysAddr, PageTableError> {
        self.unmap_at_level_with_flags(va, level).map(|(pa, _)| pa)
    }

    /// 在指定层级取消映射，返回原始物理地址和 PTE 标志。
    pub(crate) fn unmap_at_level_with_flags(
        &mut self,
        va: VirtAddr,
        level: usize,
    ) -> Result<(PhysAddr, PteFlags), PageTableError> {
        let mut path: [(PhysAddr, usize, PhysAddr); PT_LEVELS] =
            [(PhysAddr::new(0), 0, PhysAddr::new(0)); PT_LEVELS];
        let mut path_len = 0;
        let mut paddr = self.root_paddr;

        for lv in (level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, lv);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return Err(PageTableError::PageNotMapped);
            }
            if pte.is_leaf(lv) {
                return Err(PageTableError::PageNotMapped);
            }
            let child_paddr = pte.paddr();
            path[path_len] = (paddr, idx, child_paddr);
            path_len += 1;
            paddr = child_paddr;
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, level);
        let pte = table.read(idx);
        if !pte.is_valid() || !pte.is_leaf(level) {
            return Err(PageTableError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        let old_flags = pte.flags();
        table.write(idx, PageTableEntry::empty());
        self.dec_ref(paddr);

        let mut child_paddr = paddr;
        for &(parent_paddr, parent_idx, _) in path[..path_len].iter().rev() {
            let count = *self
                .ref_counts
                .get(&child_paddr)
                .expect("ref_counts: 帧未注册");
            if count > 0 {
                break;
            }
            // SAFETY: parent_paddr 指向由 self 持有的有效帧
            let mut parent_table = unsafe { Table::from_paddr(parent_paddr) };
            parent_table.write(parent_idx, PageTableEntry::empty());
            self.frames.remove(&child_paddr);
            self.ref_counts.remove(&child_paddr);
            self.dec_ref(parent_paddr);
            child_paddr = parent_paddr;
        }

        Ok((old_pa, old_flags))
    }

    // ── pub 只读方法 ──

    /// 只读遍历——查找叶 PTE。
    fn walk_readonly(&self, va: VirtAddr) -> Option<(PageTableEntry, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { Table::from_paddr(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, level));
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { Table::from_paddr(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read(idx);
        if pte.is_valid() && pte.is_leaf(0) {
            Some((pte, 0))
        } else {
            None
        }
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, level) = self.walk_readonly(va)?;
        let page_size = crate::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA）。
    ///
    /// 自动使用最大可用页大小（1GB / 2MB / 4KB）。
    /// 失败时自动回滚已建立的映射，保证事务性。
    pub(crate) fn identity_map_range(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), PageTableError> {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        if addr.as_usize() >= end_aligned.as_usize() {
            return Err(PageTableError::InvalidRange);
        }

        let mut mappings: Vec<(VirtAddr, PhysAddr, usize)> = Vec::new();
        while addr.as_usize() < end_aligned.as_usize() {
            let remaining = end_aligned.as_usize() - addr.as_usize();
            let va = VirtAddr::new(addr.as_usize());

            let mut selected_level = 0;
            let mut selected_size = config::PAGE_SIZE;
            for level in (1..PT_LEVELS).rev() {
                let page_size = crate::page_size_at_level(level);
                if addr.as_usize().is_multiple_of(page_size) && remaining >= page_size {
                    selected_level = level;
                    selected_size = page_size;
                    break;
                }
            }
            mappings.push((va, addr, selected_level));
            addr += selected_size;
        }

        for (i, &(va, pa, level)) in mappings.iter().enumerate() {
            if let Err(e) = self.map_at_level(va, pa, flags, level) {
                for &(va, _, level) in mappings[..i].iter().rev() {
                    let _ = self.unmap_at_level(va, level);
                }
                return Err(e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::error::PageTableError;
    use crate::*;
    use address::{PhysAddr, VirtAddr};

    use crate::HeapNodeFrame;
    type PageTable = crate::table::PageTable<HeapNodeFrame>;

    #[test]
    fn root_paddr_is_valid() {
        let pt = PageTable::create().expect("创建测试页表失败");
        assert_ne!(pt.root_paddr().as_usize(), 0);
    }

    // 保留原 page_table/src/table.rs 中的全部测试（约 10+ 个），
    // 此处省略以节约篇幅。实际实现时从原文件复制全部 #[test] 函数。
    // 注意：测试在 crate 内部，可以直接调 pub(crate) 方法。
}
```

- [ ] **Step 3: 验证编译和测试**

Run: `cargo test -p paging 2>&1 | tail -20`
Expected: 所有 `lib::tests` 和 `table::tests` 通过

- [ ] **Step 4: Commit**

```bash
git add crates/paging/src/lib.rs crates/paging/src/table.rs
git commit --signoff -m "$(cat <<'EOF'
feat(paging): 移入 PageTable 核心，mutating 方法降为 pub(crate)

map_page、unmap_page、identity_map_range 等写操作现为 pub(crate)，
外部无法直接调用。只读操作（create、root_paddr、get_mapping）保持 pub。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: 移入 MappedPages（重写 map_identity，删除 new_borrowed）

**Files:**
- Create: `crates/paging/src/mapping.rs`
- Modify: `crates/paging/src/lib.rs`（添加 module 声明和 re-export）

核心改动：
1. `map_identity` 内部直接调 `identity_map_range`（同 crate，pub(crate) 可达），删除逐页循环
2. 删除 `pub unsafe fn new_borrowed`——不再需要，`MmioRegion` 直接用内部构造
3. 错误类型从 `MappedPagesError` 改为 `PagingError`

- [ ] **Step 1: 创建 `crates/paging/src/mapping.rs`**

```rust
//! 仿射类型映射——move-only 的 VA→PA 映射所有权。

use alloc::sync::Arc;

use address::{FrameRange, PhysAddr, PhysPageNum, VirtAddr};
use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, UnmappedFrames};
use sync_crate::SpinLock;

use crate::error::PagingError;
use crate::{NodeFrameOps, PageTable, PteFlags, PteFlagsOps};

/// unmap_and_reclaim 每次处理的最大页数。
///
/// 栈消耗：CHUNK_SIZE × size_of::<PhysAddr>() + heapless::Vec 开销 ≈ 264 字节。
const CHUNK_SIZE: usize = 32;

/// 仿射类型映射——持有此值即证明 VA→PA 映射有效。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
/// Drop 时根据 PTE 中的 EXCLUSIVE 位决定是否释放物理帧。
pub struct MappedPages<F: NodeFrameOps> {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 构造时的请求权限（可能与实际 PTE 不同，如 COW 降级后）
    flags: PteFlags,
    /// 永久映射标记——drop 时不 unmap
    permanent: bool,
    /// 所属页表的 `Arc` 引用——Drop 时通过此引用 unmap。
    page_table: Arc<SpinLock<PageTable<F>>>,
}

impl<F: NodeFrameOps> MappedPages<F> {
    /// Identity-map 一段物理地址区间（VA == PA）。
    ///
    /// **不设置 EXCLUSIVE 位**——drop 时仅 unmap PTE，不释放帧。
    /// 内部委托 [`PageTable::identity_map_range`]，自动选择最大可用页大小。
    ///
    /// # Errors
    ///
    /// 映射冲突时返回错误。
    ///
    /// # Panics
    ///
    /// `page_count` 为 0 时 panic。
    pub fn map_identity(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, PagingError> {
        assert!(
            page_count > 0,
            "MappedPages::map_identity: page_count 不能为 0"
        );
        let va_start = VirtAddr::new(pa_start.as_usize());
        let pa_end = pa_start + page_count * PAGE_SIZE;
        {
            let mut pt = pt_ref.lock();
            pt.identity_map_range(pa_start, pa_end, flags)?;
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            permanent: false,
            page_table: pt_ref,
        })
    }

    /// 分配新帧并建立映射——**设置 EXCLUSIVE 位**。
    ///
    /// 内部分配帧并逐页映射，帧所有权通过 PTE 的 EXCLUSIVE 位追踪。
    /// 虚拟地址由调用方指定（通常通过 `AddressSpace` 的 VMA 管理）。
    ///
    /// # Errors
    ///
    /// 帧分配失败或映射冲突时返回错误。
    ///
    /// # Panics
    ///
    /// `page_count` 为 0 时 panic。
    pub fn map_alloc(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, PagingError> {
        assert!(
            page_count > 0,
            "MappedPages::map_alloc: page_count 不能为 0"
        );
        let exclusive_flags = flags.with_exclusive();
        let mut mapped_count = 0usize;
        let mut pt = pt_ref.lock();
        for i in 0..page_count {
            let frame = AllocatedFrames::alloc_one()?;
            let pa = frame.start_paddr();
            let va = va_start + i * PAGE_SIZE;
            match pt.map_page(va, pa, exclusive_flags) {
                Ok(()) => {
                    // 帧所有权转移到 PTE：forget 阻止 drop 回收
                    let mapped = frame.into_mapped();
                    core::mem::forget(mapped);
                    mapped_count += 1;
                }
                Err(e) => {
                    // 回滚已映射的页——EXCLUSIVE 帧通过 unmap 路径回收
                    for j in (0..mapped_count).rev() {
                        let va = va_start + j * PAGE_SIZE;
                        if let Ok(old_pa) = pt.unmap_page(va) {
                            reclaim_exclusive_frame(old_pa);
                        }
                    }
                    return Err(e.into());
                }
            }
        }
        drop(pt);
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags: exclusive_flags,
            permanent: false,
            page_table: pt_ref,
        })
    }

    /// 仅供 crate 内部使用——包装已由其他方法建立的映射。
    ///
    /// 调用方必须确保映射已在 `pt_ref` 中建立。
    pub(crate) fn wrap_existing(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        vaddr: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Self {
        debug_assert!(page_count > 0);
        Self {
            vaddr,
            page_count,
            flags,
            permanent: false,
            page_table: pt_ref,
        }
    }

    /// 消耗 self，标记为永久映射（drop 时不 unmap）。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    /// 返回映射起始虚拟地址。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.page_count * PAGE_SIZE
    }

    /// 返回构造时的请求权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr + offset).align_down();
        let guard = self.page_table.lock();
        guard
            .get_mapping(page_va)
            .expect("MappedPages::pte_flags: 映射不存在")
            .1
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用生命周期绑定到 `&self`——
    /// 编译器保证 `MappedPages` drop 后无法使用该引用。
    ///
    /// # Panics
    ///
    /// `offset + size_of::<T>()` 超出映射大小、或偏移未对齐时 panic。
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr.as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证映射存活，引用生命周期绑定到 self
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    ///
    /// # Panics
    ///
    /// 越界、未对齐、或 PTE 无 WRITE 权限时 panic。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr.as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type_mut",
        );
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "MappedPages::as_type_mut: PTE 无 WRITE 权限（可能已被 COW 降级）"
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // PTE 可写已验证；FromBytes 保证任意位模式均为合法 T；
        // &mut self 保证映射存活且独占访问
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 从所属页表中 unmap 所有页，EXCLUSIVE 帧自动回收。
    fn unmap_and_reclaim(&self) {
        let mut offset = 0;
        while offset < self.page_count {
            let n = (self.page_count - offset).min(CHUNK_SIZE);
            let mut exclusive_pas: heapless::Vec<PhysAddr, CHUNK_SIZE> = heapless::Vec::new();

            {
                let mut guard = self.page_table.lock();
                for i in 0..n {
                    let va = self.vaddr + (offset + i) * PAGE_SIZE;
                    if let Ok((pa, flags)) = guard.unmap_page_with_flags(va) {
                        if flags.is_exclusive() {
                            exclusive_pas
                                .push(pa)
                                .expect("exclusive 帧数不超过 CHUNK_SIZE");
                        }
                    }
                }
            }

            {
                let flush_va = self.vaddr + offset * PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            }

            for pa in &exclusive_pas {
                reclaim_exclusive_frame(*pa);
            }

            offset += n;
        }
    }
}

/// 验证偏移在映射范围内且地址对齐到 `T` 的自然边界。
pub(crate) fn check_bounds_and_align<T>(
    base: usize,
    size: usize,
    offset: usize,
    fn_name: &str,
) -> *const T {
    let type_size = core::mem::size_of::<T>();
    assert!(
        offset + type_size <= size,
        "{fn_name}: offset {:#x} + {type_size} 超出映射大小 {:#x}",
        offset,
        size,
    );
    let addr = base + offset;
    let align = core::mem::align_of::<T>();
    assert!(
        addr.is_multiple_of(align),
        "{fn_name}: 地址 {:#x} 未对齐到 {align} 字节",
        addr,
    );
    addr as *const T
}

/// 从物理地址重建 `UnmappedFrames` 并 drop 回收。
fn reclaim_exclusive_frame(pa: PhysAddr) {
    let ppn = PhysPageNum::from(pa);
    let range = FrameRange::new(ppn, ppn + 1);
    // SAFETY: 帧刚从页表 unmap，EXCLUSIVE 保证唯一引用
    let _reclaimed = unsafe { UnmappedFrames::from_range(range) };
}

impl<F: NodeFrameOps> Drop for MappedPages<F> {
    fn drop(&mut self) {
        if self.permanent {
            return;
        }
        self.unmap_and_reclaim();
    }
}

impl<F: NodeFrameOps> core::fmt::Debug for MappedPages<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = if self.permanent {
            "permanent"
        } else if self.flags.is_exclusive() {
            "exclusive"
        } else {
            "borrowed"
        };
        write!(
            f,
            "MappedPages({}, {} pages, {:?}, {})",
            self.vaddr, self.page_count, self.flags, kind
        )
    }
}

/// 创建测试用 `Arc<SpinLock<PageTable>>`。
#[cfg(any(test, feature = "test-support"))]
pub fn test_pt() -> Arc<SpinLock<PageTable<crate::HeapNodeFrame>>> {
    let pt = PageTable::create().expect("创建页表");
    Arc::new(SpinLock::new(pt, "test_pt"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HeapNodeFrame;

    type Mp = MappedPages<HeapNodeFrame>;

    #[test]
    fn map_identity_basic() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x1_0000);
        let mp = Mp::map_identity(pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect("map_identity 应成功");
        assert_eq!(mp.vaddr(), VirtAddr::new(0x1_0000));
        assert_eq!(mp.size(), PAGE_SIZE);
        {
            let guard = pt_ref.lock();
            let (got_pa, got_flags) = guard
                .get_mapping(VirtAddr::new(0x1_0000))
                .expect("应能查到映射");
            assert_eq!(got_pa, pa);
            assert!(!got_flags.is_exclusive());
        }
        let _ = mp.into_permanent();
    }

    #[test]
    fn map_identity_conflict_rollback() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x2_0000);
        let _mp1 = Mp::map_identity(pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect("首次 map 应成功")
            .into_permanent();
        let err =
            Mp::map_identity(pt_ref, pa, 1, PteFlags::kernel_rw()).expect_err("重复 map 应失败");
        assert!(matches!(err, PagingError::AlreadyMapped | PagingError::HugePageConflict));
    }

    #[test]
    fn into_permanent_marks_permanent() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x3_0000);
        let mp = Mp::map_identity(pt_ref, pa, 1, PteFlags::kernel_rw())
            .expect("map 应成功")
            .into_permanent();
        let dbg = alloc::format!("{:?}", mp);
        assert!(dbg.contains("permanent"));
    }

    #[test]
    fn map_identity_multi_page() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x5_0000);
        let _mp = Mp::map_identity(pt_ref.clone(), pa, 3, PteFlags::kernel_ro())
            .expect("多页 map 应成功")
            .into_permanent();
        let guard = pt_ref.lock();
        for i in 0..3 {
            let va = VirtAddr::new(0x5_0000 + i * PAGE_SIZE);
            assert!(guard.get_mapping(va).is_some(), "第 {} 页应已映射", i);
        }
    }

    #[test]
    fn map_alloc_sets_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x10_0000);
        let mp =
            Mp::map_alloc(pt_ref.clone(), va, 1, PteFlags::kernel_rw()).expect("map_alloc 应成功");

        let guard = pt_ref.lock();
        let (_, got_flags) = guard.get_mapping(va).expect("应能查到映射");
        assert!(got_flags.is_exclusive());
        drop(guard);
        assert!(mp.flags().is_exclusive());
        let _ = mp.into_permanent();
    }

    #[test]
    fn map_alloc_multi_page_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x20_0000);
        let _mp = Mp::map_alloc(pt_ref.clone(), va, 3, PteFlags::kernel_rw())
            .expect("多页 map_alloc 应成功")
            .into_permanent();
        let guard = pt_ref.lock();
        for i in 0..3 {
            let page_va = VirtAddr::new(0x20_0000 + i * PAGE_SIZE);
            let (_, flags) = guard.get_mapping(page_va).expect("应已映射");
            assert!(flags.is_exclusive(), "第 {} 页应有 EXCLUSIVE 位", i);
        }
    }

    #[test]
    fn as_type_reads_mapped_memory() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        let mp = MappedPages::wrap_existing(pt_ref, va, 1, PteFlags::kernel_rw())
            .into_permanent();
        let val: &u32 = mp.as_type::<u32>(0);
        assert_eq!(*val, 0);
        drop(buf);
    }

    #[test]
    #[should_panic(expected = "超出映射大小")]
    fn as_type_out_of_bounds_panics() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        let mp = MappedPages::wrap_existing(pt_ref, va, 1, PteFlags::kernel_rw())
            .into_permanent();
        let _: &u32 = mp.as_type::<u32>(PAGE_SIZE);
    }

    #[test]
    #[should_panic(expected = "page_count 不能为 0")]
    fn map_identity_zero_pages_panics() {
        let pt_ref = test_pt();
        let _ = Mp::map_identity(pt_ref, PhysAddr::new(0x1000), 0, PteFlags::kernel_rw());
    }

    #[test]
    #[should_panic(expected = "page_count 不能为 0")]
    fn map_alloc_zero_pages_panics() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let _ = Mp::map_alloc(pt_ref, VirtAddr::new(0x1000), 0, PteFlags::kernel_rw());
    }

    #[test]
    fn drop_identity_unmaps_pte() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x6_0000);
        let va = VirtAddr::new(0x6_0000);
        let mp = Mp::map_identity(pt_ref.clone(), pa, 2, PteFlags::kernel_rw())
            .expect("map_identity 应成功");
        {
            let guard = pt_ref.lock();
            assert!(guard.get_mapping(va).is_some());
            assert!(guard.get_mapping(va + PAGE_SIZE).is_some());
        }
        drop(mp);
        let guard = pt_ref.lock();
        assert!(guard.get_mapping(va).is_none());
        assert!(guard.get_mapping(va + PAGE_SIZE).is_none());
    }

    #[test]
    fn drop_alloc_unmaps_and_reclaims() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x30_0000);
        let mp =
            Mp::map_alloc(pt_ref.clone(), va, 2, PteFlags::kernel_rw()).expect("map_alloc 应成功");
        {
            let guard = pt_ref.lock();
            let (_, flags) = guard.get_mapping(va).expect("应能查到映射");
            assert!(flags.is_exclusive());
        }
        drop(mp);
        let guard = pt_ref.lock();
        assert!(guard.get_mapping(va).is_none());
        assert!(guard.get_mapping(va + PAGE_SIZE).is_none());
    }
}
```

- [ ] **Step 2: 更新 `crates/paging/src/lib.rs` 添加 mapping 模块**

在 `pub mod error;` 之后添加：

```rust
#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mapping;

#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub use mapping::MappedPages;
#[cfg(any(test, feature = "test-support"))]
pub use mapping::test_pt;
```

- [ ] **Step 3: 验证编译和测试**

Run: `cargo test -p paging 2>&1 | tail -20`
Expected: 所有 mapping 测试通过

- [ ] **Step 4: Commit**

```bash
git add crates/paging/src/mapping.rs crates/paging/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(paging): 移入 MappedPages，map_identity 委托 identity_map_range

map_identity 不再逐页循环，直接调 pub(crate) identity_map_range，
自动获得大页支持和内置回滚。删除 pub unsafe new_borrowed，
替换为 pub(crate) wrap_existing。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: 移入 MmioRegion（简化，直接调 pub(crate) 方法）

**Files:**
- Create: `crates/paging/src/mmio.rs`
- Modify: `crates/paging/src/lib.rs`

`MmioRegion` 现在在同一 crate 内，不需要通过 `new_borrowed` 绕行。

- [ ] **Step 1: 创建 `crates/paging/src/mmio.rs`**

```rust
//! 类型化 MMIO 区域——强制 volatile 语义的映射包装。

use alloc::sync::Arc;

use crate::error::PagingError;
use crate::mapping::{MappedPages, check_bounds_and_align};
use crate::{NodeFrameOps, PageTable, PteFlags, PteFlagsOps};
use address::PhysAddr;
use sync_crate::SpinLock;

/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
///
/// 内部通过 [`MappedPages`] 管理页表映射和生命周期。
pub struct MmioRegion<F: NodeFrameOps> {
    mapping: MappedPages<F>,
}

impl<F: NodeFrameOps> MmioRegion<F> {
    /// 将 `[paddr, paddr+size)` identity-map 到指定页表，返回 `MmioRegion`。
    ///
    /// 内部直接调 `identity_map_range`（同 crate，无需 unsafe 绕行）。
    ///
    /// # Errors
    ///
    /// 映射失败时返回错误。
    pub fn map_to(
        pt_ref: Arc<SpinLock<PageTable<F>>>,
        paddr: PhysAddr,
        size: usize,
    ) -> Result<Self, PagingError> {
        let pa_aligned = paddr.align_down();
        let end = paddr + size;
        let page_count = (end.align_up().as_usize() - pa_aligned.as_usize()) / config::PAGE_SIZE;
        {
            let mut guard = pt_ref.lock();
            guard.identity_map_range(pa_aligned, end, PteFlags::kernel_device())?;
        }
        let mapping = MappedPages::wrap_existing(
            pt_ref,
            address::VirtAddr::new(pa_aligned.as_usize()),
            page_count,
            PteFlags::kernel_device(),
        );
        Ok(Self { mapping })
    }

    /// 标记为永久映射——drop 时不 unmap。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.mapping = self.mapping.into_permanent();
        self
    }

    /// 返回 MMIO 区域的基地址。
    #[must_use]
    pub fn base(&self) -> address::VirtAddr {
        self.mapping.vaddr()
    }

    /// 返回 MMIO 区域的大小。
    #[must_use]
    pub fn size(&self) -> usize {
        self.mapping.size()
    }

    /// 读取指定偏移处的寄存器值（volatile 语义）。
    ///
    /// # Panics
    ///
    /// 越界或未对齐时 panic。
    #[inline]
    pub fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.mapping.vaddr().as_usize(),
            self.mapping.size(),
            offset,
            "MmioRegion::read_reg",
        );
        // SAFETY: 地址已映射为 device memory，越界和对齐已验证
        unsafe { core::ptr::read_volatile(ptr) }
    }

    /// 写入指定偏移处的寄存器值（volatile 语义）。
    ///
    /// # Panics
    ///
    /// 越界或未对齐时 panic。
    #[inline]
    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.mapping.vaddr().as_usize(),
            self.mapping.size(),
            offset,
            "MmioRegion::write_reg",
        );
        // SAFETY: 地址已映射为 device memory，越界和对齐已验证
        unsafe { core::ptr::write_volatile(ptr as *mut T, val) }
    }
}

impl<F: NodeFrameOps> core::fmt::Debug for MmioRegion<F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MmioRegion({}, size={:#x})",
            self.mapping.vaddr(),
            self.mapping.size(),
        )
    }
}
```

- [ ] **Step 2: 更新 `crates/paging/src/lib.rs` 添加 mmio 模块**

在 mapping 模块声明之后添加：

```rust
#[cfg(any(test, feature = "test-support", target_os = "none"))]
pub mod mmio;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p paging 2>&1 | head -20`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add crates/paging/src/mmio.rs crates/paging/src/lib.rs
git commit --signoff -m "$(cat <<'EOF'
feat(paging): 移入 MmioRegion，消除 unsafe new_borrowed 绕行

MmioRegion 现在直接调 pub(crate) identity_map_range + wrap_existing，
不再需要 unsafe 构造。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: 迁移 memory crate 依赖

**Files:**
- Modify: `crates/memory/Cargo.toml`
- Modify: `crates/memory/src/lib.rs`
- Modify: `crates/memory/src/node_frame.rs`
- Modify: `crates/memory/src/error.rs`
- Modify: `crates/memory/src/vma.rs`
- Modify: `crates/memory/src/init.rs`

将 `memory` 对 `page_table` 和 `mapped_pages` 的依赖替换为 `paging`。

- [ ] **Step 1: 更新 `crates/memory/Cargo.toml`**

```toml
[dependencies]
# 删除:
#   page_table = { path = "../page_table" }
#   mapped_pages_crate = { path = "../mapped_pages", package = "mapped_pages" }
# 替换为:
paging = { path = "../paging" }

# 其余依赖不变:
config = { path = "../config" }
address = { path = "../address" }
sync_crate = { path = "../sync", package = "sync" }
per_cpu = { path = "../per_cpu" }
frame_allocator = { path = "../frame_allocator" }
heap_crate = { path = "../heap", package = "heap" }
tlb = { path = "../tlb" }

bitflags.workspace = true
zerocopy.workspace = true
log.workspace = true
spin.workspace = true

[dev-dependencies]
# 删除:
#   page_table = { path = "../page_table", features = ["test-support"] }
#   mapped_pages_crate = { path = "../mapped_pages", package = "mapped_pages", features = ["test-support"] }
# 替换为:
paging = { path = "../paging", features = ["test-support"] }
frame_allocator = { path = "../frame_allocator", features = ["test-support"] }
```

- [ ] **Step 2: 更新 `crates/memory/src/node_frame.rs`**

所有 `page_table::` 改为 `paging::`：

```rust
//! NodeFrame 桥接与 PageTable 类型别名。
//!
//! `NodeFrame` 是 `memory` crate 的本地 newtype，桥接
//! `frame_allocator::AllocatedFrames`（外部类型）和
//! `paging::NodeFrameOps`（外部 trait），绕过孤儿规则。

/// 页表节点帧——包装 `AllocatedFrames` 以实现 `NodeFrameOps`。
#[cfg(target_os = "none")]
pub struct NodeFrame(frame_allocator::AllocatedFrames);

#[cfg(target_os = "none")]
impl paging::NodeFrameOps for NodeFrame {
    fn alloc() -> Result<Self, paging::error::PageTableError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|_| paging::error::PageTableError::AllocationFailed)
    }
    fn paddr(&self) -> address::PhysAddr {
        self.0.start_paddr()
    }
}

/// 具体化的页表类型——隐藏泛型参数 `F`。
#[cfg(target_os = "none")]
pub type PageTable = paging::PageTable<NodeFrame>;
#[cfg(test)]
pub type PageTable = paging::PageTable<paging::HeapNodeFrame>;

impl From<paging::error::PageTableError> for crate::error::MemoryError {
    fn from(e: paging::error::PageTableError) -> Self {
        use paging::error::PageTableError;
        match e {
            PageTableError::AllocationFailed => Self::AllocationFailed,
            PageTableError::AlreadyMapped
            | PageTableError::HugePageConflict
            | PageTableError::InvalidRange => Self::MapFailed,
            PageTableError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
```

- [ ] **Step 3: 更新 `crates/memory/src/lib.rs`**

```rust
//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_os = "none", feature(sync_unsafe_cell))]

#[cfg(any(test, target_os = "none"))]
extern crate alloc;

/// 错误类型。
pub mod error;
/// 物理帧分配器与帧生命周期状态机（re-export `frame_allocator` crate）。
#[cfg(any(test, target_os = "none"))]
pub use frame_allocator as frame;
/// 堆分配器（re-export `heap` crate）。
#[cfg(target_os = "none")]
pub use heap_crate as heap;
/// 全局内存状态。
pub mod globals;
/// 内存子系统初始化（依赖链接器符号，裸机专用）。
#[cfg(target_os = "none")]
pub mod init;
/// NodeFrame 桥接与 PageTable 类型别名。
pub mod node_frame;
/// 虚拟内存区域（VMA）与地址空间管理。
#[cfg(any(test, target_os = "none"))]
pub mod vma;

/// TLB 管理（re-export `tlb` crate）。
pub use tlb;

/// 仿射类型映射——具体化的类型别名。
#[cfg(target_os = "none")]
pub type MappedPages = paging::MappedPages<node_frame::NodeFrame>;
/// 仿射类型映射——测试用类型别名。
#[cfg(test)]
pub type MappedPages = paging::MappedPages<paging::HeapNodeFrame>;

/// MMIO 区域——具体化的类型别名。
#[cfg(target_os = "none")]
pub type MmioRegion = paging::mmio::MmioRegion<node_frame::NodeFrame>;
/// MMIO 区域——测试用类型别名。
#[cfg(test)]
pub type MmioRegion = paging::mmio::MmioRegion<paging::HeapNodeFrame>;

/// 映射错误类型 re-export。
pub use paging::error::PagingError;

// 公共 API re-export
pub use globals::{MEMORY_INFO, MemoryInfo};
#[cfg(any(test, target_os = "none"))]
pub use globals::{
    kernel_address_space, kernel_page_table, store_kernel_address_space, store_kernel_page_table,
};

#[cfg(any(test, target_os = "none"))]
pub use address::{phys_to_virt, virt_to_phys};

#[cfg(target_os = "none")]
pub use init::{init, init_smp};

/// 将 MMIO 物理地址区间 identity-map 到内核页表，返回对应虚拟地址。
#[cfg(any(test, target_os = "none"))]
pub fn map_mmio(
    paddr: address::PhysAddr,
    size: usize,
) -> Result<address::VirtAddr, error::MemoryError> {
    use paging::{PteFlags, PteFlagsOps};

    let kpt = kernel_page_table().ok_or(error::MemoryError::InvalidPageTable)?;
    let region = MmioRegion::map_to(kpt, paddr, size)?;
    let vaddr = region.base();
    let region_size = region.size();
    let _permanent = region.into_permanent();

    if let Some(kas) = kernel_address_space() {
        kas.lock()
            .register_existing(
                vaddr,
                region_size,
                PteFlags::kernel_device(),
                vma::VmaKind::Identity,
            )
            .expect("MMIO 区域注册到内核地址空间失败——可能是重复映射");
    }

    Ok(vaddr)
}
```

- [ ] **Step 4: 更新 `crates/memory/src/error.rs`**

将 `mapped_pages_crate::error::MappedPagesError` 的 From impl 替换为 `paging::error::PagingError`：

```rust
impl From<paging::error::PagingError> for MemoryError {
    fn from(e: paging::error::PagingError) -> Self {
        use paging::error::PagingError;
        match e {
            PagingError::AllocationFailed | PagingError::FrameAllocFailed => Self::AllocationFailed,
            PagingError::AlreadyMapped
            | PagingError::HugePageConflict
            | PagingError::InvalidRange => Self::MapFailed,
            PagingError::PageNotMapped => Self::PageNotMapped,
        }
    }
}
```

删除旧的 `From<MappedPagesError>` impl。

- [ ] **Step 5: 更新 `crates/memory/src/vma.rs`**

将 `use page_table::{PteFlags, PteFlagsOps};` 改为 `use paging::{PteFlags, PteFlagsOps};`。

将 `AddressSpace::mmap_identity_range` 中直接调 `pt.identity_map_range()` 的代码改为走 `MappedPages::map_identity`：

```rust
pub fn mmap_identity_range(
    &mut self,
    start: VirtAddr,
    end: VirtAddr,
    flags: PteFlags,
) -> Result<&Vma, MemoryError> {
    let start_aligned = start.align_down();
    let end_aligned = end.align_up();
    if start_aligned.as_usize() >= end_aligned.as_usize() {
        return Err(MemoryError::MapFailed);
    }
    let range = AddrRange::new(start_aligned, end_aligned);
    self.check_overlap(range)?;

    let page_count = (end_aligned - start_aligned) / PAGE_SIZE;
    let pa_start = address::PhysAddr::new(start_aligned.as_usize());
    let mapping = MappedPages::map_identity(
        self.page_table.clone(), pa_start, page_count, flags
    )?.into_permanent();

    let vma = Vma {
        range,
        flags,
        kind: VmaKind::Identity,
        mapping: Some(mapping), // 不再是 None！有 RAII 了
    };
    self.areas.insert(start_aligned, vma);
    Ok(self.areas.get(&start_aligned).expect("刚插入的 VMA"))
}
```

同时更新 `use crate::MappedPages;` 和引用 `mapped_pages_crate` 的测试 import（`use paging::test_pt;` 替换 `use mapped_pages_crate::test_pt;`）。

- [ ] **Step 6: 更新 `crates/memory/src/init.rs`**

将 `use page_table::{PteFlags, PteFlagsOps};` 改为 `use paging::{PteFlags, PteFlagsOps};`。

- [ ] **Step 7: 验证 memory crate 编译和测试**

Run: `cargo test -p memory 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 8: Commit**

```bash
git add crates/memory/
git commit --signoff -m "$(cat <<'EOF'
refactor(memory): 迁移依赖从 page_table+mapped_pages 到 paging

去掉对 page_table 和 mapped_pages 的直接依赖，
统一使用 paging crate。mmap_identity_range 改走 MappedPages，
消除 mapping: None 的无 RAII 路径。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: 迁移内核代码和根 Cargo.toml

**Files:**
- Modify: `Cargo.toml`（根，dependencies）
- Modify: `src/arch/aarch64/mod.rs`

- [ ] **Step 1: 更新根 `Cargo.toml` dependencies**

将 `page_table = { path = "crates/page_table" }` 替换为 `paging = { path = "crates/paging" }`。

- [ ] **Step 2: 更新 `src/arch/aarch64/mod.rs`**

将 `use page_table::{PteFlags, PteFlagsOps};`（第 60 行）改为 `use paging::{PteFlags, PteFlagsOps};`。

- [ ] **Step 3: 全局搜索确认无残留引用**

Run: `grep -r 'page_table::' src/ crates/memory/ --include='*.rs' | grep -v '/target/' | grep -v '// '`
Run: `grep -r 'mapped_pages_crate' src/ crates/memory/ --include='*.rs' | grep -v '/target/'`
Expected: 无输出

- [ ] **Step 4: 验证全项目编译**

Run: `cargo check 2>&1 | tail -20`
Expected: 编译成功（忽略与本次重构无关的已知 warning）

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/
git commit --signoff -m "$(cat <<'EOF'
refactor: 内核代码迁移到 paging crate

src/arch/aarch64 和根 Cargo.toml 从 page_table 迁移到 paging。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: 删除旧 crate

**Files:**
- Delete: `crates/page_table/` 整个目录
- Delete: `crates/mapped_pages/` 整个目录
- Modify: `Cargo.toml`（确认 workspace members 已更新）

- [ ] **Step 1: 确认无任何残留引用**

Run: `grep -r 'page_table' Cargo.toml crates/*/Cargo.toml src/ --include='*.toml' --include='*.rs' | grep -v '/target/' | grep -v 'page_table_entry' | grep -v 'crates/paging/' | grep -v '// '`
Expected: 无输出（所有引用已迁移）

- [ ] **Step 2: 删除旧 crate 目录**

```bash
rm -rf crates/page_table/
rm -rf crates/mapped_pages/
```

- [ ] **Step 3: 验证全项目编译和测试**

Run: `cargo test -p paging -p memory 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 4: Commit**

```bash
git add -A
git commit --signoff -m "$(cat <<'EOF'
refactor(paging): 删除旧 page_table 和 mapped_pages crate

两个 crate 已合并为 paging，PageTable 的写操作为 pub(crate)，
MappedPages 是唯一的映射入口。编译期强制仿射安全。

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: 更新 page_table README 和清理

**Files:**
- Delete: `crates/page_table/README.md`（已随 Task 8 删除）
- Verify: 无文档引用旧 crate 名

- [ ] **Step 1: 搜索文档中对旧 crate 名的引用**

Run: `grep -r 'page_table' docs/ --include='*.md' | grep -v 'page_table_entry' | head -20`

对于搜索到的结果，判断是否需要更新（设计文档中的历史引用可以保留，但活跃文档应更新）。

- [ ] **Step 2: 如有需要，更新 CLAUDE.md 中的 crate 引用**

将 CODE MAP 表格中 `page_table` 相关行更新为 `paging`。

- [ ] **Step 3: Commit**

```bash
git add -A
git commit --signoff -m "$(cat <<'EOF'
docs: 更新文档中 page_table → paging 的引用

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```
