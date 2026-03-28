# Theseus 风格 PTE Flags 重构

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除 `PageFlags` 翻译层，每个架构直接定义自己的 `PteFlags` bitflags，上层通过语义方法（`kernel_rw()` 等）使用，不感知位布局。

**Architecture:** 删除 `mod.rs` 里的通用 `PageFlags`，在 `pte_riscv64.rs` 和 `pte_aarch64.rs` 中各自定义 `PteFlags`，通过 `cfg` re-export 为统一名称。`PageTableEntry` 的 `flags()` 方法返回架构原生 `PteFlags`，不做反向翻译。`MappedPages` / `MmioRegion` / `lib.rs` 中所有 `PageFlags` 引用改为 `PteFlags`。

**Tech Stack:** Rust nightly, `bitflags`, `#[cfg(target_arch)]`

---

## 文件变更概览

| 文件 | 动作 | 职责 |
|------|------|------|
| `crates/memory/src/page_table/pte_riscv64.rs` | **改** | 定义 `PteFlags`（Sv39 原生位），`PageTableEntry` 直接使用 |
| `crates/memory/src/page_table/pte_aarch64.rs` | **改** | 定义 `PteFlags`（ARMv8 描述符原生位），`PageTableEntry` 直接使用 |
| `crates/memory/src/page_table/mod.rs` | **改** | 删除 `PageFlags`，re-export `PteFlags` |
| `crates/memory/src/page_table/table.rs` | **改** | `PageFlags` → `PteFlags` |
| `crates/memory/src/page_table/tests.rs` | **改** | `PageFlags` → `PteFlags`，更新测试 |
| `crates/memory/src/mapped_pages.rs` | **改** | `PageFlags` → `PteFlags` |
| `crates/memory/src/mmio.rs` | **改** | `PageFlags` → `PteFlags` |
| `crates/memory/src/lib.rs` | **改** | `PageFlags` → `PteFlags` |
| `src/arch/aarch64/mod.rs` | **改** | `PageFlags` → `PteFlags` |

---

### Task 1: RISC-V — 在 `pte_riscv64.rs` 中定义 `PteFlags`

**Files:**
- Modify: `crates/memory/src/page_table/pte_riscv64.rs`

当前 RISC-V 侧直接用 `PageFlags` 的 bits，位布局本身就是 Sv39 原生的，所以改动最小——把 `PageFlags` 的定义搬到这里，改名为 `PteFlags`。

- [ ] **Step 1: 将 `PteFlags` 定义写入 `pte_riscv64.rs`**

将文件内容替换为：

```rust
//! RISC-V Sv39/Sv48/Sv57 PTE 编码。
//!
//! PTE 格式（所有 Sv 模式共用）：
//! - bits [9:0]：flags（V/R/W/X/U/G/A/D + RSW）
//! - bits [53:10]：PPN（物理页号）
//! - bits [63:54]：保留

use bitflags::bitflags;

use super::PageTableEntry;
use address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

/// flags 位宽（RISC-V PTE 格式固定 10 位：bits [9:0]）
const FLAGS_BITS: u32 = 10;

/// PPN 掩码：bits [53:10]
const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

bitflags! {
    /// RISC-V Sv39/Sv48 页表项标志位——硬件原生位布局。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
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

impl PteFlags {
    /// 内核读写数据映射 (V | R | W | G | A | D)。
    #[inline]
    pub fn kernel_rw() -> Self {
        Self::VALID | Self::READ | Self::WRITE | Self::GLOBAL | Self::ACCESSED | Self::DIRTY
    }

    /// 内核读-执行映射 (V | R | X | G | A)。
    #[inline]
    pub fn kernel_rx() -> Self {
        Self::VALID | Self::READ | Self::EXECUTE | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核只读映射 (V | R | G | A)。
    #[inline]
    pub fn kernel_ro() -> Self {
        Self::VALID | Self::READ | Self::GLOBAL | Self::ACCESSED
    }

    /// 内核读写执行映射 (V | R | W | X | G | A | D)。
    #[inline]
    pub fn kernel_rwx() -> Self {
        Self::VALID
            | Self::READ
            | Self::WRITE
            | Self::EXECUTE
            | Self::GLOBAL
            | Self::ACCESSED
            | Self::DIRTY
    }
}

impl PageTableEntry {
    /// 从物理地址和标志构造 PTE。
    #[inline]
    pub fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        let ppn = ((paddr.as_usize() as u64) >> PAGE_SHIFT) << FLAGS_BITS;
        Self(ppn | flags.bits())
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((((self.0 & PPN_MASK) >> FLAGS_BITS) << PAGE_SHIFT) as usize)
    }

    /// 从 PTE 提取标志位。
    #[inline]
    pub fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & 0xFF)
    }

    /// PTE 是否有效（V 位）。
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

    /// 是否为叶节点（R/W/X 至少有一个设置）。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & (PteFlags::READ | PteFlags::WRITE | PteFlags::EXECUTE).bits() != 0
    }

    /// 空 PTE（全零）。
    #[inline]
    pub fn empty() -> Self {
        Self(0)
    }

    /// 中间节点 PTE（仅 V 位，指向下一级页表）。
    #[inline]
    pub fn new_intermediate(paddr: PhysAddr) -> Self {
        Self::new(paddr, PteFlags::VALID)
    }
}
```

---

### Task 2: AArch64 — 在 `pte_aarch64.rs` 中定义 `PteFlags`

**Files:**
- Modify: `crates/memory/src/page_table/pte_aarch64.rs`

这是核心变更：消除翻译层。`PteFlags` 直接用 ARM 描述符的原生位位置。`new()` 不再做 `PageFlags → ARM bits` 翻译，而是直接 `|` 进 PTE。`flags()` 直接返回 `PteFlags`，不做反向翻译。

- [ ] **Step 1: 将 `PteFlags` 定义写入 `pte_aarch64.rs`**

将文件内容替换为：

```rust
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

use super::PageTableEntry;
use address::PhysAddr;

const PAGE_SHIFT: u32 = config::PAGE_SIZE.trailing_zeros();

/// 输出地址掩码——根据 PAGE_SIZE 自动适配：
/// - 4KB (PAGE_SHIFT=12)：bits [47:12]
/// - 16KB (PAGE_SHIFT=14)：bits [47:14]
/// - 64KB (PAGE_SHIFT=16)：bits [47:16]
const OUTPUT_ADDR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF & !((1u64 << PAGE_SHIFT) - 1);

bitflags! {
    /// AArch64 ARMv8 页表项标志位——硬件原生位布局。
    ///
    /// 位位置直接对应 ARMv8 Stage 1 描述符定义，不做任何翻译。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
        /// bit [0]：描述符有效
        const VALID     = 1 << 0;
        /// bit [1]：Table/Page 类型（1 = table/page，0 = block）
        const TABLE     = 1 << 1;
        /// bits [4:2]：MAIR 索引 0（Normal memory，Writeback）
        const MAIR_IDX0 = 0b000 << 2;
        /// bits [4:2]：MAIR 索引 1（Device-nGnRnE）
        const MAIR_IDX1 = 0b001 << 2;
        /// AP[7:6] = 0b01：EL0 可访问（unprivileged）
        const AP_UNPRIV = 0b01 << 6;
        /// AP[7:6] = 0b10：只读（Read-Only）
        const AP_RO     = 0b10 << 6;
        /// bits [9:8] = 0b11：Inner Shareable
        const SH_INNER  = 0b11 << 8;
        /// bit [10]：Access Flag（硬件已访问）
        const AF        = 1 << 10;
        /// bit [11]：non-Global（设 1 = 非全局，清 0 = 全局）
        const NG        = 1 << 11;
        /// bit [53]：Privileged eXecute Never
        const PXN       = 1 << 53;
        /// bit [54]：User eXecute Never（EL0 不可执行）
        const UXN       = 1 << 54;
    }
}

impl PteFlags {
    /// 内核读写数据映射。
    ///
    /// Valid | Table | AF | SH_INNER | MAIR_IDX0 | PXN | UXN
    /// （不可执行，全局，EL1 读写）
    #[inline]
    pub fn kernel_rw() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::MAIR_IDX0 | Self::PXN | Self::UXN
    }

    /// 内核读-执行映射。
    ///
    /// Valid | Table | AF | SH_INNER | MAIR_IDX0 | AP_RO | UXN
    /// （EL1 只读+可执行，EL0 不可执行）
    #[inline]
    pub fn kernel_rx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::MAIR_IDX0 | Self::AP_RO | Self::UXN
    }

    /// 内核只读映射。
    ///
    /// Valid | Table | AF | SH_INNER | MAIR_IDX0 | AP_RO | PXN | UXN
    #[inline]
    pub fn kernel_ro() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::MAIR_IDX0 | Self::AP_RO | Self::PXN | Self::UXN
    }

    /// 内核读写执行映射。
    ///
    /// Valid | Table | AF | SH_INNER | MAIR_IDX0
    /// （EL1 读写+可执行，EL0 不可执行——注意 UXN 已设但 PXN 未设）
    #[inline]
    pub fn kernel_rwx() -> Self {
        Self::VALID | Self::TABLE | Self::AF | Self::SH_INNER | Self::MAIR_IDX0 | Self::UXN
    }

    /// 该标志是否表示可写（AP_RO 未设置）。
    #[inline]
    pub fn is_writable(self) -> bool {
        !self.contains(Self::AP_RO)
    }
}

impl PageTableEntry {
    /// 从物理地址和标志构造叶/页描述符。
    ///
    /// 直接将 `flags` 与地址组合，不做任何翻译。
    #[inline]
    pub fn new(paddr: PhysAddr, flags: PteFlags) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK) | flags.bits();
        Self(bits)
    }

    /// 构造表描述符（指向下一级页表）。
    #[inline]
    pub fn new_table(paddr: PhysAddr) -> Self {
        let bits = (paddr.as_usize() as u64 & OUTPUT_ADDR_MASK)
            | PteFlags::VALID.bits()
            | PteFlags::TABLE.bits();
        Self(bits)
    }

    /// 从 PTE 提取物理地址。
    #[inline]
    pub fn paddr(self) -> PhysAddr {
        PhysAddr::new((self.0 & OUTPUT_ADDR_MASK) as usize)
    }

    /// 从 PTE 提取标志位（原生 ARMv8 位，无翻译）。
    #[inline]
    pub fn flags(self) -> PteFlags {
        PteFlags::from_bits_truncate(self.0 & !OUTPUT_ADDR_MASK)
    }

    /// PTE 是否有效。
    #[inline]
    pub fn is_valid(self) -> bool {
        self.contains(PteFlags::VALID)
    }

    /// 是否为叶节点（page/block 描述符，非 table 描述符）。
    ///
    /// 叶描述符同时设置 Valid + AF；table 描述符只有 Valid + Table。
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.is_valid() && self.contains(PteFlags::AF)
    }

    /// 检查 PTE 是否包含指定标志。
    #[inline]
    fn contains(self, flags: PteFlags) -> bool {
        PteFlags::from_bits_truncate(self.0).contains(flags)
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
```

---

### Task 3: 更新 `mod.rs` — 删除 `PageFlags`，re-export `PteFlags`

**Files:**
- Modify: `crates/memory/src/page_table/mod.rs`

- [ ] **Step 1: 删除 `PageFlags` 定义和 import，添加 `PteFlags` re-export**

删除以下内容：
1. `use bitflags::bitflags;` import
2. 整个 `bitflags! { ... PageFlags ... }` 块（行 23-38）
3. 整个 `impl PageFlags { ... }` 块（行 41-71）

在 `mod pte_riscv64;` / `mod pte_aarch64;` 声明之后添加 re-export：

```rust
#[cfg(all(not(test), target_arch = "aarch64"))]
pub use pte_aarch64::PteFlags;
#[cfg(any(test, target_arch = "riscv64"))]
pub use pte_riscv64::PteFlags;
```

模块注释也需更新——将第一行改为：

```rust
//! 多级页表抽象。
//!
//! - `PteFlags` / `PageTableEntry`：各架构定义自己的 PTE 标志位（`pte_*.rs`）
//! - `table.rs`：页表 walk / map / unmap 逻辑
```

`PageTableEntry` 的文档注释也需更新：

```rust
/// 单个硬件页表项（64 位）。
///
/// 方法 `new`、`paddr`、`flags`、`is_valid`、`is_leaf`、`empty`、`new_intermediate`
/// 以及 `PteFlags` 由各架构的 `pte_*.rs` 提供。
```

---

### Task 4: 更新 `table.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `crates/memory/src/page_table/table.rs`

- [ ] **Step 1: 替换引用**

行 10 的 import：
```rust
// 旧
use super::{Level0, PageFlags, PageTableEntry, Table, vpn_index};
// 新
use super::{Level0, PageTableEntry, PteFlags, Table, vpn_index};
```

行 180（`map_page` 签名）：
```rust
// 旧
    flags: PageFlags,
// 新
    flags: PteFlags,
```

行 208（`get_mapping` 返回类型）：
```rust
// 旧
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PageFlags)> {
// 新
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
```

---

### Task 5: 更新 `tests.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `crates/memory/src/page_table/tests.rs`

- [ ] **Step 1: 全局替换 `PageFlags` → `PteFlags`**

所有 `PageFlags` 引用替换为 `PteFlags`。测试逻辑不变——因为测试环境使用 RISC-V 路径（`#[cfg(any(test, target_arch = "riscv64"))]`），`PteFlags` 的位布局与旧 `PageFlags` 完全一致。

---

### Task 6: 更新 `mapped_pages.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `crates/memory/src/mapped_pages.rs`

- [ ] **Step 1: 替换所有 `PageFlags` 引用**

行 22 import：
```rust
// 旧
use crate::page_table::PageFlags;
// 新
use crate::page_table::PteFlags;
```

行 57（`MappedPages` 结构体字段）：
```rust
// 旧
    flags: PageFlags,
// 新
    flags: PteFlags,
```

行 76（`map_identity` 签名）：
```rust
// 旧
        flags: PageFlags,
// 新
        flags: PteFlags,
```

行 110（`map_alloc` 签名）：
```rust
// 旧
        flags: PageFlags,
// 新
        flags: PteFlags,
```

行 173（`flags()` 返回类型）：
```rust
// 旧
    pub fn flags(&self) -> PageFlags {
// 新
    pub fn flags(&self) -> PteFlags {
```

行 219（`as_type_mut` 中的 debug_assert）：
```rust
// 旧
            self.flags.contains(PageFlags::WRITE),
// 新——AArch64 没有 WRITE 位，改用语义方法
            self.flags.is_writable(),
```

**注意**：`PteFlags::is_writable()` 需要在 RISC-V 侧也定义。

---

### Task 7: 为 RISC-V `PteFlags` 添加 `is_writable()` 方法

**Files:**
- Modify: `crates/memory/src/page_table/pte_riscv64.rs`

- [ ] **Step 1: 在 `impl PteFlags` 块末尾添加**

```rust
    /// 该标志是否表示可写。
    #[inline]
    pub fn is_writable(self) -> bool {
        self.contains(Self::WRITE)
    }
```

这是两个架构的统一语义接口——RISC-V 检查 `WRITE` 位，AArch64 检查 `AP_RO` 未设置。

---

### Task 8: 更新 `mmio.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `crates/memory/src/mmio.rs`

- [ ] **Step 1: 替换引用**

行 22 import：
```rust
// 旧
use crate::page_table::PageFlags;
// 新
use crate::page_table::PteFlags;
```

行 52（`MmioRegion::map` 中）：
```rust
// 旧
            MappedPages::map_identity(&mut guard, pa_aligned, page_count, PageFlags::kernel_rw())?;
// 新
            MappedPages::map_identity(&mut guard, pa_aligned, page_count, PteFlags::kernel_rw())?;
```

---

### Task 9: 更新 `lib.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `crates/memory/src/lib.rs`

- [ ] **Step 1: 替换引用**

行 31 import：
```rust
// 旧
use page_table::{PageFlags, PageTable};
// 新
use page_table::{PageTable, PteFlags};
```

行 79（`identity_map_range` 签名）：
```rust
// 旧
    flags: PageFlags,
// 新
    flags: PteFlags,
```

行 118-121（`init()` 中的使用）：
```rust
// 旧
    identity_map_range(&mut pt, mem_start, text_end, PageFlags::kernel_rwx())
// 新
    identity_map_range(&mut pt, mem_start, text_end, PteFlags::kernel_rwx())
```

```rust
// 旧
        PageFlags::kernel_rw(),
// 新
        PteFlags::kernel_rw(),
```

行 176（`map_mmio` 中）：
```rust
// 旧
    identity_map_range(&mut *guard, paddr, paddr + size, PageFlags::kernel_rw())?;
// 新
    identity_map_range(&mut *guard, paddr, paddr + size, PteFlags::kernel_rw())?;
```

---

### Task 10: 更新 `src/arch/aarch64/mod.rs` — `PageFlags` → `PteFlags`

**Files:**
- Modify: `src/arch/aarch64/mod.rs`

- [ ] **Step 1: 替换引用**

```rust
// 旧
        use memory::page_table::PageFlags;
        ...
        memory::identity_map_range(pt, start, end, PageFlags::kernel_rw())?;
// 新
        use memory::page_table::PteFlags;
        ...
        memory::identity_map_range(pt, start, end, PteFlags::kernel_rw())?;
```

---

### Task 11: 运行测试验证

- [ ] **Step 1: 运行单元测试**

Run: `cargo test -p memory`
Expected: 所有测试通过，包括 `pte_roundtrip`、`page_flags_presets`（现在叫 `pte_flags_presets`）、`map_and_get_mapping` 等。

- [ ] **Step 2: 运行 clippy**

Run: `cargo clippy -p memory -- -D warnings`
Expected: 无 warning

- [ ] **Step 3: 运行格式检查**

Run: `cargo fmt --check`
Expected: 无格式问题

- [ ] **Step 4: 交叉编译验证**

Run: `cargo xtask build --arch riscv64`
Expected: 编译通过

Run: `cargo xtask build --arch aarch64`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add crates/memory/src/page_table/pte_riscv64.rs \
       crates/memory/src/page_table/pte_aarch64.rs \
       crates/memory/src/page_table/mod.rs \
       crates/memory/src/page_table/table.rs \
       crates/memory/src/page_table/tests.rs \
       crates/memory/src/mapped_pages.rs \
       crates/memory/src/mmio.rs \
       crates/memory/src/lib.rs \
       src/arch/aarch64/mod.rs
git commit --signoff -m "refactor(memory): PageFlags → 架构原生 PteFlags（Theseus 风格）

删除通用 PageFlags 翻译层，每架构直接定义 PteFlags：
- RISC-V：位布局不变（原来就是 Sv39 原生）
- AArch64：直接用 ARMv8 描述符位，消除正/反向翻译
- 上层通过 kernel_rw()/kernel_rx()/is_writable() 语义方法使用"
```
