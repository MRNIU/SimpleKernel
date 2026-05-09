<!-- Copyright The SimpleKernel Contributors -->

# R3 内存层复审问题说明

> 日期：2026-05-07
>
> 范围：`memory_types`、`frame_allocator`、`page_table_entry`、`paging`、`tlb`、`heap`、`memory`、`dma` 以及相关调用方。
>
> 本文记录问题原因、可能触发路径、修复方案和当前处理状态。
> 第一组低耦合问题已在 2026-05-07 修复；第二组中的 W^X 固件映射、frame allocator hard IRQ 边界、单段 RAM fail-fast 已落地。剩余重点是运行期权限切换的未来所有权模型。
> 第三组设备/DMA 真机语义问题已独立到
> [`2026-05-07-device-dma-rdrive-tracking.md`](2026-05-07-device-dma-rdrive-tracking.md)，
> 后续可与 rdrive 集成一起评估。

## 总览

| # | 严重度 | 问题 | 主要风险 | 决策状态 |
|---|--------|------|----------|----------|
| 1 | P1 | `memory::init()` safe API 承担一次性 unsafe 前提 | 重复初始化破坏堆 / 帧分配器状态 | 已修复 |
| 2 | P1 | `update_range_flags()` 缺少并发写者证明 | 未来运行期权限切换 last-writer-wins | 当前无生产触发，未来需边界 |
| 3 | P1 | 公开 `kernel_rwx()` 打穿 W^X | 普通调用方可创建 RWX 映射 | 已修复 |
| 4 | P1 | DMA streaming `unmap` 无同步语义 | 真机 non-coherent 读旧 cache 行 | 需 ADR |
| 5 | P1 | coherent DMA 仍是普通 cacheable RAM | virtqueue ring 可见性不可靠 | 需 ADR |
| 6 | P1 | 帧分配中断语义与 heap 后端冲突 | IRQ/page fault 路径 panic 或死锁 | 已修复：禁止 hard IRQ |
| 7 | P2 | `VirtAddr::align_down_to()` 绕过 canonical 校验 | newtype 可产生非法 VA | 已修复 |
| 8 | P2 | `Frame` 页号范围未校验 | 超大页号截断成错误 PA | 已修复 |
| 9 | P2 | `frame_allocator::init()` 的 `reserved` 只记录不生效 | API 契约误导调用方 | 已修复 |
| 10 | P2 | FDT 只读取第一段 RAM | 多 bank 平台丢内存或误判 MMIO | 已修复（单段 fail-fast） |

## 修复状态

### 第一组已修复（2026-05-07）

- `VirtAddr::align_down_to()` 改为通过 `Self::new(...)` 返回结果，重新执行 canonical 校验。
- `Frame::new()` 和 `Frame + usize` 增加页号上界校验，防止页号转换为物理地址时截断。
- `memory::init()` 增加一次性 `AtomicBool` 守卫，二次调用直接 panic。
- `frame_allocator::init()` 在入 buddy 前校验 free/reserved 对齐、非零、溢出和重叠；文档明确 `reserved` 只校验和记录，不从 free 范围扣除。

对应回归测试：

- `memory-types-test/align-down-canonical-panic`
- `memory-types-test/frame-overflow-panic`
- `memory-test/double-init-panic`
- `frame-test/reserved-overlap-panic`

### 第二组已修复/收窄（2026-05-07）

- `KernelFdt::memory()` 检测到第二段 RAM region 时返回 `UnsupportedLayout`，避免静默只使用第一段。
- `PageTable::update_pte()` 收窄为 `PageTable` 内部机制函数；公开调用面只保留带 TLB 刷新的 `update_range_flags()`。当前生产调用只在启动期 `memory::init()`，没有多核同时改同一 VA 的路径；未来运行期调用仍需区间锁或 owner token。
- 删除公开 `kernel_rwx()` preset，新增 `kernel_firmware()` preset；`KernelFdt::firmware_reserved_memory()` 可解析 `/reserved-memory/firmware@...`，`xtask` 会给 QEMU 原生 DTB 注入该节点，`memory::init()` 再通过专用 `map_firmware_region()` 标记固件保留区。
- `frame_allocator` 在 `alloc_from_backend()` / `dealloc_to_backend()` 入口断言不在 hard IRQ 上下文中，匹配 heap 后端约束。

对应回归测试：

- `memory-test/fdt-multi-memory`
- `memory-test/fdt-firmware-reserved`
- `pte-test/pte-test`
- `paging-test/table`
- `frame-test/alloc-in-hardirq-panic`
- `frame-test/dealloc-in-hardirq-panic`

## 1. `memory::init()` safe API 承担一次性 unsafe 前提

位置：`crates/memory/src/init.rs:7-9`

### 问题原因

`memory::init()` 是公开 safe 函数，但内部直接调用：

- `heap_crate::init()`：Safety 要求在任何堆分配之前且只调用一次。
- `frame_allocator::init()`：Safety 要求空闲范围有效且只调用一次。

当前一次性约束只写在注释和 boot 流程里，`memory` 门面没有运行时状态来阻止二次调用。Rust 中 safe API 的含义是调用方不需要额外维护 unsafe 不变量；这里 safe 外观与内部约束不一致。

### 可能触发路径

```text
boot::kernel_init()
  -> memory::init()
     -> heap::init()
     -> frame_allocator::init()
```

正常启动只调用一次，不触发问题。

潜在触发路径：

- 新增系统测试直接调用 `memory::init()`，而 test harness 已经在 `kernel_init()` 中调用过。
- 未来把 `InitLevel::Memory` 和更高 level 在同一 QEMU 实例中组合运行。
- 错误恢复 / kexec / 重新初始化实验路径尝试再次初始化内存子系统。

二次调用的后果：

- `heap::init()` 可能把同一 bootstrap heap 重复加入 allocator。
- `frame_allocator::init()` 可能把同一 free range 重复加入 buddy。
- 后续分配会出现重复分配、重叠所有权或 allocator 内部状态损坏。

### 修复方案

最小修复：

1. 在 `memory::init()` 入口增加 `AtomicBool` 或 `spin::Once<()>` 守卫。
2. 第一次调用继续执行初始化。
3. 第二次调用 fail-fast panic，错误信息包含“memory::init called more than once”。
4. 补一个 should-panic 测试；如果难以在现有裸机测试中二次初始化，可先补 host 侧或拆小单元测试。

备选方案：

| 方案 | 优点 | 缺点 | 备注 |
|------|------|------|------|
| safe API + 一次性守卫 | 调用方简单；门面自证不变量 | 多一个全局状态 | 更符合 `memory` 策略层职责 |
| 改成 `unsafe fn init()` | 暴露真实前提 | 所有调用方都要承担 unsafe；仍无法防误调 | 不解决运行时重复调用 |
| 用 `spin::Once` 包住完整初始化 | 语义清晰 | 初始化函数需要处理返回值和日志路径 | 可考虑 |

## 2. `update_range_flags()` 缺少并发写者证明

位置：`crates/paging/src/table.rs:208`

### 问题原因

`PageTable::update_pte()` 原本是公开 unsafe，并要求调用方保证同一 PTE 没有并发写入者。现已收窄为 `PageTable` 内部机制函数，公开调用面只保留带 TLB 刷新的 `update_range_flags()`。

当前内存模型下，生产路径没有“多核同时改同一个 VA”的场景：

- `update_range_flags()` 的生产调用集中在 `memory::init()`。
- `memory::init()` 发生在主核启动阶段；从核尚未进入运行期调度。
- 当前没有 `mprotect`、demand paging、模块热加载、运行期 DMA PTE 属性切换等调用方。

因此它现在不是一个已知可触发的并发 bug，而是一个必须保留在 API 边界上的未来风险。若未来把 `update_range_flags()` 用到运行期，它仍没有机制证明同一 VA 区间没有并发写者：

- 没有页表权限更新锁。
- 没有 range owner / mapping owner。
- 没有 stop-the-world。
- 没有 typestate token 表示该 VA 区间由当前调用方独占。

当前实现使用原子 `swap`，可避免 torn write，但不能避免两个核心对同一 PTE 并发写入时的 last-writer-wins。

### 可能触发路径

当前启动期主要路径：

```text
memory::init()
  -> paging::kernel_page_table().identity_map_range(...)
  -> update_range_flags(.text/.rodata/.data)
```

启动期单核执行，通常安全。

未来或运行期触发路径：

- DMA 后端把普通 RAM 临时切换成 DMA-safe 属性。
- 模块加载 / trampoline / hot patch 做 RW -> RX 权限翻转。
- 未来 `mprotect` / mmap / per-process 页表引入用户区权限更新。
- 两个设备或两个任务同时更新同一页或重叠区间。

可能结果：

- 后写者覆盖先写者，权限状态不确定。
- W^X 切换中短暂出现 W+X 或错误恢复为 RW。
- DMA cache/PTE 属性切换与 CPU 访问顺序不一致。

### 修复方案

短期约束：

1. `update_pte()` 已收窄为内部函数；测试改为走 `update_range_flags()`。
2. 文档明确当前生产路径只在启动期使用；新增运行期调用前必须先设计并发写者边界。
3. 若未来需要运行期权限切换，先给 `update_range_flags()` 增加全局权限更新锁，或引入更强的 range owner / mapping owner。

中期设计：

| 方案 | 优点 | 缺点 | ADR |
|------|------|------|-----|
| 全局权限更新锁 | 实现简单；适合 SAS 单页表 | 粒度粗 | 可不单独 ADR |
| 区间锁 / VMA owner | 可表达重叠区间冲突 | 需要 VMA/地址空间元数据 | 需要 |
| typestate mapping owner | Rust 语义最强 | 需要重建映射所有权模型 | 需要 |
| `unsafe fn update_range_flags` | 真实暴露前提 | 把责任外推给调用方 | 不足以防误用 |
| stop-the-world 权限切换 | 适合 W^X / icache 同步 | 成本高，需要 SMP 协议 | 需要 |

## 3. 公开 `kernel_rwx()` 打穿 W^X

位置：`crates/page_table_entry/src/lib.rs:27`

### 问题原因

`PteFlagsOps` 曾公开 `kernel_rwx()`，而 README 又声明内核 preset 遵循 W^X。旧实现中：

- RISC-V `kernel_rwx()` 包含 `READ | WRITE | EXECUTE | DIRTY`。
- AArch64 `kernel_rwx()` 对 EL1 可写且可执行，只设置 `UXN` 阻止 EL0 执行。

这让普通调用方可以通过公开 factory 创建 RWX 映射，类型层没有表达“这是危险例外”。现已删除 `kernel_rwx()`，并用 `kernel_firmware()` 表达固件保留区的专用权限语义。固件区地址优先来自 FDT `/reserved-memory/firmware@...`；QEMU 原生 DTB 没有该节点时，`xtask` 会在 dump 出来的 DTB 中注入 SimpleKernel 专用固件保留节点。

### 可能触发路径

旧生产路径未发现直接调用 `kernel_rwx()`，但潜在路径很短：

```text
任意上层模块
  -> PteFlags::kernel_rwx()
  -> PageTable::identity_map_range(...) 或 update_range_flags(...)
```

可能场景：

- boot trampoline 为了省事使用 RWX。
- 模块加载先写代码再执行，直接把页设为 RWX。
- 测试或调试代码复制该 preset 到生产路径。

### 修复方案

可选方案：

| 方案 | 优点 | 缺点 | ADR |
|------|------|------|-----|
| 删除 `kernel_rwx()` | W^X 语义最清晰 | 若有 boot 例外需要另建入口 | 已采用 |
| 收窄为 crate-private | 防止上层误用 | paging 内仍可误用 | 可直接修 |
| 改名为 `kernel_rwx_for_boot_trampoline_only` | 语义显式 | 仍保留危险能力 | 需要说明使用边界 |
| 保留但加测试白名单 | 兼容性强 | API 仍容易误用 | 不推荐单独作为最终状态 |

配套测试：

- 跨架构断言常规 preset 中不存在 W+X。
- 若保留 RWX，测试必须验证只有显式危险 API 能生成 W+X。
- README 删除“所有 preset 遵循 W^X”这种与 API 冲突的描述，或改为“常规 preset”。

## 4. DMA streaming `unmap` 无同步语义

位置：`crates/dma/src/qemu.rs:147`

### 问题原因

`raw_unmap_single()` 是空实现，`SimpleKernelHal::unshare()` 也只转发到这里。当前 QEMU identity + cache-coherent 行为下能工作，但 streaming DMA 的核心语义是：

- `DriverToDevice`：设备读取前，CPU 写入需要 clean / writeback。
- `DeviceToDriver`：设备写入后，CPU 读取前需要 invalidate。
- `Bidirectional`：两个方向都需要正确同步。

当前代码没有使用 direction 做任何同步，也没有保存 map handle 来校验 unmap 是否对应之前的 share。

### 可能触发路径

读路径：

```text
VirtIOBlk::read_blocks(...)
  -> SimpleKernelHal::share(buffer, DeviceToDriver)
  -> dma::raw_map_single(...)
  -> device writes data
  -> SimpleKernelHal::unshare(...)
  -> dma::raw_unmap_single(...)  // 空实现
  -> CPU reads buffer
```

写路径：

```text
VirtIOBlk::write_blocks(...)
  -> SimpleKernelHal::share(buffer, DriverToDevice)
  -> raw_map_single(...)  // 未 clean
  -> device reads stale memory
```

当前 `src/device/virtio.rs` 的扇区 0 读测试和 `src/fs/fatfs_adapter.rs` 的 `sector_buf` 都会经过普通 stack buffer 的 streaming DMA 路径。

### 修复方案

短期：

1. 文档继续明确 `crates/dma` 当前只承诺 QEMU identity。
2. 增加测试覆盖 QEMU 的 read/write/multi-sector/cross-page buffer，避免 raw path 退化。

真机前必须设计：

| 方案 | 优点 | 缺点 | ADR |
|------|------|------|-----|
| share clean / unshare invalidate | 接近常见 DMA API | 需要 AArch64 cache helper，RISC-V 平台语义要分情况 | 需要 |
| bounce buffer | 不依赖调用方 buffer 物理连续/cache 属性 | 有拷贝成本 | 需要 |
| 要求 I/O buffer 来自 DMA coherent allocator | 语义简单 | 上层使用成本高，普通 slice 不能直接传 | 需要 |
| 仅声明 QEMU 支持 | 实现成本低 | 阻塞真机 non-coherent | ADR-014 已是当前状态 |

## 5. coherent DMA 仍是普通 cacheable RAM

位置：`crates/dma/src/qemu.rs:49-65`

### 问题原因

`alloc_coherent()` 只做三件事：

1. `AllocatedFrames::alloc(pages)` 分配连续物理帧。
2. 通过 identity mapping 清零。
3. 把 `AllocatedFrames` 放入 `DMA_TRACKER` 保持生命周期。

它没有改变 PTE 属性，也没有为 descriptor/ring 维护 cache 一致性。AArch64 当前普通 RAM 是 Normal Write-Back cacheable；对 non-coherent 设备，CPU 和设备看到 virtqueue descriptor/avail/used ring 的时序不可靠。

### 可能触发路径

```text
VirtIOBlk::new(...)
  -> SimpleKernelHal::dma_alloc(...)
  -> dma::raw_alloc_pages(...)
  -> QemuIdentityDmaOp::alloc_coherent(...)
  -> virtqueue descriptor / avail / used ring
```

QEMU virt 环境通常不暴露问题。真机路径可能出现：

- CPU 更新 descriptor，设备读到旧值。
- 设备更新 used ring，CPU 读到旧 cache line。
- 多核 CPU 之间能同步，但设备侧不可见。

### 修复方案

候选方案：

| 方案 | 优点 | 缺点 | ADR |
|------|------|------|-----|
| 新增 `kernel_dma_coherent()` PTE flags | coherent buffer 语义集中 | 需要 AArch64 MAIR Normal-NC；RISC-V 依赖 PMA/Svpbmt | 需要 |
| 保持 cacheable，ring 操作显式 cache maintenance | 性能可控 | 需要精确插入同步点，复杂 | 需要 |
| 全部走 bounce / uncached pool | 简单可靠 | 性能损耗和内存占用 | 需要 |
| 当前 QEMU 后端保持不变，新增 real backend | 边界清晰 | 需要后端抽象 | 需要 |

配套要求：

- DMA mask/capability 进入设备模型。
- 释放 coherent buffer 时明确是否恢复 PTE 属性。
- 文档区分 MMIO Device memory 与 DMA RAM，避免复用 `kernel_device()` 表达 DMA。

## 6. 帧分配中断语义与 heap 后端冲突

位置：`crates/frame_allocator/src/alloc.rs:99-104`

### 问题原因

`frame_allocator` 使用 `buddy_system_allocator::FrameAllocator<32>`，该后端依赖集合结构管理空闲块。项目文档又说明 frame allocator 可能在 page fault / interrupt handler 中使用，并用 `SpinLockIrq` 避免同核中断重入死锁。

但是 `heap` crate 明确禁止中断上下文堆操作。若 frame backend 在 alloc/dealloc 中触发堆分配或释放，就会违反 heap 的运行时断言。

换句话说，当前锁解决了“中断重入同一把 frame lock”的问题，没有解决“frame allocator 后端会使用 heap”的上下文问题。

### 可能触发路径

潜在路径：

```text
interrupt / exception / future page fault handler
  -> AllocatedFrames::alloc(...)
  -> FRAME_ALLOCATOR.lock().alloc(...)
  -> buddy backend mutates heap-backed metadata
  -> global allocator
  -> heap::assert_not_in_irq() panic
```

释放路径也可能触发：

```text
interrupt context drops AllocatedFrames
  -> dealloc_to_backend(...)
  -> buddy backend dealloc / merge metadata
  -> heap operation
  -> panic
```

当前系统未实现 demand paging，所以旧代码触发概率低；但文档承诺已经把未来调用方引向错误方向。现已将承诺改为：hard IRQ 中禁止分配/释放物理帧，错误调用立即 panic。

### 修复方案

先定义承诺：

| 方案 | 优点 | 缺点 | ADR |
|------|------|------|-----|
| 禁止 hard IRQ 中分配/释放帧 | 与现有 heap 后端一致 | page fault 等路径需延后或使用预留池 | 可直接文档化 |
| 保留 IRQ 可分配承诺，替换 frame backend | 语义强 | 工作量大，需要 no-heap metadata | 需要 |
| 增加 per-CPU emergency frame cache | 可覆盖少量中断场景 | 容量/补充策略复杂 | 需要 |

最小修复：

1. 已在 `alloc_from_backend()` 和 `dealloc_to_backend()` 加 `assert!(!interrupt_state::is_in_interrupt())`。
2. 已修改 `frame_allocator/README.md`，删除“中断/page fault handler 可直接分配”的承诺。
3. 已新增 `frame-test/alloc-in-hardirq-panic` 和 `frame-test/dealloc-in-hardirq-panic`。
4. 后续若需要 page fault 分配，先设计应急池或非 heap frame backend。

## 7. `VirtAddr::align_down_to()` 绕过 canonical 校验

位置：`crates/memory_types/src/addr.rs:47-55`

### 问题原因

`align_down_to()` 直接返回 `Self(self.0 & !(align - 1))`，绕过了 `VirtAddr::new()` 的 canonical 地址校验。`align_up_to()` 则会调用 `Self::new(...)`。

对 `PhysAddr` 来说绕过构造也不好，但常见 align_down 不会增大地址；对 `VirtAddr` 来说，高半区规范地址的符号扩展不变量可能被大粒度对齐破坏。

### 可能触发路径

```text
let va = VirtAddr::new(high_half_addr);
let aligned = va.align_down_to(large_align);
```

如果 `large_align` 清掉了 canonical 高位附近的关键位，`aligned` 可能变成 canonical hole 中的非规范虚拟地址，但类型仍是 `VirtAddr`。

后续触发点：

- 作为页表 walk 输入。
- 转成裸指针访问。
- 参与地址范围计算。

### 修复方案

最小修复：

1. `align_down_to()` 改为 `Self::new(self.0 & !(align - 1))`。
2. 保留 `align.is_power_of_two()` assert。
3. 补 `memory-types-test`：
   - 高半区地址页对齐仍保持 canonical。
   - 高半区地址大对齐产生非 canonical 时 should-panic。
   - 非 2 的幂 align should-panic。

## 8. `Frame` 页号范围未校验

位置：`crates/memory_types/src/page_frame.rs:20-32`

### 问题原因

`Frame::new(number_4k)` 接受任意 `usize`。`start_addr()` 直接执行：

```rust
PhysAddr::new(self.number << PAGE_SIZE_BITS)
```

页号过大时，左移可能溢出或截断，再被 `PhysAddr::new()` 当作另一个较小物理地址接受。这样 `Frame` -> `PhysAddr` 不再是单射。

### 可能触发路径

```text
Frame::new(too_large)
  -> start_addr()
  -> 左移截断
  -> PhysAddr::new(truncated)
```

潜在来源：

- FDT 解析或 page count 计算错误。
- future page allocator / VMA 代码从裸 usize 构造 frame。
- 测试或调试代码绕过 PhysAddr -> Frame 的正常路径。

### 修复方案

最小修复：

1. 在 `Frame::new()` 中校验：
   - `number_4k < (1usize << (arch::PA_BITS - config::PAGE_SIZE_BITS))`
   - 需要处理 `PA_BITS == usize::BITS` 的边界。
2. 或在 `start_addr()` 用 `checked_shl` / `checked_mul(PAGE_SIZE)` 并校验 `PhysAddr`。
3. 补 `memory-types-test` 的 `frame_overflow_panic`。

设计取舍：

- 若把校验放在 `Frame::new()`，`Frame` 是强不变量类型。
- 若把校验放在 `start_addr()`，`Frame` 是轻量页号包装，错误延迟到转换时暴露。

当前 `PhysAddr` / `VirtAddr` 已经是强不变量类型，`Frame::new()` 同步校验更一致。

## 9. `frame_allocator::init()` 的 `reserved` 只记录不生效

位置：`crates/frame_allocator/src/alloc.rs:57-83`

### 问题原因

函数文档写的是“校验并记录预留范围”，Safety 写着：

- 所有范围有效、页对齐、互不重叠。
- free 范围和 reserved 范围不得重叠。

但实现只做：

- reserved start 页对齐检查。
- count 非零检查。
- 打印日志。

它不会：

- 检查 `start + count * PAGE_SIZE` 溢出。
- 检查 reserved 之间互相重叠。
- 检查 reserved 与 free 是否重叠。
- 从 buddy free pool 中扣除 reserved。

当前调用方 `memory::init()` 把 `free_start` 放在 kernel 之后，所以 kernel reserved 段天然不在 free pool 中；这让 bug 暂时不触发，但 API 语义是误导性的。

### 可能触发路径

未来调用方可能这样使用：

```text
frame_allocator::init(ram_start, ram_size, reserved_ranges)
```

如果 `reserved_ranges` 位于 `[ram_start, ram_start + ram_size)` 内，调用方会以为 reserved 已被扣除；实际 buddy 仍可分配这些帧。

后果：

- 固件区、设备保留区、内核镜像区可能被普通分配覆盖。
- RAII 所有权认为自己独占了被 reserved 使用的物理帧。

### 修复方案

可选方案：

| 方案 | 优点 | 缺点 |
|------|------|------|
| 删除 `reserved` 参数 | API 与当前真实行为一致 | 调用方失去统一校验入口 |
| 改成完整校验但不扣除 | 能发现错误输入 | 名字仍可能误导 |
| 支持从 free pool 扣除 reserved | 符合文档直觉 | 需要 backend 支持 remove/split |
| 改为接收已经扣除 reserved 的 free spans | 机制清晰 | 初始化调用方更复杂 |

最小修复：

1. 如果短期不扣除 reserved，重命名参数或删除参数。
2. 至少补 end overflow / overlap / free overlap 的 fail-fast 校验。
3. README 明确：当前 free range 必须已排除所有 reserved 区域。

## 10. FDT 只读取第一段 RAM

位置：`src/fdt.rs:90-101`

状态：已采用短期 fail-fast 方案；完整多 bank RAM 支持仍是长期设计项。

### 问题原因

`FdtReader::memory()` 调用 `memory.reg().iter()` 后只取 `regions.next()`，返回第一个 `(address, len)`。`MemoryInfo` 也只包含单个 `physical_memory_addr` 和 `physical_memory_size`。

因此整个内存子系统当前只建模一段 RAM：

- frame allocator 只加入第一段后 kernel 的 free range。
- page table 只 identity-map 第一段 RAM。
- `MmioRegion::map()` 只用单个 RAM span 做 overlap 检查。

### 可能触发路径

多 memory bank FDT：

```dts
memory {
  reg = <bank0_start bank0_size bank1_start bank1_size>;
};
```

当前解析结果：

```text
FdtReader::memory()
  -> 只返回 bank0
  -> bank1 被静默忽略
```

后果：

- bank1 不加入 frame allocator，内存容量变小。
- bank1 不建立 RAM identity mapping。
- 如果某个 MMIO 描述符落在 bank1，`MmioRegion::map()` 的 overlap 检查看不到它，可能把真实 RAM 映射成 Device memory。

### 修复方案

短期 fail-fast：

1. 继续只支持单段 RAM。
2. `FdtReader::memory()` 检测第二个 region，如果存在则返回错误。
3. 错误日志列出“当前只支持单段 RAM”和前两段 region 的地址 / 大小。

长期支持：

1. `MemoryInfo` 改为固定容量 RAM region 列表，例如 `heapless::Vec<MemoryRegion, N>`。
2. `memory::init()` 对每段 RAM 建立 mapping，并把 kernel 所在 bank 做 reserved/free 切分。
3. `MmioRegion::map()` 遍历所有 RAM span 做 overlap 检查。
4. 文档补多 bank 初始化时序图。

## 建议修复顺序

### 第一组：低耦合、可先补测试

1. `VirtAddr::align_down_to()` canonical 校验。
2. `Frame::new()` 页号范围校验。
3. `memory::init()` 一次性守卫。
4. `frame_allocator::init()` 的 `reserved` API 语义收敛。

这些问题不需要先做 ADR，适合用小提交快速降低底层不变量风险。

### 第二组：剩余设计边界

1. `update_range_flags()` 若进入运行期路径，需要并发写者证明。
2. 完整多 bank RAM 支持。

`kernel_rwx()` 与 W^X 策略、固件 reserved-memory 解析、frame allocator hard IRQ 禁止、单段 RAM fail-fast 已先按窄口径修复。剩余项会影响 API 形状或未来架构，应先讨论并按需写 ADR。

### 第三组：真机前必须关闭

跟踪入口：
[`2026-05-07-device-dma-rdrive-tracking.md`](2026-05-07-device-dma-rdrive-tracking.md)。

1. streaming DMA cache maintenance / bounce buffer。
2. coherent DMA 的 PTE/cache 语义。
3. DMA mask / capability / IOMMU 或 bounce buffer 策略。
4. RISC-V `kernel_device()` 的 PMA/Svpbmt 边界。

这些不阻塞当前 QEMU identity 路径，但阻塞 non-coherent 真机声明。

## 测试补充清单

| 范围 | 测试 |
|------|------|
| `memory_types` | `VirtAddr::align_down_to` 高半区大对齐；非法 align；`Frame` overflow |
| `frame_allocator` | 非 2 的幂 count；非法 reserved；重复 init；中断上下文禁止分配 |
| `memory` | 二次 init fail-fast；MMIO size=0 / RAM overlap / offset 越界 / 寄存器未对齐 |
| `paging/tlb` | update 权限并发或锁保护；TLB shootdown ack 统计；远端核真实访问 probe |
| `page_table_entry` | 常规 preset 不允许 W+X；固件 preset 不暴露通用 RWX |
| `dma/device` | read/write；multi-sector；跨页 buffer；dealloc mismatch；zero pages；mask failure |
