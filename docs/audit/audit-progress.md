# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R3 — 内存子系统（ADR-013 OwnedPages 删除 + 接口精简完成）
**下一个目标**（按优先级）：
1. **DMA buffer 权限语义**：`src/device/hal.rs::dma_alloc` 未设 `PteFlags::kernel_device()`，AArch64 真机 cache 一致性风险（详见"未决问题"）
2. **TLB shootdown 接入**：`tlb::register_tlb_shootdown` 无调用者，跨核失效机制断线（详见"未决问题"）
3. R8 剩余 README：`CONTRIBUTING.md` / `CODE_OF_CONDUCT.md` / `SECURITY.md`（paging / tlb / heap / memory 已补）
4. 低优先级 review 遗留（范围 API 形状一致性、MMIO 两入口等，详见"未决问题"）

## 上次对话摘要

**日期**：2026-04-18（ADR-013）

### 已完成

**主线：删除 `OwnedPages` 抽象层（ADR-013 提议 → 已接受 → 落地）**

论证路径：
1. 对话前半做了审计驱动的内存模块注释清理（ASCII 流程图迁移到 README、删除字段级重复 doc），顺便发现 `OwnedPages` 的实际消费情况
2. 代码扫描确认 `OwnedPages` 唯一生产调用点是 `memory::init`，紧随 `mem::forget` —— Drop 分支在生产中从未触发，`set_flags` 零调用者
3. 审阅 P9 用户程序计划文档（`docs/superpowers/plans/2026-04-09-p9-elf-loader-process-model.md`）与 BusyBox 路线图：确认用户侧走独立 per-process 页表 + `UserVma { frames: Vec<AllocatedFrames> }`，**明确绕过 `OwnedPages`**（VA≠PA、map_page vs update_flags 不同原语）
4. 审阅 `src/device/hal.rs`：virtio `Hal` trait 签名 `(u64, NonNull<u8>)` 边界强制裸指针，move-only 类型传不过去——DMA 若需类型化抽象应是专用 `DmaBuffer<T>`，不复用 `OwnedPages`
5. 结论与 ADR-008 论据同构（"API 边界无用户可见面 → 删除"）

**落地提交（两个 commit）：**

1. `9d7ad44c6 refactor(memory): 删除 OwnedPages 抽象 + 精简门面（ADR-013）`（37 files, +551/-628）
   - 删除 `crates/paging/src/mapping.rs`（-133 行）
   - 新增 `PageTable::update_range_flags(va, count, flags)` 方法
   - 删除 `config::FREED_PAGE_POISON`（唯一消费者是 OwnedPages::Drop）
   - 删除 `tests/paging-test/src/mapping.rs`（6 个自证测试）+ 新增 `test_update_range_flags_batch`
   - 一并带入：对话前半的注释清理、`BOOT_STACK` 16 字节对齐修复、`frame_allocator` 直连依赖（`src/boot.rs` / `src/device/hal.rs` 去掉 `memory::frame` 间接路径）
   - 新建 ADR-013 + 4 个 crate README（memory / paging / heap / tlb）

2. `f36f347de refactor(memory): 精简 frame_allocator::init 接口 + 消除 PhysAddr→VirtAddr 冗余`（9 files, +32/-73）
   - `frame_allocator::init` 返回 `()`（原返回 `heapless::Vec<AllocatedFrames, 8>`）——消除了类型安全洞（`claim_reserved` 给出的预留帧 Drop 会污染 buddy）+ panic safety 洞 + 接口冗余三问题
   - 6 处 `VirtAddr::new(pa.as_usize())` → `pa.to_virt()`
   - 删除 `memory::heap` / `memory::tlb` 死 re-export（零外部消费者）
   - `heapless`（frame_allocator）/ `tlb`（memory）依赖同步移除

**关于 Cargo 依赖"死锁"**：原 audit-progress 记录的"根 Cargo.toml 添加 `frame_allocator` 直接依赖触发 SMP QEMU 死锁"问题在本次对话中**未复现**——直接依赖后 paging-test/table（`-smp 2`）等测试全绿。怀疑原死锁是其他因素导致，已在本次对话一并切换为直连依赖（`src/boot.rs` / `src/device/hal.rs` 用 `frame_allocator::AllocatedFrames`）。若未来再次触发 SMP 死锁需重新调查。

**验证**：riscv64 / aarch64 双架构内核构建通过；paging-test/table（9 测试）/ frame-test/alloc / heap-test / pte-test SMP QEMU 全绿；clippy / fmt clean。

### 关键决策

| # | 决策 | 状态 | ADR |
|---|------|------|-----|
| 删除 `OwnedPages` 抽象层 | 无消费者 + 与 ADR-008 论据同构；改用 `PageTable::update_range_flags` 方法 | 已实施 | [ADR-013](../decisions/013-ownedpages-necessity.md) |
| 删除 `FREED_PAGE_POISON` 常量 | 唯一消费者 OwnedPages::Drop 消失后成为孤立常量；未来如需 poison 机制再按场景引入 | 已实施 | ADR-013 |
| `frame_allocator::init` 返回 `()` | 原返回值（reserved `AllocatedFrames`）既冗余又有类型安全洞（Drop 会污染 buddy） | 已实施 | — |
| 流程图迁移 README | 原保留在 lib.rs 模块 doc 的 ASCII 流程图移至各 crate README.md；lib.rs 留一句话概述 | 已实施 | — |

### 未决设计问题（待新对话处理）

#### 🔴 高优先级：DMA buffer 权限语义缺失

**文件**：`src/device/hal.rs:37`

**现象**：`SimpleKernelHal::dma_alloc` 分配帧后 **未设置** `PteFlags::kernel_device()`，帧保留背景层 `kernel_rw` 权限。

**架构影响**：
- **RISC-V**：`kernel_device() == kernel_rw()`（无页表级缓存控制，由 PMA / Svpbmt 管理），功能上 OK
- **AArch64 真机**：`kernel_rw` = Normal Inner-Shareable Cacheable；`kernel_device` = MAIR_IDX1 (Device-nGnRnE)。两者 cache 行为**不同**，DMA buffer 走 cacheable 会导致 CPU 与设备看到的数据不一致
- **QEMU**：不模拟此差异，所以 virtio 测试通过；**真机必然出 bug**

**修复方向（ADR 待决）**：
1. 方案 A：在 `dma_alloc` 内调用 `paging::kernel_page_table().update_range_flags(va, pages, kernel_device())`，`dma_dealloc` 恢复 `kernel_rw`
2. 方案 B：为 DMA 设计专用 `DmaBuffer<T>` 类型，封装权限设置 + cache flush/invalidate + 未来 IOMMU 集成
3. 方案 C：要求 `kernel_device` 映射必须在背景层就建立（修改 `memory::init` 逻辑，DMA 区域独立识别）

方案 B 最完整但工程量大；方案 A 最快落地但不处理 cache flush。建议先做 A 保 QEMU → 真机过渡，后做 B。

**关联问题**：`PteFlagsOps::kernel_device()` 的 RISC-V 实现有 TODO 提到 Svpbmt 扩展（`crates/page_table_entry/src/riscv64.rs:85`），真机场景需要 PBMT 位设置 NC 或 IO 属性。

#### 🔴 高优先级：TLB shootdown 回调机制断线

**文件**：`crates/tlb/src/lib.rs:27`

**现象**：`register_tlb_shootdown` 有零个调用者——`TLB_SHOOTDOWN_FN: spin::Once<fn(TlbFlushRequest)>` 从未被 `call_once`，所以 `flush_tlb()` / `flush_tlb_page()` 里的跨核 IPI 路径是死代码。

**当前为什么不出 bug**：
- `PageTable::update_range_flags` 的唯一运行时调用是 `memory::init`，发生在 SMP 激活之前（从核通过 `init_smp` 激活分页时复用主核页表）
- MMIO 映射也在 SMP 激活前完成
- 运行时没有动态 PTE 修改 → 没有跨核 TLB 失效需求

**何时会出 bug**：
- P9 用户程序引入 per-process 页表 → 进程切换需要 TLB 刷新
- 任何动态 `mmap` / `mprotect` 路径
- 页回收路径（如果引入）

**修复方向（ADR 待决）**：
1. IPI 机制：需要 `src/arch/{arch}/interrupt.rs` 的中断子系统提供跨核 IPI 发送原语
2. 注册时机：在 `device_init` / 中断子系统 init 完成后调用 `tlb::register_tlb_shootdown`
3. 数据结构：`TlbFlushRequest` 通过 per-CPU mailbox 或 IPI payload 传递；目标核在 IPI handler 中执行本地 `arch::flush_tlb_page` / `flush_tlb_all`
4. 同步：发起核是否需要等待其他核 ack（影响是否需要 barrier / 跨核状态位）

**关联问题**：
- `update_range_flags` 批量更新是**非原子**的（跨页），多核竞争可能观察到中间状态——跨核 TLB 协议需要考虑此语义
- `TlbFlushRequest::All` 与 `TlbFlushRequest::Page` 的选择由 `TLB_FLUSH_THRESHOLD` 驱动，IPI 需要传递相应载荷

#### 🟡 中优先级：低优先级 review 遗留

| 编号 | 问题 | 文件 | 建议动作 |
|-----|------|------|---------|
| 5.2 | MMIO 两个公开入口（`memory::map_mmio` 做 RAM 重叠检查；`MmioRegion::map` pub 但注释写"不要调用"）| `crates/paging/src/mmio.rs:27` | 考虑 `MmioRegion::map_checked(paddr, size, ram_range)` 签名让 RAM 范围显式注入，`MmioRegion::map` 降级 `pub(crate)` |
| 5.3 | `PageTable::update_pte` 返回旧 flags 仅测试用 | `crates/paging/src/table.rs:167` | 可改 `()`——收益小，非紧急 |
| 5.4 | range API 形状不一致（`identity_map_range(start, end)` vs `update_range_flags(start, count)`）| `crates/paging/src/table.rs` | 统一为 (start, count) 或 (start, end) |
| 3.3 | `update_range_flags` 批量非原子 | `crates/paging/src/table.rs:189` | 在 doc comment 明确跨页非原子语义 |
| — | `update_range_flags` 单次 walk 批量优化 | `crates/paging/src/table.rs:185` | TODO 已标记，引入 mmap / 大映射场景时实施 |

#### 🟢 低优先级：R6 相关

- R6 范围的锁（`dev_mgr` / `ramfs` / `fd` / `virtio_blk` / `mount_table`）锁级别仍用 `UNSPECIFIED`，待 R6 审计统一分配

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程
- [x] `paging/README.md` — 分页子系统文档（ADR-013 落地时补全）
- [x] `tlb/README.md` — TLB 管理文档（同上）
- [x] `heap/README.md` — 堆分配器文档（同上）
- [x] `memory/README.md` — 内存门面 crate 文档（同上）

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
| 2026-04-03 | R1 | 审查报告 + 实施修复（span/config/build_common/memory_types） |
| 2026-04-03 | R3 | 审查报告 + 重构实施（四态 typestate、移除 page_allocator、锁级别、幂等映射） |
| 2026-04-18 | R3 (第二轮) | 精简未用 API + 错误扁平化（commit `8456b3540`，净减 ~540 行） |
| 2026-04-18 | R3 (ADR-013) | 删除 OwnedPages 抽象 + 精简 frame_allocator::init 接口 + 注释清理（commits `9d7ad44c6` / `f36f347de`，净减 ~170 行） |
