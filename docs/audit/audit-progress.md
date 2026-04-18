# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R3 — 内存子系统（第二轮精简完成）
**下一个目标**:
1. **B 方向调查**：根 `Cargo.toml` 添加 `frame_allocator` 直接依赖导致 SMP QEMU 死锁的根因（详见"未决问题"）
2. R8 README 补全（paging / tlb / heap / memory）

## 上次对话摘要

**日期**：2026-04-18

### 已完成

对内存子系统做了第二轮深度精简，提交 `8456b3540 refactor(memory): 精简未用 API + 错误扁平化（panic 优先）`（33 files, 503+/1040−）。按三条约束审查：**SAS 不需用户态、内存只映射不回收、预期外错误 fail-fast**。

**代码改动（A 方案落地）：**

- `page_table_entry`：
  - 删 `user_rw/rx/ro/rwx` 工厂（SAS 无用户态）
  - 删 `is_readable/writable/executable/user/accessed/dirty` 查询（无调用点）
  - 删 `with_writable/with_executable` builder（`set_flags` 整包替换已够用）
  - 删 `PteOps::empty`（构造全零 PTE 由帧清零保证）
  - 抽 `KERNEL_BASE`（riscv64）/ `LEAF_BASE`+`KERNEL_NORMAL`（aarch64）基础常量，各 factory 从"每条明文列所有位"变为"基础位 | 差异位"
- 错误扁平化：
  - `FrameAllocError` 仅保留 `OutOfMemory`；删 `AllocationFailed`
  - 删 `crates/paging/src/error.rs`（`PagingError` 整体）
  - 删 `crates/memory/src/error.rs`（`MemoryError` 整体）+ 所有 `From<X>` 转换
  - `map_mmio` / `identity_map_range` / `update_pte` / `PageTable::create` / `Arch::map_early_mmio` 失败即 panic，签名不再返回 `Result`
- `PageTable`：
  - 删 `create_pte` 公开接口（生产零调用）
  - `walk_create_and_write` 合并 TOCTOU re-check 与 fast-path 重复逻辑
- `OwnedPages`：
  - `batch_update_flags` 用 `for` 循环（避 `needless_for_each`）+ TODO 标记单次 walk 批量优化方向
  - `Drop` 复用 `self.size()` 替代手算 `page_count*PAGE_SIZE`
- `frame_allocator`：
  - 移除 `FrameAllocatorInner` + `initialized` 标志（由调用方 `call_once` 担保）
  - `init` 内部拆 `init_buddy` + `claim_reserved`，单一对外入口不变
  - `count()` → `page_count()`（避与 `Iterator::count` 语义混淆）
- `memory_types`：
  - 删 `Page` 类型 + `VirtAddr::page_number`（生产零调用）
  - 删 `Span::overlaps`（VMA 移除后无消费者）
  - `page_frame.rs` 退出 `impl_page_or_frame!` 宏，`Frame` 直接手写
- 测试全量同步：`pte-test` 重写，`paging-test` / `frame-test` / `memory-types-test` 相应调整

**验证**：riscv64 / aarch64 双架构 20/20 系统测试通过。

### 关键决策

| # | 决策 | 状态 | ADR |
|---|------|------|-----|
| 预期外错误 fail-fast | 页表操作、MMIO 映射失败即 panic；仅运行时帧分配 OOM 走 `Result` | 已实施 | — |
| 删除 user_* / is_* / with_* / empty API | SAS 约束下均无消费者；未来需要再加回 | 已实施 | — |
| PteFlags 基础常量抽取 | 各 factory 从"每条列位"变为"基础 \| 差异"，同时保持可读性 | 已实施 | — |
| `FrameAllocatorInner` 移除 | 由 `memory::init` 的 `call_once` 单例模式担保 init 幂等 | 已实施 | — |
| 保留 `memory::frame` 等 re-export | A 方案 workaround：避免触发 Cargo workspace 死锁（见未决问题） | 已实施 | — |

### 未决设计问题

- **🚨 Cargo 依赖死锁（高优先级，B 方向待调查）**：
  - **现象**：在根 `Cargo.toml` `[dependencies]` 添加 `frame_allocator = { path = "crates/frame_allocator" }` 会导致 SMP QEMU 启动后立即死锁（QEMU 100% CPU、无串口输出）。移除该依赖后测试全绿。
  - **最小复现**：干净 branch + 仅添加这一行 + 跑 `cargo xtask test --arch riscv64 --name pte-test` → 超时不返回。
  - **当前 workaround**：保留 `crates/memory/src/lib.rs` 中 `pub use frame_allocator as frame;` 等 re-export；`src/boot.rs`、`src/device/hal.rs` 继续通过 `memory::frame::AllocatedFrames` 访问。
  - **怀疑方向**：workspace + 多测试二进制（`tests/paging-test` 自身也直接依赖 `frame_allocator`）场景下的 linker / dependency resolver 边界情况；或 `static FRAME_ALLOCATOR` 被重复链接为两个实例。
  - **建议调查路径**：
    1. `cargo tree --duplicates` 查是否有重复 crate 实例
    2. `objdump -t target/riscv64gc-unknown-none-elf/debug/simplekernel | grep FRAME_ALLOCATOR` 查符号数量
    3. `cargo rustc -- --print=link-args` 看链接命令差异
    4. GDB 挂死锁进程，看具体卡在哪条指令
    5. 最小化复现：新建空 workspace + 两个子 crate 是否能重现
  - **记录位置**：`crates/memory/src/lib.rs` 头部有详细注释，注释本身也引用本文件。
- R6 范围的锁（`dev_mgr`、`ramfs`、`fd`、`virtio_blk`、`mount_table`）锁级别仍用 `UNSPECIFIED`，待 R6 审计统一分配
- `mmap_identity`/`mmap_anonymous` 合并后的 `mmap` 接口中 `mmap_lazy` 的 `handle_page_fault` 路径未经裸机测试验证
- `batch_update_flags` 当前每页独立调用 `update_pte` 触发完整 walk，未来大映射场景可合并单次 walk —— 已在代码中留 TODO
- `MmioRegion::map` 是 `pub` 但文档写明"不要直接调用"（仅 `memory::map_mmio` 应调用）—— 当前靠注释门控，应考虑 sealed trait / token 模式或同 crate 重构

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程
- [ ] `paging/README.md` — 分页子系统文档
- [ ] `tlb/README.md` — TLB 管理文档
- [ ] `heap/README.md` — 堆分配器文档
- [ ] `memory/README.md` — 内存门面 crate 文档

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
| 2026-04-03 | R1 | 审查报告 + 实施修复（span/config/build_common/memory_types） |
| 2026-04-03 | R3 | 审查报告 + 重构实施（四态 typestate、移除 page_allocator、锁级别、幂等映射） |
| 2026-04-18 | R3 (第二轮) | 精简未用 API + 错误扁平化（commit `8456b3540`，净减 ~540 行） |
