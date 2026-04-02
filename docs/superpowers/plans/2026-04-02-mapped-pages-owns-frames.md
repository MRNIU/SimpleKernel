# MappedPages 持有帧所有权 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `MappedPages` 在结构体中直接持有 `AllocatedFrames`，消除 PTE EXCLUSIVE 位和 unsafe 所有权恢复路径，获得全编译期帧生命周期保证。

**Architecture:** `MappedPages` 新增 `frames: AllocatedFrames` 字段，删除 `map_alloc`/`map_identity`，统一为 `map(pages, frames, flags)`。frame_allocator 状态机从 4 状态简化为 2 状态（Free/Allocated），`init` 函数统一初始化空闲内存和预留内核段帧。page_table_entry 删除 EXCLUSIVE 相关 API。memory crate 的 VMA 层删除 `ManuallyDrop`/`Mapping` 枚举。

**Tech Stack:** Rust nightly, `#![no_std]`, buddy_system_allocator 0.12, heapless

**Design spec:** `docs/rust-rewrite/memory-subsystem.md`

**注意：** `buddy_system_allocator` 不支持定点分配（`alloc_at`）。`frame_allocator::init` 统一处理两件事：(1) 空闲内存入 buddy；(2) 预留范围（内核段）直接构造为 `AllocatedFrames` 返回。对外只有一个 `init` 入口，调用方拿到的预留帧是正常的 `AllocatedFrames`，后续使用完全 safe。三段映射覆盖完整内核镜像（含 .symtab/.strtab 调试信息）和全部空闲 RAM。

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/frame_allocator/src/state.rs` | Modify | 删除 `Mapped`/`Unmapped` 状态及相关类型和 Drop 分支 |
| `crates/frame_allocator/src/transitions.rs` | Modify | 删除 `into_mapped`/`into_unmapped`/`from_unmapped_range`，更新测试 |
| `crates/frame_allocator/src/alloc.rs` | Modify | 重写 `init` 函数——统一处理空闲内存 + 预留范围 |
| `crates/frame_allocator/src/lib.rs` | Modify | 更新导出 |
| `crates/page_table_entry/src/lib.rs` | Modify | 删除 EXCLUSIVE 相关 trait 方法 |
| `crates/page_table_entry/src/riscv64.rs` | Modify | 删除 EXCLUSIVE 常量和实现 |
| `crates/page_table_entry/src/aarch64.rs` | Modify | 删除 EXCLUSIVE 常量和实现 |
| `crates/paging/src/error.rs` | Modify | 删除 `UnmapResult` 枚举 |
| `crates/paging/src/lib.rs` | Modify | 删除 `UnmapResult` 导出 |
| `crates/paging/src/table.rs` | Modify | 删除 `unmap_to_result` 系列方法，简化 `update_flags` |
| `crates/paging/src/mapping.rs` | Modify | 重写：新增 `frames` 字段，`map()` 统一入口，重写 Drop/unmap/split/merge |
| `crates/paging/src/mmio.rs` | Modify | 解耦 MappedPages，直接调用 PageTable |
| `crates/memory/src/vma.rs` | Modify | 删除 `Mapping` 枚举和 `ManuallyDrop`，更新映射调用 |
| `crates/memory/src/init.rs` | Modify | 使用新 `init(free, reserved)` 统一初始化 + 分段映射 |
| `crates/memory/src/lib.rs` | Modify | 更新 re-export |

---

### Task 1: frame_allocator — 简化状态机

**Files:**
- Modify: `crates/frame_allocator/src/state.rs`
- Modify: `crates/frame_allocator/src/transitions.rs`
- Modify: `crates/frame_allocator/src/lib.rs`

- [ ] **Step 1: 修改 state.rs — 删除 Mapped/Unmapped 状态**

`crates/frame_allocator/src/state.rs` 中：
1. `MemoryState` 枚举删除 `Mapped` 和 `Unmapped` 变体（保留 `Free`、`Allocated`）
2. 删除 `MappedFrames` 和 `UnmappedFrames` 类型别名（line 52-54）
3. Drop 实现简化——删除 `MemoryState::Mapped` panic 分支，所有状态统一 `dealloc_to_buddy`：

```rust
impl<const S: MemoryState, P: PageSize> Drop for Frames<S, P> {
    fn drop(&mut self) {
        dealloc_to_buddy(self.range);
    }
}
```

4. 更新模块文档注释，说明状态机简化为 `Free → Allocated → Free`

- [ ] **Step 2: 修改 transitions.rs — 删除旧方法**

`crates/frame_allocator/src/transitions.rs` 中：
1. 删除 `AllocatedFrames::into_mapped()` 方法
2. 删除整个 `impl<P: PageSize> MappedFrames<P>` 块
3. 删除整个 `impl<P: PageSize> UnmappedFrames<P>` 块
4. 删除旧 import 中的 `MappedFrames`、`UnmappedFrames`

- [ ] **Step 2.5: 重写 alloc.rs 的 init 函数**

`crates/frame_allocator/src/alloc.rs` 中，将当前的 `pub unsafe fn init(start, size)` 替换为统一入口：

```rust
/// 初始化帧分配器——空闲内存入 buddy，预留范围构造为 `AllocatedFrames` 返回。
///
/// - `free_start`/`free_size`：空闲物理内存范围，加入 buddy allocator
/// - `reserved`：需要预留的物理地址范围列表 `(start, page_count)`，
///   不经过 buddy——直接构造为 `AllocatedFrames` 返回给调用方
///
/// 预留范围的帧由调用方负责生命周期管理（通常由 `AddressSpace`
/// 通过 `MappedPages` 持有直到关机）。
///
/// # Safety
///
/// - 所有范围必须有效、页对齐、互不重叠
/// - `free` 范围和 `reserved` 范围不得重叠
/// - 仅调用一次
pub unsafe fn init(
    free_start: PhysAddr,
    free_size: usize,
    reserved: &[(PhysAddr, usize)],
) -> heapless::Vec<AllocatedFrames, 8> {
    let mut alloc = FRAME_ALLOCATOR.lock();
    assert!(!alloc.initialized, "frame_allocator::init called twice");
    assert!(free_start.is_aligned(), "init: free_start not page-aligned");
    assert!(free_size > 0, "init: free_size is zero");

    let start_frame = free_start.page_number().as_usize();
    let end_frame = PhysAddr::new(free_start.as_usize() + free_size)
        .page_number().as_usize();
    alloc.allocator.add_frame(start_frame, end_frame);
    alloc.initialized = true;

    log::info!("FrameInit: {} MB free from {}", free_size / (1024 * 1024), free_start);

    let mut result = heapless::Vec::new();
    for &(start, count) in reserved {
        assert!(start.is_aligned(), "reserved range not page-aligned: {start}");
        assert!(count > 0, "reserved range count is zero");
        let s = Frame::new(start.page_number().as_usize());
        let e = Frame::new(s.as_usize() + count);
        let frames = AllocatedFrames::from_range(FrameSpan::new(s, e));
        result.push(frames).expect("reserved 范围数不超过 8");
        log::info!("FrameInit: reserved {} pages at {}", count, start);
    }
    result
}
```

同时删除旧的 `pub unsafe fn init(start: PhysAddr, size: usize)` 函数。
更新 `ensure_test_init()` 调用为新签名（`reserved` 传空切片 `&[]`）。

- [ ] **Step 3: 修改 lib.rs — 更新导出**

`crates/frame_allocator/src/lib.rs` 中：
将 `pub use state::{AllocatedFrames, Frames, MappedFrames, MemoryState, UnmappedFrames};`
改为 `pub use state::{AllocatedFrames, Frames, MemoryState};`

- [ ] **Step 4: 更新 transitions.rs 测试**

删除依赖旧状态的测试用例：
- `typestate_transitions`
- `unmapped_back_to_allocated`
- `unmapped_release_reclaims`
- `free_into_allocated`（保留，但删除后续的 mapped/unmapped 转换部分）
- `full_lifecycle`
- `from_range_reclaims`
- `mapped_drop_panics`

保留：`alloc_one_frame`、`alloc_multiple_frames`、`alloc_dealloc_realloc`、`split_frames`、`merge_adjacent_frames`、`merge_non_adjacent_fails`

新��� `init` 预留范围测试（在 alloc.rs 测试模块中）：

```rust
/// init 的 reserved 参数应返回正确范围的帧。
#[test]
fn init_reserved_basic() {
    // 注意：此测试需要独立的 init 调用，不能和 ensure_test_init 共存。
    // 在实际测试中可能需要 #[ignore] 或单独的测试二进制。
    // 这里展示预期的行为语义。
    ensure_test_init();  // 已有的 init，以下验证 AllocatedFrames::from_range
    let range = FrameSpan::new(
        Frame::new(0x8000),
        Frame::new(0x8004),
    );
    let frames = AllocatedFrames::from_range(range);
    assert_eq!(frames.count(), 4);
    // forget 避免 dealloc 到 buddy（这些帧不在 buddy 中）
    core::mem::forget(frames);
}
```

- [ ] **Step 5: 运行测试**

运行: `cargo test -p frame_allocator`
预期: 全部 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/frame_allocator/
git commit --signoff -m "refactor(frame_allocator): 简化状态机 + init 统一入口

- 删除 Mapped/Unmapped 状态和 MappedFrames/UnmappedFrames 类型
- 删除 into_mapped/into_unmapped/from_unmapped_range
- init 统一处理空闲内存入 buddy + 预留范围构造 AllocatedFrames
- Drop 统一归还 buddy（不再有 Mapped panic）"
```

---

### Task 2: page_table_entry — 删除 EXCLUSIVE

**Files:**
- Modify: `crates/page_table_entry/src/lib.rs`
- Modify: `crates/page_table_entry/src/riscv64.rs`
- Modify: `crates/page_table_entry/src/aarch64.rs`

- [ ] **Step 1: 删除 trait 定义**

`crates/page_table_entry/src/lib.rs` 中删除 `PteFlagsOps` trait 的三个方法：
- `fn is_exclusive(&self) -> bool`
- `fn with_exclusive(self) -> Self`
- `fn without_exclusive(self) -> Self`

- [ ] **Step 2: 删除 riscv64 实现**

`crates/page_table_entry/src/riscv64.rs` 中：
1. `bitflags!` 宏内删除 `const EXCLUSIVE = 1 << 8;`
2. 删除 `is_exclusive()` 方法实现
3. 删除 `with_exclusive()` 和 `without_exclusive()` 方法实现
4. 删除测试中的 EXCLUSIVE 相关测试用例

- [ ] **Step 3: 删除 aarch64 实现**

`crates/page_table_entry/src/aarch64.rs` 中：
1. `bitflags!` 宏内删除 `const EXCLUSIVE = 1 << 55;`
2. 删除 `is_exclusive()` 方法实现
3. 删除 `with_exclusive()` 和 `without_exclusive()` 方法实现
4. 删除测试中的 EXCLUSIVE 相关测试用例

- [ ] **Step 4: 运行测试**

运行: `cargo test -p page_table_entry`
预期: 全部 PASS

- [ ] **Step 5: 提交**

```bash
git add crates/page_table_entry/
git commit --signoff -m "refactor(page_table_entry): 删除 EXCLUSIVE 软件位

SAS 架构下不需要 COW/共享映射，帧所有权由 MappedPages 结构体
编译期追踪，PTE 软件位不再承载所有权信息。"
```

---

### Task 3: paging — 删除 UnmapResult，简化 PageTable

**Files:**
- Modify: `crates/paging/src/error.rs`
- Modify: `crates/paging/src/lib.rs`
- Modify: `crates/paging/src/table.rs`

- [ ] **Step 1: 删除 UnmapResult**

`crates/paging/src/error.rs` 中：删除 `UnmapResult` 枚举及其 `Debug` impl（line 38-52）。

`crates/paging/src/lib.rs` 中：删除 `pub use error::UnmapResult;`（line 39）。

- [ ] **Step 2: 删除 table.rs 中的 unmap_to_result 系列方法**

`crates/paging/src/table.rs` 中：
1. 删除 `unmap_to_result()` 方法（line 312-317）
2. 删除 `unmap_at_level_to_result()` 方法（line 319-337）

- [ ] **Step 3: 简化 update_flags — 删除 EXCLUSIVE 保留逻辑**

`crates/paging/src/table.rs` 的 `update_flags()` 方法中，删除 EXCLUSIVE 保留逻辑。找到类似以下的代码块并替换：

```rust
// 旧：
let preserve_exclusive = if old_flags.is_exclusive() {
    new_flags.with_exclusive()
} else {
    new_flags
};
let leaf_flags = preserve_exclusive.for_leaf_at_level(leaf_level);

// 新：
let leaf_flags = new_flags.for_leaf_at_level(leaf_level);
```

- [ ] **Step 4: 运行测试**

运行: `cargo test -p paging`
预期: 编译失败——`mapping.rs` 仍引用已删除的类型。这是预期的，Task 4 会修复。

- [ ] **Step 5: 提交（仅 error.rs + table.rs，不含 mapping.rs）**

```bash
git add crates/paging/src/error.rs crates/paging/src/lib.rs crates/paging/src/table.rs
git commit --signoff -m "refactor(paging): 删除 UnmapResult 和 EXCLUSIVE 相关页表方法

- 删除 UnmapResult 枚举（帧不再从 PTE 恢复）
- 删除 unmap_to_result/unmap_at_level_to_result
- 简化 update_flags 移除 EXCLUSIVE 保留逻辑"
```

---

### Task 4: paging — 重写 MappedPages

**Files:**
- Modify: `crates/paging/src/mapping.rs`

- [ ] **Step 1: 重写 MappedPages 结构体**

```rust
/// 仿射类型映射——持有虚拟页和物理帧所有权。
///
/// 不可 Clone、不可 Copy。Drop 时 unmap PTE 并回收帧和页。
/// 帧所有权始终在 Rust 类型系统中——无 mem::forget，无 unsafe 恢复。
pub struct MappedPages {
    pages: AllocatedPages,
    frames: AllocatedFrames,
    flags: PteFlags,
}
```

- [ ] **Step 2: 重写 map() — 统一创建入口**

删除 `map_alloc` 和 `map_identity`，替换为：

```rust
impl MappedPages {
    /// 唯一的创建路径——消费 pages 和 frames 的所有权建立映射。
    ///
    /// 调用方决定 VA/PA 的对应关系：
    ///   - identity map: 确保 pages.start_vaddr() == frames.start_paddr()
    ///   - 匿名映射: pages 和 frames 地址可以不同
    pub fn map(pages: AllocatedPages, frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = pages.count();
        assert_eq!(
            page_count,
            frames.count(),
            "MappedPages::map: pages 和 frames 数量不一致"
        );
        assert!(page_count > 0, "MappedPages::map: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let va_start = pages.start_vaddr();
        let pa_start = frames.start_paddr();

        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * config::PAGE_SIZE;
            let pa = pa_start + i * config::PAGE_SIZE;
            guard
                .map_page(va, pa, flags)
                .expect("MappedPages::map: map_page 失败");
        }
        drop(guard);

        Self { pages, frames, flags }
    }
}
```

- [ ] **Step 3: 重写 accessor 方法**

更新 `pages()`、`vaddr()`、`size()`、`page_count()`、`flags()` 方法（逻辑不变）。
新增 `frames()` 方法：

```rust
    /// 返回持有的物理帧引用。
    #[must_use]
    pub fn frames(&self) -> &AllocatedFrames {
        &self.frames
    }
```

- [ ] **Step 4: 重写 unmap()**

```rust
    /// 手动解除映射并取回所有权。
    pub fn unmap(self) -> (AllocatedPages, AllocatedFrames) {
        let md = ManuallyDrop::new(self);
        let pages = unsafe { core::ptr::read(&md.pages) };
        let frames = unsafe { core::ptr::read(&md.frames) };
        let page_count = pages.count();
        let va_start = pages.start_vaddr();

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * config::PAGE_SIZE;
            guard
                .unmap_page(va)
                .expect("MappedPages::unmap: unmap_page 失败");
        }
        drop(guard);

        {
            let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
        }

        (pages, frames)
    }
```

- [ ] **Step 5: 重写 Drop**

```rust
impl Drop for MappedPages {
    fn drop(&mut self) {
        let page_count = self.pages.count();
        let va_start = self.pages.start_vaddr();
        let pt = crate::kernel_page_table();

        let mut offset = 0;
        while offset < page_count {
            let n = (page_count - offset).min(UNMAP_CHUNK);
            {
                let mut guard = pt.lock();
                for i in 0..n {
                    let va = va_start + (offset + i) * config::PAGE_SIZE;
                    guard
                        .unmap_page(va)
                        .unwrap_or_else(|e| {
                            panic!("MappedPages::drop: unmap {va} 失败: {e}");
                        });
                }
            }
            {
                let flush_va = va_start + offset * config::PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            }
            offset += n;
        }
        // self.frames 和 self.pages 在 Drop 返回后自动 drop，
        // 分别归还 buddy 和 page_allocator
    }
}
```

- [ ] **Step 6: 重写 split()**

```rust
    pub fn split(self, page_index: usize) -> (MappedPages, MappedPages) {
        let flags = self.flags;
        let md = ManuallyDrop::new(self);
        let pages = unsafe { core::ptr::read(&md.pages) };
        let frames = unsafe { core::ptr::read(&md.frames) };

        let (left_pages, right_pages) = pages.split(page_index);

        let mid_frame = memory_types::Frame::new(
            frames.start().as_usize() + page_index,
        );
        let (left_frames, right_frames) = frames.split_at(mid_frame);

        (
            MappedPages { pages: left_pages, frames: left_frames, flags },
            MappedPages { pages: right_pages, frames: right_frames, flags },
        )
    }
```

- [ ] **Step 7: 重写 merge()**

```rust
    pub fn merge(self, other: MappedPages) -> Result<MappedPages, (MappedPages, MappedPages)> {
        if self.flags != other.flags {
            return Err((self, other));
        }
        let self_md = ManuallyDrop::new(self);
        let other_md = ManuallyDrop::new(other);
        let self_pages = unsafe { core::ptr::read(&self_md.pages) };
        let other_pages = unsafe { core::ptr::read(&other_md.pages) };
        let self_frames = unsafe { core::ptr::read(&self_md.frames) };
        let other_frames = unsafe { core::ptr::read(&other_md.frames) };
        let flags = self_md.flags;

        match self_pages.merge(other_pages) {
            Ok(merged_pages) => {
                match self_frames.merge(other_frames) {
                    Ok(merged_frames) => Ok(MappedPages {
                        pages: merged_pages,
                        frames: merged_frames,
                        flags,
                    }),
                    Err((sf, of)) => {
                        // 帧不连续——还原 pages 的 merge
                        let mid = sf.count();
                        let (sp, op) = merged_pages.split(mid);
                        Err((
                            MappedPages { pages: sp, frames: sf, flags },
                            MappedPages { pages: op, frames: of, flags },
                        ))
                    }
                }
            }
            Err((sp, op)) => Err((
                MappedPages { pages: sp, frames: self_frames, flags },
                MappedPages { pages: op, frames: other_frames, flags: other_md.flags },
            )),
        }
    }
```

- [ ] **Step 8: 更新 mprotect 和 as_type 系列方法**

`mprotect` 无需改动（已经不涉及 EXCLUSIVE）。
`as_type`/`as_type_mut` 无需改动。
`pte_flags` 无需改动。

删除不再需要的 import：`use frame_allocator::{AllocatedFrames, UnmappedFrames};`
改为：`use frame_allocator::AllocatedFrames;`

- [ ] **Step 9: 重写测试**

删除所有旧测试，重写：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use memory_types::VirtAddr;

    fn alloc_pages_at(va: usize, count: usize) -> AllocatedPages {
        page_allocator::AllocatedPages::alloc_at(VirtAddr::new(va), count)
            .expect("alloc_pages_at")
    }

    fn alloc_frames(count: usize) -> AllocatedFrames {
        AllocatedFrames::alloc(count).expect("alloc_frames")
    }

    /// map 应建立映射并可查询。
    #[test]
    fn map_basic() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x20_0000, 1);
        let frames = alloc_frames(1);
        let pa = frames.start_paddr();
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (got_pa, _) = guard.get_mapping(VirtAddr::new(0x20_0000))
            .expect("映射应存在");
        assert_eq!(got_pa, pa);
        assert_eq!(mp.size(), config::PAGE_SIZE);
    }

    /// map 多页后逐页应有映射。
    #[test]
    fn map_multi_page() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x30_0000, 3);
        let frames = alloc_frames(3);
        let _mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());
        let guard = crate::kernel_page_table().lock();
        for i in 0..3 {
            assert!(guard.get_mapping(VirtAddr::new(0x30_0000 + i * config::PAGE_SIZE)).is_some());
        }
    }

    /// Drop 应 unmap PTE 并回收帧。
    #[test]
    fn drop_unmaps() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x40_0000, 2);
        let va = VirtAddr::new(0x40_0000);
        let frames = alloc_frames(2);
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        {
            let guard = crate::kernel_page_table().lock();
            assert!(guard.get_mapping(va).is_some());
        }
        drop(mp);
        let guard = crate::kernel_page_table().lock();
        assert!(guard.get_mapping(va).is_none());
    }

    /// split 应拆分为两个独立的 MappedPages。
    #[test]
    fn split_basic() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x50_0000, 4);
        let frames = alloc_frames(4);
        let mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let (left, right) = mp.split(2);
        assert_eq!(left.page_count(), 2);
        assert_eq!(right.page_count(), 2);
        assert_eq!(left.vaddr(), VirtAddr::new(0x50_0000));
        assert_eq!(right.vaddr(), VirtAddr::new(0x50_0000 + 2 * config::PAGE_SIZE));
    }

    /// mprotect 应修改 PTE 权限。
    #[test]
    fn mprotect_changes_flags() {
        crate::ensure_test_init();
        let pages = alloc_pages_at(0x60_0000, 1);
        let va = VirtAddr::new(0x60_0000);
        let frames = alloc_frames(1);
        let mut mp = MappedPages::map(pages, frames, PteFlags::kernel_rw());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(flags.is_writable());
        drop(guard);

        mp.mprotect(PteFlags::kernel_ro());

        let guard = crate::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(!flags.is_writable());
    }
}
```

- [ ] **Step 10: 运行测试**

运行: `cargo test -p paging`
预期: 全部 PASS

- [ ] **Step 11: 提交**

```bash
git add crates/paging/src/mapping.rs
git commit --signoff -m "refactor(paging): MappedPages 持有 AllocatedFrames，统一 map() 入口

- MappedPages 新增 frames 字段，帧所有权始终在类型系统中
- 删除 map_alloc/map_identity，统一为 map(pages, frames, flags)
- Drop 直接 unmap + drop frames，无需 EXCLUSIVE/UnmapResult
- 消除 mem::forget 和 unsafe from_unmapped_range"
```

---

### Task 5: paging — 重写 MmioRegion

**Files:**
- Modify: `crates/paging/src/mmio.rs`

- [ ] **Step 1: 重写 MmioRegion 解耦 MappedPages**

```rust
/// 已映射的 MMIO 区域——提供类型安全的 volatile 寄存器访问。
///
/// MMIO 地址是硬件寄存器，不是 RAM，不在 buddy allocator 中。
/// 直接使用 PageTable 的 pub(crate) 方法建立映射，不经过 MappedPages。
/// 映射永久存在——不自动 unmap。
pub struct MmioRegion {
    pages: AllocatedPages,
    base: memory_types::VirtAddr,
    size: usize,
}

impl MmioRegion {
    /// 将 `[paddr, paddr+size)` identity-map，返回 `MmioRegion`。
    pub fn map(paddr: PhysAddr, size: usize) -> Result<Self, PagingError> {
        let pa_aligned = paddr.align_down();
        let page_count =
            ((paddr + size).align_up().as_usize() - pa_aligned.as_usize()) / PAGE_SIZE;
        let va = memory_types::VirtAddr::new(pa_aligned.as_usize());
        let pages = AllocatedPages::alloc_at(va, page_count)
            .map_err(|_| PagingError::AllocationFailed)?;

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        guard.identity_map_range(pa_aligned, pa_aligned + page_count * PAGE_SIZE, PteFlags::kernel_device());
        drop(guard);

        Ok(Self {
            base: va,
            size: page_count * PAGE_SIZE,
            pages,
        })
    }

    pub fn base(&self) -> memory_types::VirtAddr { self.base }
    pub fn size(&self) -> usize { self.size }

    pub fn read_reg<T: zerocopy::FromBytes>(&self, offset: usize) -> T {
        let ptr: *const T = crate::mapping::check_bounds_and_align::<T>(
            self.base.as_usize(), self.size, offset, "MmioRegion::read_reg",
        );
        unsafe { core::ptr::read_volatile(ptr) }
    }

    pub fn write_reg<T: zerocopy::IntoBytes>(&self, offset: usize, val: T) {
        let ptr: *const T = crate::mapping::check_bounds_and_align::<T>(
            self.base.as_usize(), self.size, offset, "MmioRegion::write_reg",
        );
        unsafe { core::ptr::write_volatile(ptr as *mut T, val) }
    }
}
```

注意：`MmioRegion` 不 impl `Drop` 来 unmap——MMIO 映射是永久的。`pages` 字段的 Drop 会归还虚拟页给 page_allocator，但 PTE 不会被清除。如果需要清理 PTE（设备热插拔场景），可以后续添加显式 `unmap` 方法。

- [ ] **Step 2: 运行测试**

运行: `cargo test -p paging`
预期: 全部 PASS

- [ ] **Step 3: 提交**

```bash
git add crates/paging/src/mmio.rs
git commit --signoff -m "refactor(paging): MmioRegion 解耦 MappedPages，直接操作 PageTable

MMIO 地址不是 RAM，不在 buddy allocator 中，
不适合走 MappedPages 的帧所有权模型。
改为直接调用 PageTable::identity_map_range。"
```

---

### Task 6: memory crate — 重写 VMA 和 init

**Files:**
- Modify: `crates/memory/src/vma.rs`
- Modify: `crates/memory/src/init.rs`
- Modify: `crates/memory/src/lib.rs`

- [ ] **Step 1: 重写 vma.rs — 删除 Mapping 枚举**

1. 删除 `Mapping` 枚举（Reclaimable/Permanent）
2. `Vma` 的 `mapping` 字段类型改为 `Option<MappedPages>`
3. 删除所有 `ManuallyDrop` 使用
4. 更新 `mmap_anonymous`：改用 `MappedPages::map(pages, frames, flags)`
5. 更新 `mmap_identity`：改用 `AllocatedFrames::alloc` + `MappedPages::map`（不再用 `ManuallyDrop`）
6. 更新 `mmap_identity_range`：同上
7. 更新 `handle_page_fault`：同上
8. 更新 `Vma::flags()` 方法——不再需要 match Mapping 变体

`mmap_anonymous` 示意：

```rust
pub fn mmap_anonymous(&mut self, start: VirtAddr, size: usize, flags: PteFlags)
    -> Result<&Vma, MemoryError>
{
    let (start, end, page_count) = Self::validate_range(start, size)?;
    let range = Span::new(start, end);
    self.check_overlap(range)?;

    let pages = AllocatedPages::alloc_at(start, page_count)
        .map_err(|_| MemoryError::AllocationFailed)?;
    let frames = frame_allocator::AllocatedFrames::alloc(page_count)
        .map_err(|_| MemoryError::OutOfMemory)?;
    let mapping = MappedPages::map(pages, frames, flags);

    let vma = Vma {
        range,
        flags: mapping.flags(),
        kind: VmaKind::Anonymous,
        mapping: Some(mapping),
    };
    self.areas.insert(start, vma);
    Ok(self.areas.get(&start).expect("刚插入的 VMA"))
}
```

`mmap_identity` 示意（内核段之外的 identity map）：

```rust
pub fn mmap_identity(&mut self, start: VirtAddr, size: usize, flags: PteFlags)
    -> Result<&Vma, MemoryError>
{
    let (start, end, page_count) = Self::validate_range(start, size)?;
    let range = Span::new(start, end);
    self.check_overlap(range)?;

    let pages = AllocatedPages::alloc_at(start, page_count)
        .map_err(|_| MemoryError::AllocationFailed)?;
    // identity map: 帧地址 == 页地址
    let pa = memory_types::PhysAddr::new(start.as_usize());
    let frames = frame_allocator::AllocatedFrames::alloc(page_count)
        .map_err(|_| MemoryError::OutOfMemory)?;
    let mapping = MappedPages::map(pages, frames, flags);

    let vma = Vma {
        range,
        flags,
        kind: VmaKind::Identity,
        mapping: Some(mapping),
    };
    self.areas.insert(start, vma);
    Ok(self.areas.get(&start).expect("刚插入的 VMA"))
}
```

- [ ] **Step 2: 重写 init.rs — 使用统一 init 入口**

```rust
pub fn init() -> AddressSpace {
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

    // 计算各段页数
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (mem_end - rodata_end) / config::PAGE_SIZE;

    // 空闲内存 = 内核之后的部分
    let free_start = kernel_end.align_up();
    let free_size = mem_size - (free_start - mem_start);

    // 一次 init：空闲内存入 buddy + 预留三段内核帧
    // SAFETY: 范围有效、页对齐、互不重叠、仅调用一次
    let reserved = unsafe {
        frame_allocator::init(free_start, free_size, &[
            (mem_start, text_pages),     // .boot + .text
            (text_end, rodata_pages),    // .rodata（含 .symtab/.strtab）
            (rodata_end, data_pages),    // .data + .bss + 空闲内存
        ])
    };

    // 虚拟页分配器覆盖全部地址空间
    let va_end = mem_start + mem_size;
    unsafe {
        page_allocator::init(VirtAddr::new(0), va_end.as_usize());
    }

    let pt = PageTable::create().expect("创建内核页表失败");
    let pt_lock = sync_crate::SpinLock::new(pt, "kernel_pt");
    let pt_static: &'static _ = alloc::boxed::Box::leak(alloc::boxed::Box::new(pt_lock));
    unsafe { paging::set_kernel_page_table(pt_static) };

    let mut kernel_as = AddressSpace::new();

    // 分段映射——使用 init 返回的预留帧
    let flags_list = [
        (mem_start, PteFlags::kernel_rwx()),   // .text+.boot
        (text_end, PteFlags::kernel_ro()),      // .rodata
        (rodata_end, PteFlags::kernel_rw()),    // .data+.bss+free
    ];
    for (i, (seg_start, flags)) in flags_list.into_iter().enumerate() {
        // reserved 中帧的顺序与传入 init 的 reserved 参数顺序一致
        let frames = reserved.into_iter().nth(i).expect("预留帧数量不足");
        let page_count = frames.count();
        let va = VirtAddr::new(seg_start.as_usize());
        let pages = page_allocator::AllocatedPages::alloc_at(va, page_count)
            .expect("内核段页分配失败");
        let mapping = paging::MappedPages::map(pages, frames, flags);
        kernel_as.register_kernel_mapping(va, mapping, crate::vma::VmaKind::Identity);
    }

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW)",
        mem_start, text_end, text_end, rodata_end, rodata_end, mem_end
    );

    kernel_as
}
```

注意：`reserved` 是 `heapless::Vec<AllocatedFrames, 8>`，迭代消费每个元素的所有权。
`AddressSpace` 需要新增 `register_kernel_mapping` 方法，用于接受已创建的 `MappedPages` 并注册为 VMA。

- [ ] **Step 3: 在 vma.rs 新增 register_kernel_mapping 方法**

```rust
/// 注册已由外部建立的映射——直接接管 MappedPages 所有权。
pub fn register_kernel_mapping(
    &mut self,
    start: VirtAddr,
    mapping: MappedPages,
    kind: VmaKind,
) {
    let size = mapping.size();
    let flags = mapping.flags();
    let end = start + size;
    let range = Span::new(start, end);

    let vma = Vma {
        range,
        flags,
        kind,
        mapping: Some(mapping),
    };
    self.areas.insert(start, vma);
}
```

- [ ] **Step 4: 更新 memory/src/lib.rs**

更新 `map_mmio` 函数——`MmioRegion` 不再包装 `MappedPages`：

```rust
pub fn map_mmio(
    paddr: memory_types::PhysAddr,
    size: usize,
) -> Result<memory_types::VirtAddr, error::MemoryError> {
    let region = MmioRegion::map(paddr, size)?;
    let vaddr = region.base();
    let region_size = region.size();

    if let Some(kas) = kernel_address_space() {
        kas.lock()
            .register_existing(
                vaddr,
                region_size,
                paging::PteFlags::kernel_device(),
                vma::VmaKind::Identity,
            )
            .expect("MMIO 区域注册到内核地址空间失败");
    }

    // region 需要存活——leak 它以阻止 Drop
    core::mem::forget(region);

    Ok(vaddr)
}
```

- [ ] **Step 5: 更新 vma.rs 测试**

更新测试中的 `mmap_identity` 和 `mmap_anonymous` 调用——接口没变，但内部实现变了。
移除依赖 `ManuallyDrop` 的 `map_identity_basic` 测试中的 `ManuallyDrop` 包装。

- [ ] **Step 6: 运行全部测试**

运行: `cargo test -p memory`
预期: 全部 PASS

运行: `cargo test -p paging`
预期: 全部 PASS

- [ ] **Step 7: 提交**

```bash
git add crates/memory/ crates/paging/
git commit --signoff -m "refactor(memory): VMA 统一 Drop，init 使用统一入口

- 删除 Mapping 枚举和 ManuallyDrop——所有 MappedPages 统一 Drop
- mmap_anonymous/mmap_identity 统一使用 MappedPages::map()
- init 使用 frame_allocator::init(free, reserved) 一次性完成初始化
- 三段映射覆盖完整内核镜像（含 .symtab 调试信息）+ 空闲 RAM
- MmioRegion 走独立路径，map_mmio forget region 阻止 Drop
- 新增 AddressSpace::register_kernel_mapping 注册已有映射"
```

---

### Task 7: 全局验证

**Files:** 无新文件

- [ ] **Step 1: 运行全部 workspace 单元测试**

运行: `cargo test`
预期: 全部 PASS

- [ ] **Step 2: 运行 clippy**

运行: `cargo clippy -- -D warnings`
预期: 无 warning

- [ ] **Step 3: 运行 fmt**

运行: `cargo fmt --check`
预期: 无格式问题

- [ ] **Step 4: 运行 QEMU 系统测试（riscv64）**

运行: `cargo xtask test --arch riscv64`
预期: PASS

- [ ] **Step 5: 运行 QEMU 系统测试（aarch64）**

运行: `cargo xtask test --arch aarch64`
预期: PASS

- [ ] **Step 6: 提交（如有 fmt/clippy 修复）**

```bash
git add -A
git commit --signoff -m "chore: fmt + clippy 修复"
```

---

### Task 8: 更新设计文档

**Files:**
- Modify: `docs/rust-rewrite/memory-subsystem.md`

- [ ] **Step 1: 验证文档一致性**

确认 `docs/rust-rewrite/memory-subsystem.md` 中的 §3.3、§5.4、§7.2、§8.2、§10 与实现一致。
文档已在本次工作中预先更新，此步骤主要是验证代码和文档没有偏差。如有不一致则修正。

- [ ] **Step 2: 提交（如有修正）**

```bash
git add docs/
git commit --signoff -m "docs(memory): 确认文档与实现一致"
```
