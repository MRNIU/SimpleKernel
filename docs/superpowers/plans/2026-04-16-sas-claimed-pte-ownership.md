# SAS PTE CLAIMED 位所有权追踪 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除 init 阶段 reserved/free 帧范围的双重所有权问题，用 PTE 软件保留位（RISC-V RSW / AArch64 bit 55）实现运行时所有权检测。

**Architecture:** SAS 架构下全部物理内存永久 identity-mapped（PTE 永不删除）。MappedPages 语义从"创建/销毁映射"改为"声明/释放所有权"——通过 PTE 的 CLAIMED 软件位标记。init 阶段按正确权限逐段映射所有物理内存（不使用大页），reserved 帧范围仅覆盖内核段（不包含 free memory），消除与 buddy 的重叠。

**Tech Stack:** Rust nightly, `#![no_std]`, `bitflags`, RISC-V Sv39 / AArch64 ARMv8 PTE

**遗留项（本次不做）：** `AllocatedFrames::alloc` 的清零操作可后续移除，让调用方按需清零，减少 alloc-free 周期的双重写入开销。

---

## 文件结构

| 文件 | 职责 | 操作 |
|------|------|------|
| `crates/page_table_entry/src/lib.rs` | PteFlagsOps trait | 修改：新增 CLAIMED 相关方法 |
| `crates/page_table_entry/src/riscv64.rs` | RISC-V PTE 编解码 | 修改：添加 CLAIMED 位 + 实现 trait 方法 |
| `crates/page_table_entry/src/aarch64.rs` | AArch64 PTE 编解码 | 修改：添加 CLAIMED 位 + 实现 trait 方法 |
| `crates/paging/src/table.rs` | 页表 walk/map/unmap | 修改：map_page/unmap_page → pub(crate)，删除大页相关 |
| `crates/paging/src/mapping.rs` | MappedPages RAII 类型 | 重写：claim/release 语义 + poison pattern |
| `crates/paging/src/mmio.rs` | MmioRegion | 修改：适配 map_page pub(crate) |
| `crates/paging/src/lib.rs` | paging crate 入口 | 修改：更新 re-export 和模块文档 |
| `crates/paging/src/error.rs` | PagingError | 修改：新增 CLAIMED 相关错误变体 |
| `crates/config/src/lib.rs` | 全局常量 | 修改：新增 FREED_PAGE_POISON |
| `crates/memory/src/init.rs` | 内存初始化 | 重写：拆分 data_pages + identity map free memory |
| `crates/memory/src/vma.rs` | VMA 管理 | 修改：适配 claim/release API |
| `crates/memory/src/error.rs` | MemoryError | 修改：新增 CLAIMED 相关错误 |
| `tests/paging-test/src/mapping.rs` | 映射测试 | 重写：适配新语义 + 新增 double claim 测试 |

---

### Task 1: PTE 层——添加 CLAIMED 软件位

**Files:**
- Modify: `crates/page_table_entry/src/lib.rs:26-71`
- Modify: `crates/page_table_entry/src/riscv64.rs:37-49`
- Modify: `crates/page_table_entry/src/aarch64.rs:38-68`

- [ ] **Step 1: 在 PteFlagsOps trait 中添加 CLAIMED 方法**

在 `crates/page_table_entry/src/lib.rs` 的 `PteFlagsOps` trait 中，在"查询方法"和"Builder 方法"分组中分别添加：

```rust
// 查询方法分组中添加：

/// PTE 是否被 MappedPages 声明了所有权（软件保留位）。
fn is_claimed(self) -> bool;

// Builder 方法分组中添加：

/// 设置或清除 CLAIMED 所有权标记位。
fn with_claimed(self, claimed: bool) -> Self;
```

- [ ] **Step 2: RISC-V 实现——使用 RSW[0] (bit 8)**

在 `crates/page_table_entry/src/riscv64.rs` 的 `PteFlags` bitflags 中添加：

```rust
/// 软件保留位 RSW[0]——MappedPages 所有权标记。
/// 硬件忽略此位（RISC-V Privileged Spec §5.4）。
const CLAIMED = 1 << 8;
```

在 `impl PteFlagsOps for PteFlags` 中添加：

```rust
#[inline]
fn is_claimed(self) -> bool {
    self.contains(Self::CLAIMED)
}

#[inline]
fn with_claimed(self, claimed: bool) -> Self {
    if claimed {
        self | Self::CLAIMED
    } else {
        self.difference(Self::CLAIMED)
    }
}
```

注意：`FLAGS_BITS` 常量为 10，`from_bits_truncate` 已覆盖 bits [9:0]，CLAIMED (bit 8) 在范围内，无需修改掩码。

- [ ] **Step 3: AArch64 实现——使用 bit 55**

在 `crates/page_table_entry/src/aarch64.rs` 的 `PteFlags` bitflags 中添加：

```rust
/// 软件保留位——MappedPages 所有权标记。
/// 硬件忽略此位（Arm ARM §D8.3，bit 55 在未启用 DBM 时为软件可用）。
const CLAIMED = 1 << 55;
```

在 `impl PteFlagsOps for PteFlags` 中添加：

```rust
#[inline]
fn is_claimed(self) -> bool {
    self.contains(Self::CLAIMED)
}

#[inline]
fn with_claimed(self, claimed: bool) -> Self {
    if claimed {
        self | Self::CLAIMED
    } else {
        self.difference(Self::CLAIMED)
    }
}
```

注意：AArch64 的 `FLAGS_MASK` 由 `PteFlags::all().bits()` 自动计算，添加 CLAIMED 后自动包含 bit 55。

- [ ] **Step 4: 验证所有 preset 不包含 CLAIMED 位**

检查所有工厂方法（`kernel_rw`、`kernel_ro`、`kernel_rwx`、`kernel_device`、`user_*`）的返回值都不包含 `CLAIMED` 位。当前实现中无一使用 bit 8（RISC-V）或 bit 55（AArch64），天然满足。

- [ ] **Step 5: 编译验证**

```bash
cargo clippy -p page_table_entry --target riscv64gc-unknown-none-elf -- -D warnings
```

Expected: 通过，无警告。

- [ ] **Step 6: 提交**

```bash
git add crates/page_table_entry/src/lib.rs crates/page_table_entry/src/riscv64.rs crates/page_table_entry/src/aarch64.rs
git commit --signoff -m "feat(page_table_entry): 添加 CLAIMED 软件位用于 MappedPages 所有权追踪

RISC-V 使用 RSW[0] (bit 8)，AArch64 使用 bit 55。
硬件忽略这些位，仅供内核软件使用。"
```

---

### Task 2: config 层——添加 poison 常量

**Files:**
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: 添加 FREED_PAGE_POISON 常量**

在 `crates/config/src/lib.rs` 的常量列表中（`PHYS_OFFSET` 之后）添加：

```rust
/// 释放页面的 poison 填充字节——帮助检测 use-after-free。
///
/// MappedPages 释放所有权时用此值填充页面内容。
/// 0xFE 与 Linux SLAB 的 freed 标记一致。
pub const FREED_PAGE_POISON: u8 = 0xFE;
```

- [ ] **Step 2: 编译验证**

```bash
cargo clippy -p config -- -D warnings
```

- [ ] **Step 3: 提交**

```bash
git add crates/config/src/lib.rs
git commit --signoff -m "feat(config): 添加 FREED_PAGE_POISON 常量 (0xFE)"
```

---

### Task 3: PageTable 层——收紧可见性 + 删除大页

**Files:**
- Modify: `crates/paging/src/table.rs`
- Modify: `crates/paging/src/error.rs`

- [ ] **Step 1: map_page / unmap_page / update_flags 改为 pub(crate)**

在 `crates/paging/src/table.rs` 中：

`map_page` (line 176): `pub fn map_page` → `pub(crate) fn map_page`

`unmap_page` (line 247): `pub fn unmap_page` → `pub(crate) fn unmap_page`

`unmap_at_level_with_flags` (line 262): `pub fn unmap_at_level_with_flags` → `fn unmap_at_level_with_flags`（仅 `unmap_page` 调用它）

`update_flags` (line 318): `pub fn update_flags` → `pub(crate) fn update_flags`

`map_at_level` (line 198): `pub fn map_at_level` → `fn map_at_level`（仅 `map_page` 和 `identity_map_range` 调用它）

- [ ] **Step 2: identity_map_range 简化为 4KB 逐页映射**

替换 `crates/paging/src/table.rs` 中 `identity_map_range` 的实现：

```rust
/// 将 `[start, end)` 物理地址区间 identity-map（VA == PA），统一使用 4KB 页。
///
/// 映射失败时直接 panic——内核启动阶段的 identity map 失败不可恢复。
///
/// # Panics
///
/// `start >= end` 或映射冲突时 panic。
pub fn identity_map_range(&mut self, start: PhysAddr, end: PhysAddr, flags: PteFlags) {
    let mut addr = start.align_down();
    let end_aligned = end.align_up();

    assert!(
        addr.as_usize() < end_aligned.as_usize(),
        "identity_map_range: 无效地址范围 [{addr}, {end_aligned})"
    );

    while addr.as_usize() < end_aligned.as_usize() {
        let va = VirtAddr::new(addr.as_usize());
        match self.map_page(va, addr, flags) {
            Ok(()) => {}
            // 幂等：同一页已被相同 PA+flags 映射，跳过
            Err(PagingError::AlreadyMappedIdentical) => {}
            Err(e) => panic!("identity_map_range: 映射 {va} 失败: {e}"),
        }
        addr += config::PAGE_SIZE;
    }
}
```

- [ ] **Step 3: 删除 HugePageConflict 错误变体**

在 `crates/paging/src/error.rs` 中：

删除 `HugePageConflict` 变体及其 Display 分支。

在 `crates/paging/src/table.rs` 中 `walk_create` 的 `pte.is_leaf(level)` 分支：

```rust
} else if pte.is_leaf(level) {
    return Err(PagingError::HugePageConflict);
}
```

改为（因为 `map_at_level` 现在只在 level=0 调用，中间层的叶节点理论上不会出现，但保留防御检查）：

```rust
} else if pte.is_leaf(level) {
    panic!(
        "walk_create: 中间层 level {} 遇到叶 PTE (va={va}, paddr={paddr})",
        level
    );
}
```

同步删除 `crates/memory/src/error.rs` 中的 `HugePageConflict` 变体和对应的 `From<PagingError>` 分支。

- [ ] **Step 4: 编译验证**

```bash
cargo clippy -p paging --target riscv64gc-unknown-none-elf -- -D warnings
```

Expected: 可能有 `dead_code` 警告（`map_at_level` 等），根据情况处理。

- [ ] **Step 5: 提交**

```bash
git add crates/paging/src/table.rs crates/paging/src/error.rs crates/memory/src/error.rs
git commit --signoff -m "refactor(paging): 收紧 PageTable 可见性，删除大页映射路径

- map_page/unmap_page/update_flags → pub(crate)
- map_at_level/unmap_at_level_with_flags → 私有
- identity_map_range 简化为统一 4KB 逐页映射
- 删除 HugePageConflict 错误变体"
```

---

### Task 4: MappedPages 重写——claim/release 语义

**Files:**
- Modify: `crates/paging/src/mapping.rs`
- Modify: `crates/paging/src/lib.rs`
- Modify: `crates/paging/src/error.rs`

- [ ] **Step 1: 添加 PageAlreadyClaimed 错误变体**

在 `crates/paging/src/error.rs` 中添加：

```rust
/// 页面已被另一个 MappedPages 声明所有权（CLAIMED 位已设置）
PageAlreadyClaimed,
```

以及对应的 Display 分支：

```rust
Self::PageAlreadyClaimed => write!(f, "page already claimed by another MappedPages"),
```

在 `crates/memory/src/error.rs` 中添加对应变体和 From 转换。

- [ ] **Step 2: 重写 MappedPages——claim/release 语义**

将 `crates/paging/src/mapping.rs` 完整替换为：

```rust
//! 仿射类型所有权追踪——move-only 的物理帧所有权管理。
//!
//! SAS 架构下全部物理内存永久 identity-mapped（PTE 永不删除）。
//! [`MappedPages`] 通过 PTE 的 CLAIMED 软件位追踪帧所有权：
//!
//! - [`claim`]：设置 CLAIMED 位 + 可选修改权限
//! - [`release`] / Drop：写 poison pattern + 清除 CLAIMED 位 + 恢复 `kernel_rw`
//!
//! CLAIMED 位实现了运行时所有权检测——双重声明立即 panic。

use core::mem::ManuallyDrop;

use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, MappedFrames, UnmappedFrames};
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// release 分块大小——每次持锁处理的最大页数，避免长时间关中断。
const RELEASE_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 仿射类型所有权——持有物理帧所有权，VA 从 PA 推导。
///
/// 不可 Clone、不可 Copy。SAS 架构下 PTE 永久存在（identity mapping），
/// 此类型仅追踪所有权和权限，不创建/删除 PTE。
///
/// Drop 时写入 poison pattern、恢复默认权限（`kernel_rw`）并归还帧。
pub struct MappedPages {
    /// `ManuallyDrop` 阻止 `MappedFrames::Drop`（会 panic）在 `MappedPages::Drop` 前运行。
    /// Drop 中手动 take + `into_unmapped()` 安全归还。
    frames: ManuallyDrop<MappedFrames>,
    flags: PteFlags,
}

impl MappedPages {
    /// 声明物理帧的所有权——设置 CLAIMED 位，可选修改权限。
    ///
    /// PTE 必须已存在（由 init 阶段的 `identity_map_range` 建立）。
    /// 若 CLAIMED 位已设置，panic（双重所有权）。
    ///
    /// # Panics
    ///
    /// - `frames` 为空（页数 == 0）
    /// - 任一页的 PTE 不存在（init 未正确映射）
    /// - 任一页的 CLAIMED 位已设置（双重所有权）
    pub fn claim(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "MappedPages::claim: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let pa_start = frames.start_paddr();
        let va_start = pa_start.to_virt();

        let claimed_flags = flags.with_claimed(true);
        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * PAGE_SIZE;
            let old_flags = guard
                .update_flags(va, claimed_flags)
                .unwrap_or_else(|e| {
                    panic!(
                        "MappedPages::claim: PTE 不存在 (va={va}, pa={}): {e}——\
                         init 阶段 identity_map_range 未覆盖此地址",
                        pa_start + i * PAGE_SIZE
                    )
                });
            assert!(
                !old_flags.is_claimed(),
                "MappedPages::claim: 双重所有权——页 {va} 的 CLAIMED 位已设置 \
                 (old_flags={old_flags:?})"
            );
        }
        drop(guard);

        // 权限变更需要 TLB flush
        {
            let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
        }

        let mapped_frames = frames.into_mapped();

        Self {
            frames: ManuallyDrop::new(mapped_frames),
            flags,
        }
    }

    /// 包装已由 init 映射且已设置 CLAIMED 的帧——仅做 typestate 转换。
    ///
    /// 用于 init 阶段：PageTable 已直接建立映射并设置 CLAIMED，
    /// 此方法只转换 `AllocatedFrames → MappedFrames` 并记录 flags。
    ///
    /// # Safety
    ///
    /// 调用方必须保证：
    /// - 帧对应的 PTE 已存在且 CLAIMED 位已设置
    /// - flags 与 PTE 中的实际权限一致
    pub unsafe fn from_claimed(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let mapped_frames = frames.into_mapped();
        Self {
            frames: ManuallyDrop::new(mapped_frames),
            flags,
        }
    }

    /// 返回起始虚拟地址（从 PA 推导）。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.frames.start_paddr().to_virt()
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.frames.count() * PAGE_SIZE
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frames.count()
    }

    /// 返回构造时的请求权限（不含 CLAIMED 位）。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 返回持有的物理帧引用（Mapped 状态）。
    #[must_use]
    pub fn frames(&self) -> &MappedFrames {
        &self.frames
    }

    /// 读取指定偏移所在页的实际 PTE 标志（含 CLAIMED 位）。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr() + offset).align_down();
        let guard = crate::kernel_page_table().lock();
        guard
            .get_mapping(page_va)
            .expect("MappedPages::pte_flags: 映射不存在")
            .1
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用生命周期绑定到 `&self`——编译器保证所有权释放后无法使用。
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证所有权存活
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type_mut",
        );
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "MappedPages::as_type_mut: PTE 无 WRITE 权限"
        );
        // SAFETY: 偏移和对齐已验证，PTE 可写已验证，&mut self 保证独占
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 修改权限——遍历 PTE 更新标志位（保留 CLAIMED），刷新 TLB。
    pub fn mprotect(&mut self, new_flags: PteFlags) {
        let pt = crate::kernel_page_table();
        let claimed_flags = new_flags.with_claimed(true);
        let mut guard = pt.lock();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            guard
                .update_flags(va, claimed_flags)
                .expect("mprotect: update_flags 失败");
        }
        drop(guard);
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        }
        self.flags = new_flags;
    }

    /// 手动释放所有权——写 poison、恢复默认权限、清 CLAIMED、归还帧。
    pub fn release(self) -> UnmappedFrames {
        let mut md = ManuallyDrop::new(self);
        // SAFETY: md 不会 Drop，手动接管字段所有权
        let mapped_frames = unsafe { ManuallyDrop::take(&mut md.frames) };
        release_frames_chunked(&mapped_frames, md.flags, "MappedPages::release");
        mapped_frames.into_unmapped()
    }
}

/// 分块释放——写 poison + 恢复权限 + 清 CLAIMED，每次持锁最多处理 RELEASE_CHUNK 页。
fn release_frames_chunked(frames: &MappedFrames, flags: PteFlags, caller: &str) {
    let page_count = frames.count();
    let va_start = frames.start_paddr().to_virt();
    let pt = crate::kernel_page_table();
    let default_flags = PteFlags::kernel_rw(); // 不含 CLAIMED

    let mut offset = 0;
    while offset < page_count {
        let n = (page_count - offset).min(RELEASE_CHUNK);

        // 1. 写 poison pattern（不需要持锁）
        for i in 0..n {
            let va = va_start + (offset + i) * PAGE_SIZE;
            // SAFETY: 帧仍在 MappedFrames 所有权内，PTE 有效（CLAIMED=1）
            unsafe {
                core::ptr::write_bytes(va.as_mut_ptr::<u8>(), config::FREED_PAGE_POISON, PAGE_SIZE);
            }
        }

        // 2. 恢复默认权限 + 清 CLAIMED（需要持锁）
        {
            let mut guard = pt.lock();
            for i in 0..n {
                let va = va_start + (offset + i) * PAGE_SIZE;
                let old_flags = guard.update_flags(va, default_flags).unwrap_or_else(|e| {
                    panic!("{caller}: 恢复权限失败 (va={va}): {e}");
                });
                assert!(
                    old_flags.is_claimed(),
                    "{caller}: CLAIMED 状态不一致——页 {va} 的 CLAIMED 位未设置 \
                     (flags={old_flags:?})，可能有外部代码篡改了 PTE"
                );
            }
        }

        // 3. TLB flush（权限变更需要刷新）
        if flags != PteFlags::kernel_rw() {
            let flush_va = va_start + offset * PAGE_SIZE;
            let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
        }

        offset += n;
    }
}

/// 验证偏移在范围内且地址对齐到 `T` 的自然边界，返回目标指针。
pub(crate) fn check_bounds_and_align<T>(
    base: usize,
    size: usize,
    offset: usize,
    fn_name: &str,
) -> *const T {
    let type_size = core::mem::size_of::<T>();
    assert!(
        type_size > 0,
        "{fn_name}: 不支持 ZST（size_of::<T>() == 0）"
    );
    assert!(
        type_size <= size && offset <= size - type_size,
        "{fn_name}: offset {:#x} + {type_size} 超出大小 {:#x}",
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

impl Drop for MappedPages {
    fn drop(&mut self) {
        release_frames_chunked(&self.frames, self.flags, "MappedPages::drop");
        // SAFETY: Drop 执行中，self.frames 不会再被访问。
        // 将 MappedFrames 转换为 UnmappedFrames，后者的 Drop 安全归还 buddy。
        let mapped_frames = unsafe { ManuallyDrop::take(&mut self.frames) };
        let _unmapped = mapped_frames.into_unmapped();
    }
}

impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MappedPages({}, {} pages, {:?})",
            self.vaddr(),
            self.page_count(),
            self.flags,
        )
    }
}
```

- [ ] **Step 3: 更新 paging crate 入口**

在 `crates/paging/src/lib.rs` 中：

更新模块文档注释，将"映射所有权"改为"帧所有权追踪"。

更新 `pub use mapping::MappedPages;` 保持不变（类型名不变）。

- [ ] **Step 4: 编译验证**

```bash
cargo clippy -p paging --target riscv64gc-unknown-none-elf -- -D warnings
```

- [ ] **Step 5: 提交**

```bash
git add crates/paging/src/mapping.rs crates/paging/src/lib.rs crates/paging/src/error.rs crates/memory/src/error.rs
git commit --signoff -m "refactor(paging): MappedPages 改为 claim/release 语义

- map → claim：设置 CLAIMED 位 + 可选修改权限（不创建 PTE）
- unmap → release：写 poison (0xFE) + 恢复 kernel_rw + 清 CLAIMED（不删 PTE）
- 新增 from_claimed：init 阶段用，跳过 CLAIMED 检查
- drop 时写 poison pattern 帮助检测 use-after-free
- 双重声明（CLAIMED 已设置）立即 panic"
```

---

### Task 5: init.rs 重写——消除双重所有权

**Files:**
- Modify: `crates/memory/src/init.rs`

- [ ] **Step 1: 重写 init 函数**

```rust
//! 内存子系统初始化——主核 / 从核。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 初始化顺序：堆 → 帧分配器 → 页表 → identity mapping → 所有权声明。
///
/// SAS 架构下全部物理内存在此阶段完成 identity mapping（PTE 永不删除）。
/// 内核段帧通过 `MappedPages::from_claimed` 声明所有权，free memory 帧
/// 由 buddy allocator 管理，使用时通过 `MappedPages::claim` 声明。
pub fn init() -> AddressSpace {
    // SAFETY: 在任何堆分配之前调用，且仅调用一次（由启动流程保证）
    unsafe { heap_crate::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();
    let mem_end = mem_start + mem_size;
    let kernel_end_aligned = kernel_end.align_up();

    // 计算各段页数——仅覆盖内核镜像，不包含 free memory
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (kernel_end_aligned - rodata_end) / config::PAGE_SIZE;

    // 空闲内存 = 内核镜像之后的部分，交由 buddy allocator 管理
    let free_start = kernel_end_aligned;
    let free_size = mem_size - (free_start - mem_start);

    // SAFETY: 范围有效、页对齐、互不重叠、仅调用一次
    let mut reserved = unsafe {
        frame_allocator::init(
            free_start,
            free_size,
            &[
                (mem_start, text_pages),
                (text_end, rodata_pages),
                (rodata_end, data_pages),
            ],
        )
    };

    // 创建页表并建立全部物理内存的 identity mapping。
    // 分段设置权限：.text(RWX)、.rodata(RO)、其余(RW)。
    // 内核段的 CLAIMED 位在此设置——表示这些帧由 MappedPages 持有。
    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    {
        let mut guard = paging::kernel_page_table().lock();
        // .text 段：RWX + CLAIMED
        guard.identity_map_range(
            mem_start,
            text_end,
            PteFlags::kernel_rwx().with_claimed(true),
        );
        // .rodata 段：RO + CLAIMED
        guard.identity_map_range(
            text_end,
            rodata_end,
            PteFlags::kernel_ro().with_claimed(true),
        );
        // .data+.bss 段：RW + CLAIMED
        guard.identity_map_range(
            rodata_end,
            kernel_end_aligned,
            PteFlags::kernel_rw().with_claimed(true),
        );
        // free memory：RW（无 CLAIMED——由 buddy 管理，使用时通过 claim 声明）
        guard.identity_map_range(free_start, mem_end, PteFlags::kernel_rw());
    }

    let mut kernel_as = AddressSpace::new();

    // 包装 reserved frames 为 MappedPages——仅做 typestate 转换，
    // PTE 和 CLAIMED 位已在上面的 identity_map_range 中设置。
    let segments: [PteFlags; 3] = [
        PteFlags::kernel_rwx(),
        PteFlags::kernel_ro(),
        PteFlags::kernel_rw(),
    ];
    let seg_starts = [mem_start, text_end, rodata_end];

    for (i, flags) in segments.into_iter().enumerate() {
        let frames = reserved.remove(0);
        let va = memory_types::VirtAddr::new(seg_starts[i].as_usize());
        // SAFETY: PTE 已由 identity_map_range 建立且 CLAIMED 位已设置
        let mapping = unsafe { paging::MappedPages::from_claimed(frames, flags) };
        kernel_as.register_kernel_mapping(va, mapping);
    }

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW), free {}-{} (RW)",
        mem_start,
        text_end,
        text_end,
        rodata_end,
        rodata_end,
        kernel_end_aligned,
        free_start,
        mem_end
    );

    kernel_as
}

/// 从核内存初始化——复用主核页表并激活分页。
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let guard = paging::kernel_page_table().lock();
    activate(&guard);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo build --target riscv64gc-unknown-none-elf 2>&1 | grep "^error" || echo "BUILD OK"
```

- [ ] **Step 3: 提交**

```bash
git add crates/memory/src/init.rs
git commit --signoff -m "fix(memory): 消除 init reserved/free 帧双重所有权

- data_pages 仅覆盖 rodata_end..kernel_end（内核 .data+.bss）
- free memory 单独 identity_map_range（无 CLAIMED）
- reserved 和 free 范围不再重叠
- 内核段 PTE 由 identity_map_range 直接建立（含 CLAIMED）
- MappedPages::from_claimed 包装 reserved frames（纯 typestate 转换）"
```

---

### Task 6: VMA 层适配

**Files:**
- Modify: `crates/memory/src/vma.rs`

- [ ] **Step 1: 更新 mmap 和 handle_page_fault 中的 MappedPages 调用**

在 `crates/memory/src/vma.rs` 中：

将 `MappedPages::map(frames, flags)` 替换为 `MappedPages::claim(frames, flags)`。

涉及两处：
- `mmap` 方法中 (line 138): `let mapping = MappedPages::map(frames, flags);` → `let mapping = MappedPages::claim(frames, flags);`
- `handle_page_fault` 方法中 (line 211): `let mapping = MappedPages::map(frames, vma.flags);` → `let mapping = MappedPages::claim(frames, vma.flags);`

- [ ] **Step 2: 编译验证**

```bash
cargo build --target riscv64gc-unknown-none-elf 2>&1 | grep "^error" || echo "BUILD OK"
```

- [ ] **Step 3: 提交**

```bash
git add crates/memory/src/vma.rs
git commit --signoff -m "refactor(memory): VMA 层适配 MappedPages::claim API"
```

---

### Task 7: 外部调用方适配

**Files:**
- Modify: `src/device/hal.rs`（DMA 分配不经过 MappedPages，不需改动——确认）
- Modify: `crates/memory/src/lib.rs`（map_mmio 可能需要适配）

- [ ] **Step 1: 检查 hal.rs**

`hal.rs` 使用 `AllocatedFrames::alloc` + `DMA_TRACKER`，不经过 MappedPages。无需修改。

- [ ] **Step 2: 检查 map_mmio**

`crates/memory/src/lib.rs` 中的 `map_mmio` 调用 `MmioRegion::map`，后者调用 `identity_map_range`。MmioRegion 在 paging crate 内部，使用 `pub(crate)` 的 `identity_map_range`——但 `identity_map_range` 是 `pub` 方法在 `PageTable` 上，MmioRegion 通过 `kernel_page_table().lock()` 访问。

确认 MmioRegion::map 仍然可以正常工作（MMIO 地址不在 buddy 中，不涉及 CLAIMED）。

- [ ] **Step 3: 全局编译验证**

```bash
cargo build --target riscv64gc-unknown-none-elf 2>&1 | tail -5
```

- [ ] **Step 4: 提交（如有改动）**

---

### Task 8: 测试更新

**Files:**
- Modify: `tests/paging-test/src/mapping.rs`

- [ ] **Step 1: 重写映射测试**

```rust
//! 仿射类型所有权测试——验证 MappedPages 的 claim/release/mprotect 行为。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use paging::{MappedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_claim_basic();
    log::info!("test claim_basic ... ok");

    test_claim_multi_page();
    log::info!("test claim_multi_page ... ok");

    test_release_clears_claimed();
    log::info!("test release_clears_claimed ... ok");

    test_drop_writes_poison();
    log::info!("test drop_writes_poison ... ok");

    test_mprotect_preserves_claimed();
    log::info!("test mprotect_preserves_claimed ... ok");

    test_release_returns_unmapped_frames();
    log::info!("test release_returns_unmapped_frames ... ok");

    test_reclaim_after_release();
    log::info!("test reclaim_after_release ... ok");

    log::info!("paging-mapping-test: all 7 tests passed");
}

/// claim 基本功能——CLAIMED 位被设置，权限正确。
fn test_claim_basic() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    assert_eq!(mp.vaddr(), pa.to_virt());
    assert_eq!(mp.size(), config::PAGE_SIZE);

    // PTE 应包含 CLAIMED 位
    let guard = paging::kernel_page_table().lock();
    let (got_pa, flags) = guard.get_mapping(pa.to_virt()).expect("映射应存在");
    assert_eq!(got_pa, pa);
    assert!(flags.is_claimed());
    assert!(flags.is_writable());
}

/// 多页 claim。
fn test_claim_multi_page() {
    let frames = AllocatedFrames::alloc(3).expect("alloc frames");
    let pa_start = frames.start_paddr();
    let _mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let guard = paging::kernel_page_table().lock();
    for i in 0..3 {
        let pa = pa_start + i * config::PAGE_SIZE;
        let (_, flags) = guard.get_mapping(pa.to_virt()).expect("映射应存在");
        assert!(flags.is_claimed());
    }
}

/// release 清除 CLAIMED 位并恢复默认权限。
fn test_release_clears_claimed() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_ro());

    // claim 后 CLAIMED=1，权限=RO
    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_claimed());
        assert!(!flags.is_writable());
    }

    let _unmapped = mp.release();

    // release 后 CLAIMED=0，权限恢复为 kernel_rw
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应仍存在（SAS 不删 PTE）");
    assert!(!flags.is_claimed());
    assert!(flags.is_writable());
}

/// drop 后页面内容被填充 poison pattern。
fn test_drop_writes_poison() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    // 写入已知数据
    // SAFETY: 页已 claimed 且可写
    unsafe {
        core::ptr::write_bytes(va.as_mut_ptr::<u8>(), 0x42, config::PAGE_SIZE);
    }

    drop(mp);

    // drop 后内容应为 poison pattern
    // SAFETY: SAS 下 PTE 仍有效（只是 CLAIMED 被清除了），读取是安全的
    let first_byte = unsafe { *(va.as_usize() as *const u8) };
    assert_eq!(
        first_byte,
        config::FREED_PAGE_POISON,
        "drop 后页面未被 poison 填充"
    );
}

/// mprotect 保留 CLAIMED 位。
fn test_mprotect_preserves_claimed() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mut mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    mp.mprotect(PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应存在");
    assert!(flags.is_claimed(), "mprotect 不应清除 CLAIMED 位");
    assert!(!flags.is_writable());
}

/// release 返回 UnmappedFrames，可用于重新分配。
fn test_release_returns_unmapped_frames() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());

    let unmapped = mp.release();
    assert_eq!(unmapped.start_paddr(), pa);
}

/// release 后同一帧可被再次 claim。
fn test_reclaim_after_release() {
    // 分配 + claim + release
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let mp = MappedPages::claim(frames, PteFlags::kernel_rw());
    let _unmapped = mp.release();
    // unmapped 的 Drop 将帧归还 buddy

    // 再次分配（buddy 可能返回同一帧）+ claim 应成功
    let frames2 = AllocatedFrames::alloc(1).expect("alloc frames 2");
    let _mp2 = MappedPages::claim(frames2, PteFlags::kernel_rw());
    // 如果 CLAIMED 位未正确清除，此处会 panic
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target riscv64gc-unknown-none-elf -p paging-test --bin mapping 2>&1 | grep "^error" || echo "BUILD OK"
```

- [ ] **Step 3: 运行测试**

```bash
cargo xtask test --arch riscv64 --name paging-test/mapping
```

Expected: 全部 7 个测试通过，无 panic。

- [ ] **Step 4: 运行 double-claim panic 测试**

在 `tests/paging-test/` 中创建 `src/double_claim_panic.rs`（should_panic 测试）：

```rust
//! 双重 claim 应 panic。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use frame_allocator::AllocatedFrames;
use memory_types::{Frame, FrameSpan};
use paging::{MappedPages, PteFlags, PteFlagsOps};

test_harness::test_main!(
    simplekernel::boot::InitLevel::Full,
    run_test,
    should_panic
);

/// 用 unsafe 构造指向同一物理帧的第二个 AllocatedFrames，
/// 触发 double claim → 应 panic。
fn run_test() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let range = frames.range();
    let _mp1 = MappedPages::claim(frames, PteFlags::kernel_rw());

    // SAFETY: 故意构造重复帧以触发 CLAIMED panic（仅用于测试）
    let dup_frames = AllocatedFrames::from_range(range);
    let _mp2 = MappedPages::claim(dup_frames, PteFlags::kernel_rw()); // 应 panic
}
```

在 `tests/paging-test/Cargo.toml` 中添加：

```toml
[[bin]]
name = "double-claim-panic"
path = "src/double_claim_panic.rs"
test = false
```

运行：

```bash
cargo xtask test --arch riscv64 --name paging-test/double-claim-panic
```

Expected: panic 发生，测试通过（should_panic）。

- [ ] **Step 5: 运行全部测试**

```bash
cargo xtask test --arch riscv64
```

Expected: 全部测试通过。

- [ ] **Step 6: 提交**

```bash
git add tests/paging-test/
git commit --signoff -m "test(paging): 适配 claim/release 语义，新增 double-claim-panic 测试"
```

---

### Task 9: 文档更新

**Files:**
- Modify: `docs/design/memory-subsystem.md`
- Modify: `docs/audit/audit-progress.md`

- [ ] **Step 1: 更新内存子系统设计文档**

在 `docs/design/memory-subsystem.md` 中更新相关章节，反映：
- MappedPages 语义变化：map/unmap → claim/release
- PTE CLAIMED 位说明
- init 流程变化：data_pages 仅覆盖内核段
- SAS 下 PTE 永不删除的设计原则
- poison pattern (0xFE) 说明

- [ ] **Step 2: 更新审计进度**

在 `docs/audit/audit-progress.md` 中添加记录。

- [ ] **Step 3: 提交**

```bash
git add docs/
git commit --signoff -m "docs: 更新内存子系统设计文档，反映 claim/release + CLAIMED 位设计"
```
