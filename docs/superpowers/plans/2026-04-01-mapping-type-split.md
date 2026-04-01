# MappedPages 类型拆分实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `MappedPages` 拆分为 `MappedPagesInner`（共享基类）+ `MappedPages`（Drop unmap）+ `PermanentMapping`（Drop 空操作），同时封装 EXCLUSIVE 位、修复 unmap 失败处理。

**Architecture:** 内部结构体 `MappedPagesInner` 持有所有字段和共享方法，`MappedPages` 和 `PermanentMapping` 通过 `Deref` 委托方法调用，仅 Drop 行为不同。EXCLUSIVE 标志从 `PteFlags` 中剥离为内部 `bool` 字段，不暴露给公开 API。

**Tech Stack:** Rust nightly, `no_std`, `heapless`, `zerocopy`

---

## 文件变更总览

| 文件 | 动作 | 说明 |
|------|------|------|
| `crates/paging/src/mapping.rs` | 重构 | 拆分为 Inner + MappedPages + PermanentMapping |
| `crates/paging/src/lib.rs` | 修改:37 | 更新 re-export，新增 `PermanentMapping` |
| `crates/paging/src/mmio.rs` | 修改:21,39 | `mapping` 类型从 `MappedPages` 改为 `PermanentMapping` |
| `crates/memory/src/vma.rs` | 修改:48,185-220,310-325 | 引入内部 `Mapping` enum 存储两种映射类型 |
| `crates/memory/src/lib.rs` | 修改:33,37 | 新增 `PermanentMapping` re-export |

---

### Task 1: 重构 `MappedPagesInner` + `MappedPages` + `PermanentMapping` 核心类型

**Files:**
- Modify: `crates/paging/src/mapping.rs`

- [ ] **Step 1: 定义 `MappedPagesInner` 结构体和共享方法**

将当前 `MappedPages` 的字段和只读方法迁移到 `MappedPagesInner`。删除 `permanent: bool` 字段，改用类型区分。将 `flags` 字段语义改为"用户请求的原始 flags"（不含 EXCLUSIVE），新增 `exclusive: bool` 字段追踪帧所有权。

```rust
/// 映射的内部共享数据——`MappedPages` 和 `PermanentMapping` 通过 `Deref` 共享。
///
/// 不可直接构造——只能通过 `MappedPages` 或 `PermanentMapping` 的工厂方法创建。
pub struct MappedPagesInner {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 用户请求的原始权限（不含 EXCLUSIVE 等内部标志位）
    flags: PteFlags,
    /// EXCLUSIVE 帧所有权——drop 时是否回收物理帧
    exclusive: bool,
    /// 所属页表的 `Arc` 引用——Drop 时通过此引用 unmap。
    page_table: Arc<SpinLock<PageTable>>,
}
```

在 `MappedPagesInner` 上实现共享方法：`vaddr()`, `size()`, `flags()`, `pte_flags()`, `as_type()`, `as_type_mut()`。这些方法的实现与当前 `MappedPages` 上的完全相同。

- [ ] **Step 2: 定义 `MappedPages` 和 `PermanentMapping` 包装类型**

```rust
/// 可回收映射——Drop 时 unmap 并回收 EXCLUSIVE 帧。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
pub struct MappedPages(MappedPagesInner);

/// 永久映射——Drop 时不执行任何操作。
///
/// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
/// 不可 Clone、不可 Copy。
pub struct PermanentMapping(MappedPagesInner);
```

为两者实现 `Deref`：

```rust
impl core::ops::Deref for MappedPages {
    type Target = MappedPagesInner;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl core::ops::DerefMut for MappedPages {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl core::ops::Deref for PermanentMapping {
    type Target = MappedPagesInner;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl core::ops::DerefMut for PermanentMapping {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
```

- [ ] **Step 3: 迁移工厂方法到新类型**

`map_identity` 和 `map_alloc` 改为 `MappedPages` 的方法。`map_identity` 返回 `MappedPages`（`exclusive: false`）。`map_alloc` 返回 `MappedPages`（`exclusive: true`），`flags` 字段存储用户原始 flags（不含 EXCLUSIVE），仅在调用 `pt.map_page` 时使用 `flags.with_exclusive()`。

`into_permanent` 消耗 `MappedPages`，返回 `PermanentMapping`：

```rust
impl MappedPages {
    /// 消耗 self，转换为永久映射（drop 时不 unmap）。
    #[must_use]
    pub fn into_permanent(self) -> PermanentMapping {
        // 阻止 MappedPages 的 Drop 执行 unmap
        let inner = unsafe {
            let inner = core::ptr::read(&self.0);
            core::mem::forget(self);
            inner
        };
        PermanentMapping(inner)
    }
}
```

注意：`into_permanent` 需要 `unsafe` + `mem::forget` 来阻止 `MappedPages::drop` 执行 unmap。更安全的替代方案是在 `MappedPagesInner` 中加一个 `consumed: bool` 标记，但这增加了运行时开销。使用 `ManuallyDrop`：

```rust
impl MappedPages {
    #[must_use]
    pub fn into_permanent(self) -> PermanentMapping {
        let md = core::mem::ManuallyDrop::new(self);
        // SAFETY: self 已被 ManuallyDrop 包装，不会 double-drop。
        // 读取内部 Inner 并转移所有权到 PermanentMapping。
        let inner = unsafe { core::ptr::read(&md.0) };
        PermanentMapping(inner)
    }
}
```

`wrap_existing` 保留在 `MappedPages` 上（仅测试使用）。

- [ ] **Step 4: 修改 `unmap_and_reclaim` 的错误处理——unmap 失败改为 panic**

将 `unmap_and_reclaim` 移到 `MappedPagesInner` 上（`MappedPages::Drop` 调用），修改 `Err` 分支：

```rust
// 原来的：
Err(e) => {
    log::warn!(
        "MappedPages::unmap_and_reclaim: unmap {va} 失败: {e}，可能存在状态不一致"
    );
}

// 改为：
Err(e) => {
    panic!(
        "MappedPages::unmap_and_reclaim: unmap {va} 失败: {e}——\
         MappedPages 保证映射存在，此错误说明内核状态已损坏"
    );
}
```

- [ ] **Step 5: 实现 Drop 和 Debug**

```rust
impl Drop for MappedPages {
    fn drop(&mut self) {
        self.0.unmap_and_reclaim();
    }
}
// PermanentMapping 不实现 Drop（或实现空 Drop）——默认 Drop 不做 unmap

impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = if self.0.exclusive { "exclusive" } else { "borrowed" };
        write!(
            f,
            "MappedPages({}, {} pages, {:?}, {})",
            self.0.vaddr, self.0.page_count, self.0.flags, kind
        )
    }
}

impl core::fmt::Debug for PermanentMapping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PermanentMapping({}, {} pages, {:?})",
            self.0.vaddr, self.0.page_count, self.0.flags
        )
    }
}
```

- [ ] **Step 6: 更新测试**

关键测试变更：
- `into_permanent_marks_permanent` 改为检查返回类型是 `PermanentMapping`，Debug 输出包含 "PermanentMapping"
- `wrap_existing_ownership` 删除 `assert!(!mp.flags().is_exclusive())` 检查——EXCLUSIVE 不再通过 `flags()` 暴露
- `map_alloc_sets_exclusive` 删除 `assert!(mp.flags().is_exclusive())`——改为检查 PTE 中的 EXCLUSIVE 位（通过 `pte_flags()`）
- `map_identity_conflict_panics` 中 `_mp1` 类型从 `MappedPages` 变为 `PermanentMapping`（`.into_permanent()` 返回值）

- [ ] **Step 7: 运行测试验证**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 8: Commit**

```bash
git add crates/paging/src/mapping.rs
git commit --signoff -m "refactor(paging): 拆分 MappedPages 为 Inner + MappedPages + PermanentMapping"
```

---

### Task 2: 更新 `paging` crate 的 re-export

**Files:**
- Modify: `crates/paging/src/lib.rs:37`

- [ ] **Step 1: 新增 `PermanentMapping` 的 re-export**

```rust
// 原来的：
pub use mapping::MappedPages;

// 改为：
pub use mapping::{MappedPages, PermanentMapping};
```

- [ ] **Step 2: 运行测试验证**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/lib.rs
git commit --signoff -m "refactor(paging): re-export PermanentMapping"
```

---

### Task 3: 更新 `MmioRegion`

**Files:**
- Modify: `crates/paging/src/mmio.rs:10,21,39`

- [ ] **Step 1: 修改 `MmioRegion` 存储类型**

```rust
// 原来的 import:
use crate::mapping::{MappedPages, check_bounds_and_align};
// 改为：
use crate::mapping::{PermanentMapping, check_bounds_and_align};

// 结构体：
pub struct MmioRegion {
    mapping: PermanentMapping,  // 原来是 MappedPages
}
```

`map_to` 方法中 `.into_permanent()` 返回值现在直接是 `PermanentMapping`，无需其他修改。`base()` 和 `size()` 通过 `Deref` 仍然可用。

- [ ] **Step 2: 运行测试验证**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 3: Commit**

```bash
git add crates/paging/src/mmio.rs
git commit --signoff -m "refactor(paging): MmioRegion 使用 PermanentMapping 类型"
```

---

### Task 4: 更新 `memory` crate——VMA 存储和 re-export

**Files:**
- Modify: `crates/memory/src/vma.rs:9,48,185-220,310-325`
- Modify: `crates/memory/src/lib.rs:33`

- [ ] **Step 1: 在 `vma.rs` 中引入 `Mapping` enum**

```rust
use crate::MappedPages;
use crate::PermanentMapping;

/// VMA 内部映射存储——区分可回收和永久映射。
enum Mapping {
    /// 匿名映射——Drop 时 unmap 并回收帧
    Reclaimable(MappedPages),
    /// 永久映射——Drop 时不操作
    Permanent(PermanentMapping),
}

impl Mapping {
    /// 返回用户请求的原始权限。
    fn flags(&self) -> PteFlags {
        match self {
            Self::Reclaimable(mp) => mp.flags(),
            Self::Permanent(pm) => pm.flags(),
        }
    }
}
```

修改 `Vma` 结构体：

```rust
pub struct Vma {
    range: AddrRange<VirtAddr>,
    flags: PteFlags,
    kind: VmaKind,
    mapping: Option<Mapping>,  // 原来是 Option<MappedPages>
}
```

- [ ] **Step 2: 更新 `AddressSpace` 的工厂方法**

`mmap_anonymous`（line 185）：

```rust
let mapping = MappedPages::map_alloc(self.page_table.clone(), start, page_count, flags);
let vma = Vma {
    range,
    flags: mapping.flags(),
    kind: VmaKind::Anonymous,
    mapping: Some(Mapping::Reclaimable(mapping)),
};
```

`mmap_identity`（line 215-219）：

```rust
let mapping = {
    let pa = address::PhysAddr::new(start.as_usize());
    MappedPages::map_identity(self.page_table.clone(), pa, page_count, flags)?
        .into_permanent()
};
let vma = Vma {
    range,
    flags,
    kind: VmaKind::Identity,
    mapping: Some(Mapping::Permanent(mapping)),
};
```

`handle_page_fault`（line 309-324）：

```rust
let mapping = match vma.kind {
    VmaKind::Anonymous => {
        let mp = MappedPages::map_alloc(
            self.page_table.clone(),
            vma.range.start(),
            vma.page_count(),
            vma.flags,
        );
        Mapping::Reclaimable(mp)
    }
    VmaKind::Identity => {
        let pa = address::PhysAddr::new(vma.range.start().as_usize());
        let pm = MappedPages::map_identity(self.page_table.clone(), pa, vma.page_count(), vma.flags)?
            .into_permanent();
        Mapping::Permanent(pm)
    }
};
vma.flags = mapping.flags();
vma.mapping = Some(mapping);
```

`mmap_identity_range`（line 356-358）：

```rust
let mapping =
    MappedPages::map_identity(self.page_table.clone(), pa_start, page_count, flags)?
        .into_permanent();
// ...
mapping: Some(Mapping::Permanent(mapping)),
```

- [ ] **Step 3: 更新 `memory/src/lib.rs` re-export**

```rust
// 原来的：
pub type MappedPages = paging::MappedPages;
// 保留，并新增：
pub type PermanentMapping = paging::PermanentMapping;
```

- [ ] **Step 4: 更新 vma 测试**

`mmap_anonymous_creates_exclusive` 测试（line 505）：删除 `assert!(vma.flags().is_exclusive())`——EXCLUSIVE 不再暴露给外部 API。替换为检查 PTE 级别的 EXCLUSIVE 位：

```rust
fn mmap_anonymous_creates_exclusive() {
    crate::frame::ensure_test_init();
    let pt_ref = test_pt();
    let mut aspace = AddressSpace::new(pt_ref.clone());
    let start = VirtAddr::new(0x20_0000);
    let vma = aspace
        .mmap_anonymous(start, 2 * PAGE_SIZE, PteFlags::kernel_rw())
        .expect("mmap_anonymous 应成功");
    assert_eq!(vma.page_count(), 2);
    assert_eq!(vma.kind(), VmaKind::Anonymous);
    assert!(vma.is_mapped());
    // EXCLUSIVE 是内部实现细节，不通过 flags() 暴露
    // 验证 PTE 级别确实设置了 EXCLUSIVE 位
    let guard = pt_ref.lock();
    let (_, pte_flags) = guard.get_mapping(start).expect("应能查到映射");
    assert!(pte_flags.is_exclusive(), "匿名映射的 PTE 应设置 EXCLUSIVE 位");
}
```

- [ ] **Step 5: 运行全部测试**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p memory 2>&1 | tail -30`
Expected: 所有测试通过

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test -p paging 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 6: Commit**

```bash
git add crates/memory/src/vma.rs crates/memory/src/lib.rs
git commit --signoff -m "refactor(memory): VMA 使用 Mapping enum 适配类型拆分"
```

---

### Task 5: 最终验证

- [ ] **Step 1: 全量编译检查**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo test 2>&1 | tail -30`
Expected: 所有测试通过

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo clippy -- -D warnings 2>&1 | tail -20`
Expected: 无 warning

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo fmt --check 2>&1`
Expected: 无格式问题

- [ ] **Step 2: 裸机交叉编译验证**

Run: `cd /Users/zhihongniu/Documents/github/MRNIU/SimpleKernel && cargo xtask build --arch riscv64 2>&1 | tail -20`
Expected: 编译成功

- [ ] **Step 3: 确认 EXCLUSIVE 不再泄漏**

在整个 crate 外部搜索 `is_exclusive` 调用，确认只在 `paging` crate 内部使用：

Run: `grep -rn "is_exclusive" crates/ --include="*.rs" | grep -v "paging/" | grep -v "page_table_entry/"`
Expected: 无输出（或仅在测试中通过 PTE 查询）
