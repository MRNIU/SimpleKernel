# 内存管理子系统 v2

> 本文档面向**内核开发者**，系统描述 SimpleKernel 内存管理的设计哲学、
> 所有权模型和生命周期流转。读完本文你应该能回答：
>
> - SimpleKernel 的内存模型与 Theseus 等内核有什么根本区别？
> - 一个物理帧从分配到回收经历了哪些类型状态？
> - `OwnedPages` 如何通过 Rust 所有权在编译期保证帧不泄漏？
> - 为什么不需要大页和 page splitting？
>
> 演进历史：本文档取代 [memory-subsystem.md](memory-subsystem.md)（旧设计）。
> 决策记录见 [ADR-006](../decisions/006-memory-subsystem-simplification.md)。
>
> **⚠ 部分内容已过时**：[ADR-007](../decisions/007-eliminate-vma-and-dead-code.md) 删除了
> VMA 模块（`AddressSpace`/`Vma`/`mmap`/`munmap`/`mprotect`）、`unmap_page`、`as_type`/`as_type_mut`。
> 内核段 OwnedPages 改为 `mem::forget` 永久持有，MMIO 重叠检测改为 `BTreeMap` 内联实现。
> 本文档中涉及这些内容的章节（§7 VMA、§8 生命周期中的 munmap/as_type 等）以代码为准。

---

## 1. 设计哲学

SimpleKernel 内存子系统基于三条原则：

### 1.1 比 Theseus 更激进的 SAS

SimpleKernel 采用单地址空间（SAS）架构，所有代码运行在同一特权级和地址空间中。
与 Theseus 不同的是，SimpleKernel 对全量映射做了更激进的选择：**所有物理内存在
boot 时一次性 identity map，运行时永远不创建或删除 PTE**。

这意味着页表的角色发生了根本变化：

| | Theseus | SimpleKernel |
|--|---------|-------------|
| 页表角色 | **隔离机制**——PTE 有无决定帧能否访问 | **权限管控**（纵深防御）——PTE 始终存在 |
| "映射"的含义 | 创建 VA→PA 翻译（PTE 从无到有） | 不存在——所有翻译永远存在 |
| 帧可访问性 | 取决于有没有 PTE 指向它 | 始终可访问（identity mapping） |
| 分配后的帧 | 需要先 map 才能用 | 分配完就能用（背景 kernel_rw） |
| 帧所有权追踪 | PTE EXCLUSIVE 位（运行时硬件位） | Rust struct 字段（编译期） |

### 1.2 内存充裕——全量映射无代价

现代计算机的物理内存远小于虚拟地址空间（RISC-V Sv39: 512GB，AArch64-48bit: 256TB）。
全量映射所有物理内存的页表开销在 128MB RAM 下约 264KB（0.2%），可忽略。
这个前提让我们可以在 boot 时一次性建立完整映射，运行时只管权限。

### 1.3 编译期保证优于运行时保证

帧的生命周期完全由 Rust 类型系统在编译期追踪：

- **帧不会 double-free**——Rust 所有权唯一（编译期强制）
- **帧不会泄漏**——`OwnedPages::Drop` 必然执行（编译期强制）
- **Drop 恢复权限**——结构体析构顺序确定（编译期强制）
- **映射存活期间帧不被回收**——字段与 struct 同生命周期（编译期强制）

不依赖运行时 panic 保护网、不依赖 PTE 软件位标记、不依赖引用计数。

### 1.4 学术定位

SimpleKernel 处于以下研究线的交汇点：

| 研究方向 | 代表系统 | SimpleKernel 的取法 |
|---------|---------|-------------------|
| 全量映射 | Linux direct map, rCore identity map | 相同——全部物理内存一次性映射 |
| 语言级隔离 | Theseus OSDI'20, RedLeaf OSDI'20, SPIN SOSP'95 | 相同——Rust 编译器是保护环 |
| 权限覆盖 | Intel MPK, ARM POE, libhermitMPK VEE'20 | 软件层实现——映射不变，只改权限 |
| 纵深防御 | SafeBPF CCSW'24, Unishyper EMSOFT'23 | 语言安全（主）+ PTE 权限（辅） |
| SAS + 语言安全 | Singularity MSR'05 | 类似——但 Singularity 用 GC 语言，SimpleKernel 用 Rust 零开销所有权 |

完整参考文献见 `docs/design/references.md`。

### 1.5 安全 trade-off

全量映射意味着：任何 `unsafe` 代码可通过裸指针访问任何物理内存。
防线是：

1. APP crate 标记 `#![forbid(unsafe_code)]`——编译器禁止 unsafe
2. 内核内部 `unsafe` 集中在少数底层 crate 并严格审计
3. PTE 权限位作为纵深防御——即使有 unsafe 越权，NX/RO 仍能拦截部分操作

这是有意识的取舍：更强的编译期保证 + 更弱的硬件兜底。
如果需要运行不受信任的二进制，那就不再是 SAS——需要从根本上重新设计。

---

## 2. 架构总览

内存子系统由 6 个 crate 组成，分为三层：

```
┌─────────────────────────────────────────────────┐
│  策略层 — memory crate                            │
│  init · map_mmio · MMIO 跟踪                      │
├─────────────────────────────────────────────────┤
│  机制层 — paging crate                            │
│  PageTable · OwnedPages · MmioRegion              │
├───────────────┬───────────────┬─────────────────┤
│ frame_allocator│ page_table_entry│ tlb            │
│ 物理帧分配     │ PTE 编解码      │ TLB 刷新       │
│ Frames<S>     │               │ TlbFlushGuard   │
├───────────────┴───────────────┴─────────────────┤
│  memory_types — PhysAddr · VirtAddr · Frame · Span │
└─────────────────────────────────────────────────┘
```

### 各层职责

| 层 | Crate | 职责 | 不做什么 |
|----|-------|------|---------|
| **策略** | `memory` | 初始化编排、MMIO 注册、map_mmio 便捷函数 | 不直接操作 PTE |
| **机制** | `paging` | 页表遍历、PTE 读写、OwnedPages 权限管理、MmioRegion | 不分配帧——由上层告知 |
| **资源** | `frame_allocator` | 物理帧的分配/回收/typestate 追踪 | 不知道页表的存在 |
| **资源** | `page_table_entry` | 单个 PTE 的 bit-level 编解码（跨架构） | 不做页表遍历 |
| **资源** | `memory_types` | 地址/帧号 newtype、Span 区间 | 无运行时状态 |
| **资源** | `tlb` | 架构相关的 TLB invalidate（RAII guard） | 仅 flush，不操作 PTE |

### 分层命名约定

每一层用自己的术语，互不侵入：

```
frame_allocator:  FrameState::Free / Allocated
                  AllocatedFrames::alloc() / Drop

paging:           OwnedPages::new() / set_flags() / Drop

memory:           init() / map_mmio()
```

---

## 3. 基础类型——`memory_types`

所有内存操作的基石是编译期区分物理/虚拟的 newtype：

```rust
PhysAddr  ──page_number()──▶ Frame   ──Span──▶ FrameSpan
VirtAddr  ──page_number()──▶ Page    ──Span──▶ PageSpan
```

- `PhysAddr` / `VirtAddr`：编译期不可互换——把 PA 传给期望 VA 的函数是编译错误
- `Frame` / `Page`：页粒度标识（固定 4KB）
- `Span<A>`：泛型半开区间 `[start, end)`，支持 split/merge/overlap 检测

所有页大小固定为 4KB（`config::PAGE_SIZE = 4096`）。
两目标架构（RISC-V Sv39 + AArch64 4KB granule）的基础页大小一致，
不支持大页（2MB/1GB）——详见 [ADR-006](../decisions/006-memory-subsystem-simplification.md)。

---

## 4. 物理帧生命周期——`frame_allocator`

### 4.1 2-state typestate

物理帧使用 **const generic typestate** 追踪所有权：

```
                alloc()              Drop
  buddy pool ──────────▶ Allocated ────────▶ buddy pool
               (Free→Allocated)       (dealloc_to_backend)
```

只有两个状态——帧在分配器中（Free）或被外部持有（Allocated）。

```rust
#[derive(PartialEq, Eq, ConstParamTy)]
pub enum FrameState {
    Free,       // 分配器持有
    Allocated,  // 被某个 Rust struct 持有
}

pub struct Frames<const S: FrameState> {
    range: FrameSpan,  // 连续物理帧 [start, end)
}

pub type FreeFrames = Frames<{ FrameState::Free }>;
pub type AllocatedFrames = Frames<{ FrameState::Allocated }>;
```

Drop 行为统一——所有状态的帧都归还分配器：

```rust
impl<const S: FrameState> Drop for Frames<S> {
    fn drop(&mut self) {
        dealloc_to_backend(self.range);
    }
}
```

### 4.2 为什么只有 2 个状态

旧设计（来自 Theseus）有 4 个状态：Free / Allocated / Mapped / Unmapped。
Mapped/Unmapped 在 Theseus 中追踪 PTE 生命周期：

- `Mapped`：帧地址已写入 PTE（帧"沉入"页表）
- `Unmapped`：PTE 已清零（帧从页表"恢复"）
- `MappedFrames::Drop` panic——防止帧卡在 PTE 里无法回收

在 SimpleKernel 全量映射下，这些都不适用：

- PTE 始终存在——帧从不"沉入"或"恢复"
- 帧始终在 `OwnedPages` 的 struct 字段中——Rust 所有权直接追踪
- 不需要运行时 panic 保护网——编译器保证 `OwnedPages::Drop` 必然执行

2-state 让帧所有权完全由 Rust 类型系统追踪，消除了 `ManuallyDrop` + `unsafe`。

### 4.3 分配器后端

```rust
static FRAME_ALLOCATOR: SpinLockIrq<FrameAllocatorInner>;

// 唯一的分配出口
fn alloc_from_backend(count: usize) -> Result<FreeFrames, FrameAllocError>;

// 唯一的回收入口（仅由 Frames::Drop 调用）
fn dealloc_to_backend(range: FrameSpan);
```

后端使用 `buddy_system_allocator::FrameAllocator<32>`：
- O(log n) alloc / dealloc（buddy 合并）
- 元数据存储在堆上的 BTreeSet，不触碰空闲帧内存
- 分配结果按 next_power_of_two 对齐——满足硬件对齐需求

### 4.4 初始化

`init()` 接受两类参数：

- `free_start` / `free_size`：空闲物理内存范围，加入 buddy
- `reserved`：预留的物理地址范围（内核 .text/.rodata/.data 段），
  **不经过 buddy**——直接构造为 `AllocatedFrames` 返回

预留范围的帧从未在 buddy 中注册。如果意外 Drop，`dealloc_to_backend`
会将未注册的帧交给 buddy 导致状态污染。

**不变量**：内核段帧由 `mem::forget` 永久持有——权限已写入页表，
Drop 永不执行（不恢复权限、不归还帧）。

### 4.5 连续帧分配

所有分配返回单个连续范围（`FrameSpan`）。这是 SAS identity mapping 下的自然约束：

| 好处 | 原因 |
|------|------|
| 映射简单 | 一个 `FrameSpan`，不需要 scatter-gather 列表 |
| DMA 友好 | 大多数 DMA 控制器要求物理连续 buffer |
| 硬件预取有效 | CPU prefetcher 按物理地址模式预取 |

这与 Linux `kmalloc`（物理连续，优先选择）是同一个取舍。

---

## 5. 权限覆盖——`OwnedPages`

### 5.1 核心概念：背景层 + 覆盖层

```
物理内存全貌:
┌──────────────────────────────────────────────────────┐
│ .text        │ .rodata   │ .data/.bss │  free pool   │
│ RWX          │ RO        │ RW         │  kernel_rw   │  ← 背景层（init 建立，永久）
│              │           │            │              │
│              │           │            │  ┌─OwnedP─┐  │
│              │           │            │  │ RO      │  │  ← 覆盖层（运行时，临时）
│              │           │            │  └─────────┘  │
└──────────────────────────────────────────────────────┘
```

- **背景层**：`memory::init()` 建立，identity map 全部物理内存为 kernel_rw，永不变化
- **覆盖层**：`OwnedPages::new` 将特定帧的 PTE 权限改为指定值，
  `OwnedPages::Drop` 恢复回 kernel_rw

`OwnedPages` 是一个**权限守卫**（permission guard），
与 `MutexGuard` 同一模式：构造时获取资源，析构时释放资源。

### 5.2 结构

```rust
pub struct OwnedPages {
    frames: AllocatedFrames,  // 帧所有权——直接持有，无 ManuallyDrop
    flags: PteFlags,          // 当前权限
}
// 不可 Clone，不可 Copy——move-only 仿射类型
```

### 5.3 API

```rust
impl OwnedPages {
    /// 构造：接管帧所有权 + 设置 PTE 权限。
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self;

    /// 修改权限。
    pub fn set_flags(&mut self, new_flags: PteFlags);

    /// 访问器。
    pub fn vaddr(&self) -> VirtAddr;
    pub fn size(&self) -> usize;
    pub fn page_count(&self) -> usize;
    pub fn flags(&self) -> PteFlags;

    /// 类型化内存访问。
    pub fn as_type<T: FromBytes>(&self, offset: usize) -> &T;
    pub fn as_type_mut<T: FromBytes + IntoBytes>(&mut self, offset: usize) -> &mut T;
}

impl Drop for OwnedPages {
    fn drop(&mut self) {
        restore_default_flags(self.vaddr(), self.page_count()); // 恢复 kernel_rw
        // self.frames 自动 Drop → dealloc_to_backend
    }
}
```

**零 unsafe**。`Drop` 先恢复 PTE 权限，然后 `AllocatedFrames` 字段自动析构归还 buddy。

### 5.4 编译期保证

| 保证 | 如何实现 |
|------|---------|
| 帧不会 double-free | Rust 所有权唯一（编译期） |
| 帧不会泄漏 | `OwnedPages::Drop` 必然执行（编译期） |
| Drop 恢复权限 | Drop 方法体显式执行 `restore_default_flags`（编译期确定调用） |
| 映射存活期间帧不被回收 | `frames` 字段与 `OwnedPages` 同生命周期（编译期） |
| `as_type` 引用不逃逸 | 返回引用生命周期绑定到 `&self`（编译期） |

### 5.5 与 Theseus `MappedPages` 的对比

| | Theseus `MappedPages` | SimpleKernel `OwnedPages` |
|--|----------------------|--------------------------|
| 持有帧？ | **否**——帧沉入 PTE | **是**——`AllocatedFrames` 字段 |
| 持有页？ | 是——`AllocatedPages` 字段 | 否——VA 从 PA 推导（identity mapping） |
| 构造 | `map(pages, frames, flags)` | `new(frames, flags)` |
| 析构 | 清零 PTE + 回收帧（从 PTE 恢复）+ 回收页 | 恢复 PTE 权限 + 释放帧（从 struct 字段） |
| unsafe | `mem::forget` + `unsafe from_unmapped_range` | 零 |
| 帧 typestate | 4 状态（追踪 PTE 生命周期） | 2 状态（追踪分配器所有权） |
| EXCLUSIVE 位 | 需要（区分可回收/共享帧） | 不需要（所有帧都可回收） |

---

## 6. MMIO 区域——`MmioRegion`

MMIO 地址是硬件寄存器，**不是 RAM**，不在 buddy allocator 中。
`MmioRegion` 与 `OwnedPages` 是平级类型，各自独立：

```rust
pub struct MmioRegion {
    base: VirtAddr,
    size: usize,
}
```

- 直接使用 `PageTable` 建立 identity mapping（设备地址不在背景映射中）
- `read_reg<T>` / `write_reg<T>` 使用 **volatile** 语义
- 映射永久存在（生命周期等于设备驱动）
- 不追踪帧所有权——MMIO 帧不归分配器管

---

## 7. ~~地址空间与 VMA~~ → 内存初始化与 MMIO——`memory` crate

> **⚠ 已过时**：VMA 模块（`AddressSpace`/`Vma`/`mmap`/`munmap`/`mprotect`）已在
> [ADR-007](../decisions/007-eliminate-vma-and-dead-code.md) 中删除。
> 以下内容保留供历史参考，**以代码为准**。

`memory` crate 是面向内核其他模块的**唯一公共 API**。

### 7.1 VMA（Virtual Memory Area）

```rust
pub struct Vma {
    range: Span<VirtAddr>,        // [start, end)，页对齐
    flags: PteFlags,              // 权限
    mapping: Option<OwnedPages>,  // None = lazy，Some = 已物化
}

pub struct AddressSpace {
    areas: BTreeMap<VirtAddr, Vma>,
}
```

SAS 架构下全局唯一。VMA 提供：
- 重叠检测（防止双重分配）
- 区域查询（`find_vma`）
- lazy mapping（page fault 按需物化）

### 7.2 POSIX 映射

POSIX API 在策略层实现，内部调用机制层：

| POSIX API | 内部操作 |
|-----------|---------|
| `mmap(size, flags)` | `AllocatedFrames::alloc` + `OwnedPages::new` + 注册 VMA |
| `munmap(addr)` | 移除 VMA → `OwnedPages::Drop`（恢复权限 + 释放帧） |
| `mprotect(addr, flags)` | `OwnedPages::set_flags` |

SAS 下不适用的 POSIX 特性：

| 特性 | 原因 |
|------|------|
| `MAP_SHARED` 多进程共享 | SAS 单地址空间，传 `&T` 即可 |
| `MAP_FIXED` 定点映射 | VA = PA，需 buddy 支持定点分配（当前不支持） |
| COW / fork | SAS 无多进程 |

---

## 8. 页表——`paging::PageTable`

### 8.1 结构

```rust
pub struct PageTable {
    root_paddr: PhysAddr,
    root: NodeFrame,
    root_ref_count: u16,
    nodes: BTreeMap<PhysAddr, NodeEntry>,
}
```

SAS 架构下全局唯一，`SpinLock` 保护。

### 8.2 多级遍历

```
RISC-V Sv39 (3 级)          AArch64 4KB (4 级)
Level 2 (root)              Level 3 (root)
  └─ Level 1                  └─ Level 2
       └─ Level 0 (4KB 叶)        └─ Level 1
                                        └─ Level 0 (4KB 叶)
```

所有映射均为 4KB 叶页（level 0）。不支持大页（2MB/1GB），
消除了 page splitting 和多级 map/unmap 的复杂性。

### 8.3 核心操作

| 方法 | 语义 |
|------|------|
| `set_page_flags(va, pa, flags)` | PTE 不存在则创建，存在则更新 flags（幂等） |
| `update_flags(va, flags)` | PTE 必须存在，更新 flags |
| `unmap_page(va)` | 清除 PTE（仅 MMIO 路径使用） |
| `identity_map_range(start, end, flags)` | 批量 4KB identity map |
| `get_mapping(va)` | 查询映射信息 |

`set_page_flags` 是 `OwnedPages::new` 的内部调用——能处理 init 阶段（PTE 不存在）
和运行时（PTE 已存在）两种场景。

### 8.4 引用计数回收

中间页表节点通过引用计数管理。每个节点记录有效 PTE 数量，
unmap 时 ref_count 降为 0 的节点自动释放。

---

## 9. 初始化顺序

```
_start (汇编)
  └─ bootstrap()
       └─ early_init()
            └─ FDT 解析 → MEMORY_INFO
                 └─ memory::init()
                      ├─ 1. heap::init()
                      ├─ 2. frame_allocator::init(free, reserved)
                      │      └─ 空闲入 buddy + 内核段帧返回
                      ├─ 3. PageTable::create()
                      ├─ 4. identity_map_range(mem_start, mem_end, kernel_rw)
                      │      └─ 背景层：全部物理内存，4KB 页
                      ├─ 5. OwnedPages::new(text,  kernel_rwx)  ─┐
                      ├─ 6. OwnedPages::new(rodata, kernel_ro)   ├─ 覆盖层
                      ├─ 7. OwnedPages::new(data,   kernel_rw)  ─┘
                      └─ 8. store_kernel_address_space(as)
```

**先背景、后覆盖**：步骤 4 建立全量映射（所有 PTE 存在），
步骤 5-7 的 `OwnedPages::new` 通过 `update_flags` 覆盖内核段权限。
data 段权限与背景相同（kernel_rw），但 `OwnedPages` 追踪其所有权。

**约束**：
- 堆必须在帧分配器之前（buddy 内部用堆上 BTreeSet）
- 背景映射在 OwnedPages 之前（OwnedPages 内部用 `update_flags`）
- 从核复用主核页表，只需激活分页

---

## 10. 端到端生命周期

### 10.1 运行时帧分配的一生

```
1. AllocatedFrames::alloc(4)
   └─ buddy 分配 4 帧 → AllocatedFrames { range: [100, 104) }
   └─ 帧内容清零（背景映射下直接写入）

2. OwnedPages::new(frames, kernel_ro)
   └─ 遍历 4 页，update_flags → PTE 从 kernel_rw 改为 kernel_ro
   └─ TLB flush
   └─ 返回 OwnedPages { frames, flags: kernel_ro }

3. 使用中
   └─ owned.as_type::<MyStruct>(0) → 类型化只读访问
   └─ 生命周期绑定到 &self，编译器保证引用不逃逸

4. Drop（或 munmap 触发）
   └─ restore_default_flags → PTE 从 kernel_ro 恢复为 kernel_rw
   └─ TLB flush
   └─ AllocatedFrames::Drop → dealloc_to_backend → 帧归还 buddy
```

### 10.2 内核段映射的一生

```
1. frame_allocator::init(reserved=[(text, N), (rodata, M), (data, K)])
   └─ 直接构造 AllocatedFrames（不经 buddy）

2. identity_map_range(mem_start, mem_end, kernel_rw)
   └─ 全部物理内存建立背景映射

3. OwnedPages::new(text_frames, kernel_rwx)
   └─ update_flags → 覆盖 .text 段权限
   └─ 注册到 AddressSpace

4. 永久持有
   └─ AddressSpace 存于 spin::Once（'static）→ 永远不 Drop
```

---

## 11. 错误处理

各层定义自己的错误类型，`memory` crate 统一转换：

```
FrameAllocError ──────┐
  AllocationFailed     │   From impl
  OutOfMemory          ├──────────────→ MemoryError
                       │                  AllocationFailed
PagingError ──────────┘                  OutOfMemory
  AllocationFailed                        MapFailed
  HugePageConflict                        PageNotMapped
  PageNotMapped                           RegionOverlap
  FrameAllocFailed                        RegionNotFound
                                          RegionIdentical
```

---

## 12. unsafe 边界总结

| 位置 | unsafe 操作 | 不变量 |
|------|------------|--------|
| `frame_allocator::init()` | 空闲内存入 buddy + 预留范围构造帧 | 区间有效、互不重叠、仅调用一次 |
| `AllocatedFrames::alloc()` | 零初始化：`write_bytes(pa.to_virt(), 0, size)` | 帧刚分配，无其他引用 |
| `Table::from_paddr()` | 将 PA 转为 PTE 数组指针 | PA 指向有效、页对齐的帧 |
| `OwnedPages::as_type{_mut}()` | 从 VA 创建类型化引用 | 映射存活、偏移和对齐已验证 |
| `MmioRegion` 内部 | 对 MMIO 地址建立映射 + volatile 读写 | PA 是有效设备地址 |

注意：`OwnedPages` 自身**零 unsafe**——帧管理完全通过 Rust 所有权系统。
相比旧设计（2 处 `ManuallyDrop::take` unsafe）和 Theseus（`mem::forget` +
`unsafe from_unmapped_range`），这是一个显著的简化。
