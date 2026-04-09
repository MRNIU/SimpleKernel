# SAS 全量映射 + OwnedPages 所有权模型 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复内存子系统的 typestate/全量映射矛盾，使 Rust 类型系统完美追踪帧所有权。

**Architecture:** boot 全量映射物理内存（背景层 kernel_rw），`MappedPages` 改名 `OwnedPages`——管理帧所有权 + 权限覆盖。`map_page` 接受已有同-PA 映射（幂等/更新 flags）。`drop` 恢复默认权限而非删除 PTE。bitmap 分配器替换 buddy（外部元数据，修复 safety 契约）。

**Tech Stack:** Rust nightly, `#![no_std]`, RISC-V Sv39 / AArch64 4KB pages

**ADR:** `docs/decisions/005-sas-full-mapping-owned-pages.md`

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 修改 | `crates/frame_allocator/Cargo.toml` | 移除 `buddy_system_allocator` 依赖 |
| 新建 | `crates/frame_allocator/src/backend.rs` | `FrameAllocBackend` trait（已存在） |
| 新建 | `crates/frame_allocator/src/bitmap.rs` | `BitmapAllocator`（已存在） |
| 修改 | `crates/frame_allocator/src/lib.rs` | 添加 `mod backend; mod bitmap;` |
| 修改 | `crates/frame_allocator/src/alloc.rs` | 用 `BitmapAllocator` 替换 buddy |
| 修改 | `crates/frame_allocator/src/transitions.rs` | 恢复 `alloc()` 清零 + 更新文档 |
| 修改 | `crates/frame_allocator/src/state.rs` | 更新 typestate 文档（所有权语义） |
| 修改 | `crates/paging/src/table.rs` | `map_at_level` 处理同-PA 已有映射 |
| 修改 | `crates/paging/src/mapping.rs` | `MappedPages` → `OwnedPages`，map/drop 语义 |
| 修改 | `crates/paging/src/lib.rs` | re-export `OwnedPages`，`NodeFrame::alloc` 清零 |
| 修改 | `crates/paging/src/error.rs` | 移除 `AlreadyMappedIdentical`（不再是错误） |
| 修改 | `crates/memory/src/init.rs` | 分离 reserved/free 边界 + 背景映射 |
| 修改 | `crates/memory/src/lib.rs` | re-export 改名 |
| 修改 | `crates/memory/src/vma.rs` | 使用 `OwnedPages` |
| 修改 | `tests/paging-test/src/mapping.rs` | 更新测试预期 |

---

### Task 1: bitmap 分配器（已完成，验证并清理）

`backend.rs` 和 `bitmap.rs` 已在本次对话中创建。需要更新文档注释移除"unmapped"语言。

**Files:**
- Modify: `crates/frame_allocator/src/backend.rs`
- Modify: `crates/frame_allocator/src/bitmap.rs`
- Modify: `crates/frame_allocator/Cargo.toml`
- Modify: `crates/frame_allocator/src/lib.rs`
- Modify: `crates/frame_allocator/src/alloc.rs`

- [ ] **Step 1: 更新 `backend.rs` 文档**

移除"否则空闲帧无法保持 unmapped 状态"——在全量映射下空闲帧 IS mapped。改为：
```rust
/// 实现不得在空闲帧内存中存储任何元数据（链表指针等），
/// 确保分配器状态与帧内容完全解耦——帧内容由所有者管理。
```

- [ ] **Step 2: 更新 `bitmap.rs` 模块文档**

移除 typestate unmapped 语言，改为：
```rust
//! alloc/dealloc 只操作 bitmap，不读写帧的物理地址——
//! 分配器元数据与帧内容完全解耦，帧内容由所有者管理。
```

- [ ] **Step 3: 确认 `Cargo.toml` 已移除 `buddy_system_allocator`**

```toml
[dependencies]
config = { path = "../config" }
memory_types = { path = "../memory_types" }
sync_crate = { path = "../sync", package = "sync" }

heapless.workspace = true
log.workspace = true
```

- [ ] **Step 4: 确认 `lib.rs` 已包含 `mod backend; mod bitmap;`**

- [ ] **Step 5: 确认 `alloc.rs` 使用 `BitmapAllocator`**

确认 `FrameAllocatorInner` 使用 `BitmapAllocator`，`add_frames` 调用正确。

- [ ] **Step 6: 构建验证**

```bash
cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target aarch64-unknown-none-softfloat -p frame_allocator 2>&1 | grep '^error'
```
Expected: 无 error

- [ ] **Step 7: Commit**

```bash
git add crates/frame_allocator/
git commit --signoff -m "refactor(frame_allocator): 用 bitmap 替换 buddy_system_allocator

外部元数据分配器——alloc/dealloc 不触碰帧内存，
分配器状态与帧内容完全解耦。

ADR-005: SAS 全量映射 + OwnedPages 所有权模型"
```

---

### Task 2: `map_at_level` 处理同-PA 已有映射

**Files:**
- Modify: `crates/paging/src/table.rs:230-241`
- Modify: `crates/paging/src/error.rs`

- [ ] **Step 1: 修改 `map_at_level` 逻辑**

```rust
// crates/paging/src/table.rs — map_at_level 中 current.is_valid() 分支
if current.is_valid() {
    if current.paddr() != pa {
        panic!(
            "map_at_level: VA {} 已映射到 PA {}，试图重映射到 PA {}（不同 PA 是内核 bug）",
            va, current.paddr(), pa
        );
    }
    // 同一 PA——幂等或权限变更，更新 flags
    if current.flags() != leaf_flags {
        table.write(idx, PageTableEntry::new(pa, leaf_flags));
    }
    return Ok(());
}
```

- [ ] **Step 2: 清理 `PagingError`**

从 `error.rs` 中移除 `AlreadyMappedIdentical` 和 `AlreadyMappedConflict`——同 PA 不再是错误，不同 PA 直接 panic。

```rust
pub enum PagingError {
    AllocationFailed,
    // AlreadyMappedIdentical 和 AlreadyMappedConflict 已移除
    HugePageConflict,
    PageNotMapped,
    FrameAllocFailed,
}
```

同步更新 `Display` impl 和 `crates/memory/src/error.rs` 的 `From<PagingError>` impl。

- [ ] **Step 3: 更新 `identity_map_range`**

移除对 `AlreadyMappedIdentical` 的特殊处理（不再需要，`map_at_level` 已处理）：

```rust
// table.rs — identity_map_range
match self.map_at_level(va, addr, flags, level) {
    Ok(()) => {}
    Err(e) => panic!("identity_map_range: 映射 {va} 失败: {e}"),
}
```

- [ ] **Step 4: 构建验证**

```bash
cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target aarch64-unknown-none-softfloat -p paging 2>&1 | grep '^error'
```

- [ ] **Step 5: Commit**

```bash
git add crates/paging/src/table.rs crates/paging/src/error.rs crates/memory/src/error.rs
git commit --signoff -m "refactor(paging): map_at_level 接受同-PA 已有映射

同 PA + 同 flags → 幂等跳过；同 PA + 不同 flags → 更新权限。
不同 PA → panic（内核 bug）。
移除 AlreadyMappedIdentical / AlreadyMappedConflict 错误变体。

ADR-005"
```

---

### Task 3: `MappedPages` → `OwnedPages`——语义重构

**Files:**
- Modify: `crates/paging/src/mapping.rs`
- Modify: `crates/paging/src/lib.rs`
- Modify: `crates/paging/src/mmio.rs`（无变化——MmioRegion 不用 MappedPages）
- Modify: `crates/memory/src/lib.rs`
- Modify: `crates/memory/src/vma.rs`

- [ ] **Step 1: 重命名 `MappedPages` → `OwnedPages`**

`crates/paging/src/mapping.rs`:
- `pub struct MappedPages` → `pub struct OwnedPages`
- 更新模块文档注释：

```rust
//! 仿射类型所有权——move-only 的物理帧所有权 + 权限管理。
//!
//! [`OwnedPages`] 持有物理帧的独占所有权，VA 通过 identity mapping
//! 从 PA 推导。SAS 架构下所有物理内存始终有背景 identity mapping（kernel_rw），
//! `OwnedPages` 管理的是 **所有权和权限覆盖层**——
//! `map` 接管所有权并按需调整 PTE flags，`drop` 恢复默认权限并归还帧。
```

- 更新 struct 文档：

```rust
/// 仿射类型帧所有权——持有物理帧的独占所有权和当前权限。
///
/// 不可 Clone、不可 Copy。Drop 时恢复 PTE 为默认权限（kernel_rw）并回收帧。
///
/// SAS 全量映射下，所有物理内存始终有 identity mapping（背景层）。
/// `OwnedPages` 不创建/删除 PTE，而是管理权限覆盖：
/// - `map`：接管帧所有权，按需更新 PTE flags
/// - `mprotect`：修改权限
/// - `drop`：恢复 kernel_rw + 归还帧
```

- [ ] **Step 2: 修改 `OwnedPages::map`**

移除 `.expect("map_page 失败")` 恢复到正常 `map_page` 调用（现在 `map_page` 自己处理同-PA）。文档更新：

```rust
/// 消费 frames 的所有权，设置指定权限。
///
/// SAS 全量映射下，PTE 已由 boot 背景映射建立。
/// 此方法接管帧所有权并按需更新 PTE flags（如从默认 kernel_rw 改为 kernel_ro）。
pub fn map(frames: AllocatedFrames, flags: PteFlags) -> Self {
```

- [ ] **Step 3: 修改 `OwnedPages::drop`——恢复默认权限而非 unmap**

```rust
impl Drop for OwnedPages {
    fn drop(&mut self) {
        // 恢复 PTE 为默认 kernel_rw（背景映射权限），不删除 PTE
        let pt = crate::kernel_page_table();
        let default_flags = PteFlags::kernel_rw();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            let mut guard = pt.lock();
            guard
                .update_flags(va, default_flags)
                .expect("OwnedPages::drop: update_flags 失败");
            drop(guard);
        }
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        }
        // SAFETY: Drop 执行中，self.frames 不会再被访问。
        let mapped_frames = unsafe { ManuallyDrop::take(&mut self.frames) };
        let _unmapped = mapped_frames.into_unmapped();
    }
}
```

- [ ] **Step 4: 修改 `OwnedPages::unmap`——恢复默认权限**

```rust
/// 释放所有权并取回帧——恢复 PTE 为默认权限（不删除映射）。
pub fn unmap(self) -> UnmappedFrames {
    let mut md = ManuallyDrop::new(self);
    let mapped_frames = unsafe { ManuallyDrop::take(&mut md.frames) };
    // 恢复默认权限
    let pt = crate::kernel_page_table();
    let default_flags = PteFlags::kernel_rw();
    let va_start = mapped_frames.start_paddr().to_virt();
    let page_count = mapped_frames.count();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        let mut guard = pt.lock();
        guard
            .update_flags(va, default_flags)
            .expect("OwnedPages::unmap: update_flags 失败");
        drop(guard);
    }
    {
        let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
    }
    mapped_frames.into_unmapped()
}
```

- [ ] **Step 5: 更新 re-export 和使用点**

`crates/paging/src/lib.rs`:
```rust
pub use mapping::OwnedPages;
```

`crates/memory/src/lib.rs`:
```rust
pub type OwnedPages = paging::OwnedPages;
```

`crates/memory/src/vma.rs`: 将 `MappedPages` 替换为 `OwnedPages`。

全仓库搜索替换 `MappedPages` → `OwnedPages`（除文档引用和 ADR 中的对比说明）。

- [ ] **Step 6: 构建验证**

```bash
cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target aarch64-unknown-none-softfloat 2>&1 | grep '^error'
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit --signoff -m "refactor(paging): MappedPages → OwnedPages 所有权模型

SAS 全量映射下 OwnedPages 管理帧所有权 + 权限覆盖：
- map: 接管所有权 + 按需更新 PTE flags
- drop: 恢复默认 kernel_rw + 归还帧（不删 PTE）
- unmap: 同 drop 但返回 UnmappedFrames

ADR-005"
```

---

### Task 4: `memory::init` 分离 reserved/free 边界 + 背景映射

**Files:**
- Modify: `crates/memory/src/init.rs`

- [ ] **Step 1: 修改 init.rs**

```rust
pub fn init() -> AddressSpace {
    unsafe { heap_crate::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();
    let mem_end = mem_start + mem_size;

    // 空闲内存 = 内核之后的部分
    let free_start = kernel_end.align_up();
    let free_size = mem_size - (free_start - mem_start);

    // reserved 只含内核段（不含 free pool），满足 "reserved 与 free 不重叠" 契约
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (free_start - rodata_end) / config::PAGE_SIZE;

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

    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    let mut kernel_as = AddressSpace::new();

    // 分段映射：.text(RWX) / .rodata(RO) / .data(RW)
    let segments: [PteFlags; 3] = [
        PteFlags::kernel_rwx(),
        PteFlags::kernel_ro(),
        PteFlags::kernel_rw(),
    ];
    let seg_starts = [mem_start, text_end, rodata_end];

    for (i, flags) in segments.into_iter().enumerate() {
        let frames = reserved.remove(0);
        let va = memory_types::VirtAddr::new(seg_starts[i].as_usize());
        let mapping = paging::OwnedPages::map(frames, flags);
        kernel_as.register_kernel_mapping(va, mapping);
    }

    // 背景映射：free pool 全量映射为 kernel_rw（通过 identity_map_range 直接操作页表）
    // 这些帧不被 OwnedPages 持有——是 SAS 背景层的一部分
    {
        let mut guard = paging::kernel_page_table().lock();
        guard.identity_map_range(free_start, mem_end, PteFlags::kernel_rw());
    }

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW), free {}-{} (RW bg)",
        mem_start, text_end,
        text_end, rodata_end,
        rodata_end, free_start,
        free_start, mem_end
    );

    kernel_as
}
```

- [ ] **Step 2: 构建验证**

```bash
cargo build -Z build-std=core,compiler_builtins,alloc -Z build-std-features=compiler-builtins-mem --target aarch64-unknown-none-softfloat 2>&1 | grep '^error'
```

- [ ] **Step 3: Commit**

```bash
git add crates/memory/src/init.rs
git commit --signoff -m "fix(memory): 分离 reserved/free 边界 + free pool 背景映射

reserved 只含内核段（text/rodata/data），不含 free pool，
满足 frame_allocator::init 的 safety 契约。
free pool 通过 identity_map_range 建立背景映射（kernel_rw）。

ADR-005"
```

---

### Task 5: 恢复 `AllocatedFrames::alloc()` 清零 + 更新 typestate 文档

**Files:**
- Modify: `crates/frame_allocator/src/transitions.rs`
- Modify: `crates/frame_allocator/src/state.rs`

- [ ] **Step 1: 恢复 `alloc()` 中的清零**

全量映射下帧始终可访问，可以安全清零。reserved 帧通过 `from_range` 构造（不走 `alloc()`），所以内核代码不会被清零。

```rust
pub fn alloc(count: usize) -> Result<Self, FrameAllocError> {
    let count_4k = count << P::NUM_4K_PAGES_SHIFT;
    let free = alloc_from_buddy(count_4k)?;

    // 全量映射下帧始终可访问，分配后立即清零防止泄漏旧数据。
    // SAFETY: identity mapping 下 PA.to_virt() 有效，帧刚分配无其他引用。
    unsafe {
        let ptr = free.start_paddr().to_virt().as_mut_ptr::<u8>();
        core::ptr::write_bytes(ptr, 0, count_4k * config::PAGE_SIZE);
    }

    let range = free.range();
    core::mem::forget(free);

    assert!(
        P::NUM_4K_PAGES == 1 || range.start().as_usize() & (P::NUM_4K_PAGES - 1) == 0,
        "AllocatedFrames::alloc: 分配器返回的帧未对齐到 P 边界"
    );

    Ok(Self::from_range(range))
}
```

- [ ] **Step 2: 更新 state.rs typestate 文档**

```rust
/// 帧生命周期状态——追踪物理帧的**所有权**。
///
/// SAS 全量映射下所有帧始终有 identity mapping（背景层 kernel_rw）。
/// 状态不代表 PTE 是否存在，而代表谁持有帧：
///
/// ```text
/// Free -> Allocated -> Mapped -> Unmapped --> Free
///                                         \-> Allocated（重新映射）
/// ```
///
/// - `Free`：分配器持有，背景 kernel_rw 权限
/// - `Allocated`：用户持有，尚未通过 OwnedPages 管理权限
/// - `Mapped`：OwnedPages 持有，PTE flags 可能已被覆盖为非默认值
/// - `Unmapped`：从 OwnedPages 释放，PTE 已恢复为 kernel_rw，等待回收
```

- [ ] **Step 3: 更新 `transitions.rs` alloc 文档**

```rust
/// 分配 `count` 个连续的 P 大小物理帧，内容清零。
///
/// SAS 全量映射下帧始终可访问，分配后立即清零防止泄漏旧数据。
```

- [ ] **Step 4: 移除 `KernelNodeFrame::alloc()` 中的手动清零**

`crates/paging/src/lib.rs` 中 `KernelNodeFrame::alloc()` 不再需要手动清零——`AllocatedFrames::alloc_one()` 已清零。

```rust
impl NodeFrameOps for KernelNodeFrame {
    fn alloc() -> Result<Self, error::PagingError> {
        frame_allocator::AllocatedFrames::alloc_one()
            .map(Self)
            .map_err(|e| {
                log::warn!("页表节点帧分配失败: {:?}", e);
                error::PagingError::AllocationFailed
            })
    }
    fn paddr(&self) -> PhysAddr {
        self.0.start_paddr()
    }
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/frame_allocator/src/ crates/paging/src/lib.rs
git commit --signoff -m "refactor(frame_allocator): 恢复 alloc 清零 + typestate 所有权语义

全量映射下帧始终可访问，alloc 后立即清零。
typestate 文档更新为所有权追踪语义（不再声称 Free 帧无 PTE）。
移除 KernelNodeFrame::alloc 中的冗余手动清零。

ADR-005"
```

---

### Task 6: 更新测试

**Files:**
- Modify: `tests/paging-test/src/mapping.rs`

- [ ] **Step 1: 更新 import 和类型名**

`MappedPages` → `OwnedPages`

- [ ] **Step 2: 修改 `test_drop_unmaps`**

原测试期望 drop 后 PTE 消失。新行为：drop 后 PTE 存在但 flags 恢复为 kernel_rw。

```rust
/// Drop 自动恢复默认权限。
fn test_drop_restores_default_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    {
        let guard = paging::kernel_page_table().lock();
        let (_, flags) = guard.get_mapping(va).expect("映射应存在");
        assert!(!flags.is_writable(), "map 时应为 RO");
    }
    drop(mp);
    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("背景映射应仍存在");
    assert!(flags.is_writable(), "drop 后应恢复为默认 RW");
}
```

- [ ] **Step 3: 修改 `test_unmap_returns_unmapped_frames`**

```rust
/// unmap 返回 UnmappedFrames 并恢复默认权限。
fn test_unmap_returns_unmapped_frames() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    let unmapped = mp.unmap();
    assert_eq!(unmapped.start_paddr(), pa);

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("背景映射应仍存在");
    assert!(flags.is_writable(), "unmap 后应恢复为默认 RW");
}
```

- [ ] **Step 4: 新增 `test_map_preexisting` 测试幂等映射**

```rust
/// 对已有背景映射的帧调用 map 应成功（幂等）。
fn test_map_preexisting() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let pa = frames.start_paddr();
    let va = pa.to_virt();

    // 背景映射已存在（kernel_rw），map 应成功
    let mp = OwnedPages::map(frames, PteFlags::kernel_rw());
    assert_eq!(mp.vaddr(), va);

    // 验证 PTE 正确
    let guard = paging::kernel_page_table().lock();
    let (got_pa, _) = guard.get_mapping(va).expect("映射应存在");
    assert_eq!(got_pa, pa);
}
```

- [ ] **Step 5: 新增 `test_map_changes_flags` 测试权限覆盖**

```rust
/// map 可以覆盖背景映射的权限。
fn test_map_changes_flags() {
    let frames = AllocatedFrames::alloc(1).expect("alloc frames");
    let va = frames.start_paddr().to_virt();
    let mp = OwnedPages::map(frames, PteFlags::kernel_ro());

    let guard = paging::kernel_page_table().lock();
    let (_, flags) = guard.get_mapping(va).expect("映射应存在");
    assert!(!flags.is_writable(), "map(kernel_ro) 应设为 RO");
    drop(guard);
    drop(mp);
}
```

- [ ] **Step 6: 更新 `run_tests` 函数**

```rust
fn run_tests() {
    test_map_basic();
    log::info!("test map_basic ... ok");

    test_map_multi_page();
    log::info!("test map_multi_page ... ok");

    test_map_preexisting();
    log::info!("test map_preexisting ... ok");

    test_map_changes_flags();
    log::info!("test map_changes_flags ... ok");

    test_drop_restores_default_flags();
    log::info!("test drop_restores_default_flags ... ok");

    test_mprotect_changes_flags();
    log::info!("test mprotect_changes_flags ... ok");

    test_unmap_returns_unmapped_frames();
    log::info!("test unmap_returns_unmapped_frames ... ok");

    log::info!("paging-test: all 7 tests passed");
}
```

- [ ] **Step 7: 运行测试**

```bash
cargo xtask test --arch aarch64 --name paging-test/mapping
cargo xtask test --arch riscv64 --name paging-test/mapping
```

- [ ] **Step 8: Commit**

```bash
git add tests/paging-test/
git commit --signoff -m "test(paging): 更新映射测试适配 OwnedPages 所有权模型

- test_drop_unmaps → test_drop_restores_default_flags
- test_unmap: 检查 flags 恢复而非 PTE 删除
- 新增 test_map_preexisting（幂等）
- 新增 test_map_changes_flags（权限覆盖）

ADR-005"
```

---

### Task 7: 全仓库搜索 + CLAUDE.md 更新

- [ ] **Step 1: 搜索所有使用 `MappedPages` 的文件**

```bash
rg 'MappedPages' --type rust -l
```

确保全部替换为 `OwnedPages`。

- [ ] **Step 2: 更新 CLAUDE.md**

- `MappedPages` → `OwnedPages`
- `帧生命周期` 描述从"编译期追踪"改为"编译期追踪所有权"
- `映射所有权` 改为"帧所有权"

- [ ] **Step 3: 最终构建 + 全部测试**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo xtask test --arch aarch64
cargo xtask test --arch riscv64
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit --signoff -m "docs: 更新全仓库 MappedPages → OwnedPages 引用

ADR-005"
```
