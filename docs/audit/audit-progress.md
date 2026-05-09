<!-- Copyright The SimpleKernel Contributors -->

# 审计进度

> 此文件由 AI 在每次审计对话结束时自动更新，用于跨对话传递上下文。
> 请勿手动编辑，除非需要纠正 AI 的记录。

## 当前状态

**当前 Phase**: R4 — 架构层第二阶段低耦合修复已完成。`R4-09 should_panic harness/xtask 假阳性`
已通过串口 success sentinel 和 xtask 输出判定收口；`R4-03/R4-07/R4-08/R4-10` 已修复；
`R4-15` 的 fail-fast 部分已修复，absolute deadline / 漂移语义保留到 timer-preemption 设计阶段。
**下一个目标**（按优先级）：
1. **R4 时序与跨核协议设计**：R4-01/R4-02 timer 与调度边界、R4-04/R4-12 CPU topology 与 SGI、R4-05 TLB shootdown 协议、R4-11 PLIC context 与启用顺序、R4-13 discovered/online core count、R4-14 timekeeper core。
2. **R4/R5 timer 语义剩余项**：R4-15 absolute deadline / tick 漂移语义，需与 preemption 闭环一起定。
3. **R3 遗留设计跟踪**：`PageTable::update_range_flags()` 若进入运行期路径，需要并发写者证明；完整多 bank RAM、真机设备/DMA 语义继续按 `docs/audit/2026-05-07-device-dma-rdrive-tracking.md` 跟踪。

验证计划：后续设计边界改动仍需先补目标回归测试，再按变更面执行
`cargo fmt --all -- --check`、`cargo xtask check --arch riscv64`、
`cargo xtask check --arch aarch64`。涉及 QEMU 的命令必须使用 30 秒超时并在
超时后清理残留 `qemu-system` 进程。

验证结果（2026-05-08 R4 第一阶段测试可信度修复）：容器 `simplekernel-dev`
内 `cargo fmt --all -- --check`、`cargo test -p xtask`、`cargo xtask check --arch riscv64`、
`cargo xtask check --arch aarch64` 通过（保留既有 warning）。新增 xtask 单测覆盖：
QEMU 进程状态成功但缺少 success sentinel 时必须失败、normal `TEST OK` 成功、
`should_panic` 的 `SHOULD_PANIC OK` 成功、timeout 即使带 success sentinel 也失败。
RISC-V QEMU 30 秒超时下定点验证通过：`memory-types-test/codec`、
`memory-types-test/align-down-canonical-panic`、`panic-test`、`device-test`。
随后运行 `cargo xtask test --arch riscv64 --timeout 30`，30 个独立测试全部通过
（30 passed, 0 failed, 0 timed out）。另用 AArch64 目标构建 `device-test` 和
`memory-types-test/align-down-canonical-panic`，确认 test harness normal / should_panic
路径在 AArch64 也能编译。第一次全量 RISC-V 回归曾暴露 `device-test` 的裸串口
`TEST OK` 被从核日志穿插打碎，最终改为通过 `log` 后端输出 success sentinel，
由 console lock 保证单条 sentinel 不被并发日志破坏。提交前复查 warning 时，
清理了 `tests/test_harness` 中新触发的 `unused feature: alloc_error_handler`；
复跑验证后，本轮改动文件不再产生新的编译 warning，剩余 warning 均来自既有内核路径。

验证结果（2026-05-09 R4 第二阶段低耦合修复）：容器 `simplekernel-dev`
内 `cargo fmt --all -- --check`、`cargo xtask check --arch riscv64`、
`cargo xtask check --arch aarch64` 通过（保留既有 warning）。新增 `arch-test`
覆盖 `KernelStack` 16 字节栈顶对齐和 `checked_tick_interval()` 非零 interval 合约。
RISC-V QEMU 30 秒超时下 `cargo xtask test --arch riscv64 --name arch-test --timeout 30`
通过，覆盖 Full 初始化和从核启动路径。AArch64 首次验证暴露 QEMU `cortex-a72`
仅报告 44-bit PARange，直接断言硬件必须支持编译期 `PA_BITS=48` 会导致 MMU 启用前 panic；
后续决策改为 SimpleKernel AArch64 当前只支持 48-bit PA，因此保留该 fail-fast 语义并删去
动态收窄代码。当前 QEMU `cortex-a72` 上 `cargo xtask test --arch aarch64 --name arch-test --timeout 30`
应输出明确 panic；若要恢复 AArch64 QEMU 通过，需要调整 QEMU CPU/平台或重新讨论动态收窄策略。

验证结果（2026-05-08 R4 复审文档化）：本轮只新增审计文档并更新进度文件，
未修改实现代码，未运行构建或 QEMU 系统测试。R4 问题详情见
`docs/audit/2026-05-08-r4-architecture-review-findings.md`。

验证结果（2026-05-07 第二组修复）：`cargo fmt --all -- --check`、
`cargo xtask check --arch riscv64`、`cargo xtask check --arch aarch64` 通过
（保留既有 warning）。RISC-V QEMU 30 秒超时下运行并通过：
`memory-test/fdt-multi-memory`、`memory-test/fdt-firmware-reserved`、
`pte-test/pte-test`、`paging-test/table`、`frame-test/alloc-in-hardirq-panic`、
`frame-test/dealloc-in-hardirq-panic`。其中 `fdt-firmware-reserved` 验证
`KernelFdt::firmware_reserved_memory()` 可解析 `/reserved-memory/firmware@...`；
`xtask` 会给 QEMU 原生 DTB 注入该节点，启动日志可见
`FirmwareReserved: addr=0x80000000, size=0x200000`。`PageTable::update_pte()`
已收窄为内部机制函数，删除公开 `kernel_rwx()` preset，新增 `kernel_firmware()`；
frame allocator 后端已在 hard IRQ 上下文分配/释放时 fail-fast。

验证结果（2026-05-08 R3 文档与 warning 收口）：容器 `simplekernel-dev`
内 `cargo fmt --all -- --check`、`cargo xtask check --arch riscv64`、
`cargo xtask check --arch aarch64` 通过。当前 R3 内存 crate 本身无编译 warning；
`src/fdt.rs` 中与内存初始化相邻的 3 个过期 `#[expect(dead_code)]` warning 已移除。

验证结果（2026-05-07 第一组修复）：`cargo fmt --all -- --check`、
`cargo xtask check --arch riscv64`、`cargo xtask check --arch aarch64` 通过
（保留既有 warning）。RISC-V QEMU 30 秒超时下运行并通过：
`memory-types-test/align-down-canonical-panic`、`memory-types-test/frame-overflow-panic`、
`memory-test/double-init-panic`、`frame-test/reserved-overlap-panic`、
`memory-types-test/codec`、`frame-test/alloc`。其中 should_panic 测试额外 grep
了串口中的 `SHOULD_PANIC OK` 和具体 panic 文案，因为当前 RISC-V harness
不会把 should_panic 未触发转换成非零退出码。

## 上次对话摘要

**日期**：2026-05-09（R4 第二阶段低耦合修复）

### 已完成

本轮按 `docs/audit/2026-05-08-r4-architecture-review-findings.md` 的第二阶段处理顺序，
完成 R4 低耦合修复切片：

1. RISC-V `_boot` 无条件初始化 `gp`，从核启动不再因 `opaque=0` 跳过全局指针设置。
2. `KernelStack` 改为显式 16 字节对齐分配，`top()` 保留 ABI 对齐断言。
3. `ArchOps::dtb_addr()` 改为 `unsafe fn`，AArch64 boot args 解析补齐 `# Safety` 边界和 `argc < 3` 防护。
4. AArch64 `TCR_EL1.IPS` 固定为 48-bit PA，启动期检查 `ID_AA64MMFR0_EL1.PARange`，不匹配则 fail-fast。
5. timer interval 统一走 `checked_tick_interval()`，RISC-V `set_timer` 失败改为携带 deadline/error/value 的 fail-fast。

### 关键结论

| # | 结论 | 状态 | ADR |
|---|------|------|-----|
| R4-03 RISC-V 从核 `gp` 初始化 | 已改为所有 hart 无条件初始化 `gp` | 已修复 | — |
| R4-07 任务栈 16 字节对齐 | 已通过显式 `Layout` 分配和 `arch-test` 回归覆盖 | 已修复 | — |
| R4-08 `dtb_addr` unsafe 边界 | 已把裸 boot args 解析移入显式 unsafe 契约 | 已修复 | — |
| R4-10 AArch64 `TCR_EL1.IPS` | 当前只支持 48-bit PA；QEMU `cortex-a72` 44-bit PA 会按设计在 MMU 启用前 panic | 已修复 | — |
| R4-15 timer fail-fast | 零 interval / SBI timer 失败已 fail-fast；absolute deadline / 漂移语义仍待 timer-preemption 设计 | 部分完成 | 待定 |

### 下一步

进入 R4 时序与跨核协议设计：R4-01/R4-02 timer 与调度边界、R4-04/R4-12 CPU topology 与 SGI、
R4-05 TLB shootdown 协议、R4-11 PLIC context 与启用顺序、R4-13 discovered/online core count、
R4-14 timekeeper core。

---

**日期**：2026-05-08（R4 第一阶段测试可信度修复）

### 已完成

本轮按 `docs/audit/2026-05-08-r4-architecture-review-findings.md` 的处理顺序，
先修复 R4-09 `should_panic` harness/xtask 假阳性问题：

1. normal `test_main!` 测试成功后输出 `TEST OK` sentinel。
2. should_panic 和手写 `panic-test` 的成功路径统一输出 `SHOULD_PANIC OK` sentinel。
3. `cargo xtask test --name` 与全量测试均改用捕获输出路径，并由 xtask 同时检查 QEMU 进程状态、
   timeout、失败 sentinel 和 success sentinel。
4. success sentinel 改用 `log` 后端输出，避免 SMP 从核日志与裸串口 sentinel 交错导致误判。

### 关键结论

| # | 结论 | 状态 | ADR |
|---|------|------|-----|
| R4-09 `should_panic` harness 假阳性 | 已通过统一 success sentinel + xtask 输出判定修复 | 已修复 | — |
| QEMU guest `exit_qemu(code)` 仍不应作为唯一可信信号 | xtask 现在要求串口 success sentinel；缺失即失败 | 已收口 | — |
| SMP 日志可打碎裸串口 sentinel | success sentinel 必须走日志锁或等价原子输出路径 | 已修复 | — |
| R4-03/R4-07/R4-08/R4-10/R4-15 | 下一组低耦合修复切片 | 待修复 | 部分待定 |

### 下一步

进入 R4 低耦合修复切片：优先处理 R4-03 RISC-V 从核 `gp` 初始化、R4-07 任务栈 16 字节对齐、
R4-08 `ArchOps::dtb_addr()` unsafe 边界、R4-10 AArch64 `TCR_EL1.IPS`、R4-15 timer fail-fast。

---

**日期**：2026-05-08（R4 架构层复审文档化）

### 已完成

本轮将 R4 架构层复审结论固化为
`docs/audit/2026-05-08-r4-architecture-review-findings.md`，覆盖
`src/arch/`、`src/boot.rs`、`src/main.rs`、`src/timer.rs`、
`src/tlb_shootdown.rs`、`src/fdt.rs`、`src/task/` 中与架构层直接耦合的入口，
以及 `tests/test_harness/` / `xtask/src/qemu.rs` 中影响 R4 验证可信度的路径。

文档按 `R4-01` 到 `R4-17` 编号记录每个问题的原因、可能触发路径、修复方案和验证方向。
本轮只做文档和进度更新，未修改实现代码。

### 关键结论

| # | 结论 | 状态 | ADR |
|---|------|------|-----|
| R4-09 `should_panic` harness 假阳性会污染后续回归 | 应优先修复测试可信度 | 待修复 | — |
| R4-03/R4-07/R4-08/R4-10/R4-15 属于低耦合启动/ABI/fail-fast 切片 | 可先分批落地 | 待修复 | 部分待定 |
| R4-01/R4-02 timer 与调度边界跨 R4/R5 | 需要明确 post-IRQ preempt 或协作式调度语义 | 待设计 | 待定 |
| R4-04/R4-12/R4-13 暴露 CPU topology、logical id 和 online mask 模型缺口 | 需要统一拓扑模型 | 待设计 | 待定 |
| R4-05 TLB shootdown 需要发布屏障、等待上下文约束和远端 ack 测试 | 需要协议设计 | 待设计 | 待定 |
| R4-06 RISC-V 浮点状态保存/禁用策略未定 | 影响 ABI 和上下文切换 | 待设计 | 待定 |

### 下一步

先从 R4-09 `should_panic` harness/xtask 假阳性修复开始；测试可信度补齐后，
再进入 R4 低耦合修复切片和 timer/SMP/TLB 的设计讨论。

---

**日期**：2026-05-07（第一组低耦合内存不变量修复）

### 已完成

**主线：修复 R3 内存层第一组可直接修的问题**

本轮从 `docs/audit/2026-05-07-r3-memory-review-findings.md` 的“第一组”开始，
按测试先行修复了四个低耦合不变量问题：

1. `VirtAddr::align_down_to()` 改为重新经过 `Self::new(...)`，防止高半区大粒度对齐生成 canonical hole 地址。
2. `Frame::new()` 增加页号上界校验，`Frame + usize` 同步防止越过 `arch::PA_BITS` 可表示范围。
3. `memory::init()` 入口增加 `AtomicBool` 一次性守卫，二次调用直接 fail-fast，不再进入 heap/frame allocator 内部损坏路径。
4. `frame_allocator::init()` 在入 buddy 前校验 free/reserved 页对齐、非零、溢出和重叠；文档明确 `reserved` 只校验和记录，不从 free 范围扣除。

### 关键结论

| # | 结论 | 状态 | ADR |
|---|------|------|-----|
| `memory::init()` 是 safe public API，但包住 heap/frame allocator 的“一次性 unsafe”前提 | 已加一次性 fail-fast 守卫 | 已修复 | — |
| `update_range_flags()` safe wrapper 没有兑现运行期同页无并发写者前提 | 当前生产只在启动期 `memory::init()` 使用，不存在多核同时改同一 VA；未来运行期权限 / DMA 属性切换仍需要锁、token 或 unsafe 边界 | 待未来设计 | 待定 |
| `kernel_rwx()` 公开存在，与 README 的 W^X 约束冲突 | 已删除公开 `kernel_rwx()`；FDT reserved-memory / xtask 注入提供固件区来源，`kernel_firmware()` + `map_firmware_region()` 表达固件保留区 | 已修复 | — |
| DMA QEMU identity 后端仍无真机 cache sync / coherent PTE / mask 语义 | 与 ADR-014 一致，已独立到 `docs/audit/2026-05-07-device-dma-rdrive-tracking.md` 跟踪；硬件/rdrive 集成前必须继续设计 | 待设计 | ADR-014 后续 |
| `Frame` / `VirtAddr::align_down_to` 有 newtype 不变量漏洞 | 已补 QEMU should_panic 回归并修复 | 已修复 | — |
| `frame_allocator::init reserved` 只记录不生效，API 契约误导 | 已收敛为“校验 + 记录”，要求 caller 预先排除 free 范围 | 已修复 | — |
| `frame_allocator` hard IRQ 语义与 heap 后端冲突 | 已明确禁止 hard IRQ 中分配/释放物理帧，并在后端入口 assert | 已修复 | — |
| RISC-V MMIO、AArch64 段边界、多段 RAM、TLB shootdown 测试均有语义或测试缺口 | 需要分拆为设计讨论和测试任务 | 待讨论 | 待定 |

### 验证

- `cargo fmt --all -- --check` 通过。
- `cargo xtask check --arch riscv64` / `cargo xtask check --arch aarch64` 通过，保留既有 warning。
- 30 秒超时运行 RISC-V QEMU 回归：`align-down-canonical-panic`、`frame-overflow-panic`、`double-init-panic`、`reserved-overlap-panic` 均打印预期 `SHOULD_PANIC OK` 和具体 panic 文案。
- 30 秒超时运行 RISC-V 正常路径：`memory-types-test/codec`、`frame-test/alloc` 通过。

### 下一步

进入第二组剩余设计讨论：运行期权限切换所有权 / 多段 RAM 支持边界。
另需优先修复 should_panic harness
的退出码/失败判定，否则未触发 panic 的测试可能被 xtask 误判为通过。

---

### 上轮摘要（2026-04-30 DMA / VirtIO HAL 权限语义审计）

### 已完成

**主线：继续 R3 未决高优先级项，审计 DMA buffer 权限语义**

本轮只做审计报告和设计分歧整理，未修改代码。结论比旧记录更细：

1. **`dma_alloc` coherent buffer 仍保留背景层 `kernel_rw`**
   - `SimpleKernelHal::dma_alloc` 分配 `AllocatedFrames` 后只清零并放入 `DMA_TRACKER`
   - 未调用 `PageTable::update_range_flags(..., PteFlags::kernel_device())` 或任何 DMA 专用权限 factory
   - AArch64 下背景层 `kernel_rw` 是 Normal Write-Back cacheable；真机非 coherent DMA 会看到旧数据或写回丢失
2. **`share/unshare` streaming buffer 完全没有 cache maintenance**
   - `VirtIOBlk::read_blocks` / `write_blocks` 会把普通栈/堆 buffer 交给 virtqueue
   - `virtio-drivers` 通过 `Hal::share` / `Hal::unshare` 暴露流式 DMA 同步点
   - 当前实现仅 `VA -> PA`，忽略 `BufferDirection`，没有 DriverToDevice clean，也没有 DeviceToDriver invalidate
   - 这比 `dma_alloc` 更直接影响 FAT/块设备读写：`src/device/virtio.rs` 的扇区 0 读测试和 `src/fs/fatfs_adapter.rs` 的 `sector_buf` 都是普通 cacheable buffer
3. **`kernel_device()` 语义是 MMIO，不应直接等同 DMA RAM**
   - AArch64 当前 `kernel_device()` 使用 MAIR_IDX1 Device-nGnRnE，适合寄存器 MMIO
   - DMA buffer 是 RAM，被 CPU 正常读写；把 RAM 映射成 Device memory 虽可避开 cache，但会引入强排序/非聚合/非重排语义和潜在访问限制
   - 更精确的方向是新增 `kernel_dma_coherent()` / Normal Non-Cacheable MAIR 属性，或保留 cacheable 并在 streaming map/unmap 做 clean/invalidate
4. **旧记录中的 TLB shootdown 状态已过期**
   - 最新代码已新增 `src/tlb_shootdown.rs`，`kernel_init()` 在 `Arch::init_interrupt()` 后调用 `tlb_shootdown::init_primary()`
   - `update_range_flags` 现在会通过 `TlbFlushGuard` 触发本核 flush + 已在线核心 shootdown
   - 当前 `device_init()` 在 `wake_secondary_cores()` 前执行，所以启动期 VirtIO queue DMA 分配发生在从核上线前；但未来热插拔/新队列/运行期 DMA 分配仍需正确处理跨核属性切换

**依赖检查**：
- `virtio-drivers = 0.13.0`，`cargo search` 显示 0.13.0 仍是 crates.io 最新
- `aarch64-cpu` crates.io 最新为 11.2.0，但项目仍使用 fork 分支获取 TLBI 封装；这与本轮 DMA 议题无直接冲突
- 候选 crate：
  - `aarch64-cpu-ext = 0.1.4`：no_std，提供 AArch64 cache clean/invalidate range helper
  - `dma-api = 0.7.2`：no_std-ish DMA 抽象，含 coherent/map_single/cache sync 模型，但引入一套外部 OSAL，不宜在未决策前替换现有 HAL

### 关键结论

| # | 结论 | 状态 | ADR |
|---|------|------|-----|
| DMA buffer 不是 MMIO | DMA 分配的是设备可访问 RAM，不是寄存器区；用 `MmioRegion` 语义解释不成立 | 已确认 | — |
| `share/unshare` cache 同步缺失 | 当前 VirtIO 数据 buffer 是普通 cacheable 内存，缺少 clean/invalidate 是真机正确性问题 | 待设计 | 待定 |
| coherent DMA PTE 属性缺失 | `dma_alloc` 返回的 virtqueue / used ring 等 coherent 区域未设置 DMA-safe 属性 | 待设计 | 待定 |
| `kernel_device()` 不宜复用为 DMA RAM 的唯一语义 | Device-nGnRnE 是 MMIO 属性；DMA RAM 可能需要 Normal-NC 或 explicit cache maintenance | 待设计 | 待定 |
| TLB shootdown 已接入 | 旧记录中"零调用者"已被后续提交修正；后续只需测试和协议审查 | 已更新认知 | — |

### 未决设计问题（待讨论）

#### 🔴 高优先级：VirtIO streaming DMA cache maintenance

**文件**：`src/device/hal.rs:89` / `src/device/hal.rs:99`

**现象**：`share()` / `unshare()` 忽略 `BufferDirection`，只做 `VirtAddr::to_phys()`。`virtio-drivers` 对普通 I/O buffer 的同步点就在这两个方法里。

**备选方案（ADR 待决）**：
1. 方案 A：在 `share()` 对 `DriverToDevice|Both` 执行 clean，在 `unshare()` 对 `DeviceToDriver|Both` 执行 invalidate
2. 方案 B：为 streaming buffer 建立临时 non-cacheable alias / bounce buffer，`share` 拷贝出去，`unshare` 拷贝回来
3. 方案 C：要求所有块设备 I/O buffer 来自 DMA coherent allocator，普通栈/堆 buffer 不允许直接提交给 VirtIO
4. 方案 D：保持现状，仅支持 coherent QEMU/虚拟平台

#### 🔴 高优先级：DMA coherent allocation 的 PTE 属性

**文件**：`src/device/hal.rs:37`

**现象**：`dma_alloc()` 返回的 virtqueue DMA 区域保留 `kernel_rw`，AArch64 为 Normal Write-Back cacheable。

**备选方案（ADR 待决）**：
1. 方案 A：新增 `PteFlagsOps::kernel_dma_coherent()`，AArch64 使用 Normal Non-Cacheable MAIR 属性，RISC-V 暂时等同 `kernel_rw()`
2. 方案 B：短期复用 `kernel_device()` 改 PTE，快速消除 cacheable 行为，但明确这是过渡方案
3. 方案 C：保持 PTE 为 cacheable，在 `dma_alloc` / descriptor 更新路径显式做 cache maintenance
4. 方案 D：引入 `DmaBuffer<T>` / DMA allocator 层，封装权限、cache sync、DMA mask、未来 IOMMU

#### 🟡 中优先级：AArch64 cache maintenance helper 归属

**备选方案（ADR 待决）**：
1. 方案 A：在 `src/arch/aarch64/` 手写 `dc cvac` / `dc ivac` / `dc civac` range helper，暴露给 HAL
2. 方案 B：引入 `aarch64-cpu-ext` 仅复用 cache helper
3. 方案 C：等上游 `aarch64-cpu` / fork 增加 cache helper 后统一使用同一 crate

### 验证

- 本轮无代码变更，未运行构建或 QEMU 系统测试
- 执行了源码审计、`cargo search` / `cargo info` 依赖版本查询

### 下一步

等待项目作者选择 DMA 方案；确认后再实施代码修改、补测试，并更新 ADR / 文档。

---

### 上轮摘要（2026-04-18 低优先级 review 遗留清理）

### 已完成

**主线：清理 R3 审计低优先级遗留 + 两个高优先级问题重新定性**

四项低优先级 review 遗留一次性清掉：

1. **5.2 MMIO 单一入口改造**（`crates/paging/src/mmio.rs` + `crates/memory/src/lib.rs`）
   - `MmioRegion::map` 改为接收 `ram_range: Range<PhysAddr>` 参数，RAM 重叠校验**下沉到 paging 层**——paging 自持完整不变量检查
   - `memory::map_mmio` 从 `MEMORY_INFO` 读 RAM 范围后构造 `ram_range` 转发——保留门面语义（调用方不需要知道 `MEMORY_INFO` 在哪）
   - 消除"两个公开入口"（`MmioRegion::map` 不再标注"不要直接调用"）
2. **5.3 `PageTable::update_pte` 返回 `()`**——原返回旧 flags 仅一个测试消费，测试改为 `update_pte` 后直接 `get_mapping` 校验新 flags
3. **5.4 range API 形状差异**（`identity_map_range(start_pa, end_pa)` vs `update_range_flags(va, page_count)`）—— **不强制统一**，在两个 API doc comment 分别说明形状选择理由：
   - `identity_map_range` 使用 byte range：调用方（FDT / MMIO 描述符）天然持有字节起止，内部 `align_*` 吸收差异
   - `update_range_flags` 使用 page count：调用方（DMA 分配器 / 未来 mprotect）天然以页为单位
4. **3.3 `update_range_flags` 跨页非原子**——doc comment 补充"跨页非原子"节，明确其他核可观察到区间部分新 / 部分旧 flags 的中间状态

**两个高优先级问题的概念澄清**（对话前半）：

- **DMA vs MMIO**：`SimpleKernelHal::dma_alloc` 分配的是**设备 DMA 访问的 RAM**（virtqueue / 数据 buffer），不是 MMIO 寄存器——两者是不同概念。DMA 权限问题**真实存在**，QEMU 不模拟 CPU cache 一致性，所以测试绿但真机会爆。保留在未决问题清单。
- **TLB shootdown**：改 flag **本核**必须刷 TLB（`TlbFlushGuard` 已做）；**跨核广播**当前不需要，因为所有 PTE 修改都发生在 SMP 激活之前（`memory::init` 时其他核尚未启动，从核 `init_smp` 复用主核已经 finalize 的页表）。此问题**降优先级**，等 P9 per-process 页表 / mmap / 页回收再接入，届时配合 R4 中断子系统的 IPI 原语。

**验证**：riscv64 + aarch64 双架构构建通过；paging-test/table（9 测试）/ conflict-panic / equal-range-panic / reversed-range-panic SMP QEMU 全绿；fmt clean。

### 追加：MMIO 子系统布局修正（同日后续轮次）

**动机**：用户质疑"为什么 MMIO 一个在 paging 一个在 memory？"——考古 git 历史发现：
- `ac09ec801`（2026-03-31）把 `MmioRegion` 从 memory 移入 paging，理由是**消除跨 crate unsafe**（当时 paging 内部构造函数是 `unsafe new_borrowed`，移入后可用 `pub(crate) wrap_existing`）
- `7987681fb`（2026-04）在 memory 加回 `map_mmio` 门面以集中做 **RAM 重叠校验**

两个原始动机在 ADR-013 + 5.2 改造后**都已失效**：`PageTable::identity_map_range` 现在是公开安全接口、RAM 校验已下沉到 `MmioRegion::map`。两处分布是历史惯性，不是当前架构意图。

**实施**（本轮第二次提交）：
- `crates/paging/src/mmio.rs` 整体迁至 `crates/memory/src/mmio.rs`
- `MmioRegion::map(paddr, size, ram_range)` 简化为 `MmioRegion::map(paddr, size)` —— 内部直接读 `MEMORY_INFO`（既然合并到 memory 就没必要保留参数注入）
- 删除 `memory::map_mmio` 函数——**真正单一入口**是 `memory::MmioRegion::map(...)`
- paging 回归**纯页表 crate**（2 个源文件：lib.rs + table.rs），`zerocopy` 依赖随 MMIO 模块一并移至 memory
- 调用方 4 处更新：`src/device/virtio.rs` / `src/arch/aarch64/mod.rs` / `src/arch/aarch64/interrupt.rs` / `src/arch/riscv64/interrupt.rs`
- paging/README.md + memory/AGENTS.md 同步更新分层图

### 关键决策

| # | 决策 | 状态 | ADR |
|---|------|------|-----|
| MmioRegion 单一入口 + ram_range 显式注入 | RAM 校验下沉到 paging 层，`memory::map_mmio` 降级为读 `MEMORY_INFO` 的 thin wrapper | 已实施 | — |
| `update_pte` 返回 `()` | 旧 flags 返回值无生产消费者，测试改为 `get_mapping` 验证 | 已实施 | — |
| range API 保留双形状 | byte range vs page count 反映不同调用场景，强制统一反而加负担——在 doc 里说明差异理由 | 已实施 | — |
| TLB shootdown 接入降优先级 | 当前 PTE 修改全在 SMP 激活前，不触发跨核失效需求；保留未决，等 P9 前置阶段再做 | 已调整 | — |
| **MmioRegion 整体迁回 memory crate** | 当年迁入 paging 的原始动机（消除 unsafe）已消失，让架构回到与当前动机匹配的状态 | 已实施 | — |

---

### 上上轮摘要（2026-04-18 ADR-013）

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
   - 新建 ADR-013 + crate 文档（memory/AGENTS.md，paging / heap / tlb README）

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
| 删除 `OwnedPages` 抽象层 | 无消费者 + 与 ADR-008 论据同构；改用 `PageTable::update_range_flags` 方法 | 已实施 | [ADR-013](../adr/013-ownedpages-necessity.md) |
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

#### 🟡 中优先级：TLB shootdown 回调机制待接入（**已降优先级**）

**文件**：`crates/tlb/src/lib.rs:27`

**现象**：`register_tlb_shootdown` 有零个调用者——`TLB_SHOOTDOWN_FN: spin::Once<fn(TlbFlushRequest)>` 从未被 `call_once`，所以 `flush_tlb()` / `flush_tlb_page()` 里的跨核 IPI 路径是死代码。

**当前为什么不爆**（本轮澄清的结论）：
- `PageTable::update_range_flags` 的唯一运行时调用是 `memory::init`，**发生在 SMP 激活之前**
- 从核通过 `init_smp` 激活分页时复用主核已经 finalize 的页表——从核启动时 TLB 从空开始，看不到任何中间状态
- MMIO 映射也在 SMP 激活前完成
- **结论**：运行时没有动态 PTE 修改 → 不需要跨核 TLB 失效 → 当前实现正确，不是 bug

**何时必须接入**：
- P9 用户程序引入 per-process 页表 → 进程切换需要 TLB 刷新
- 任何动态 `mmap` / `mprotect` 路径
- 页回收路径（如果引入）

**接入方向（ADR 待决，等 P9 前置 / R4 中断子系统就绪后再讨论）**：
1. IPI 机制：需要 `src/arch/{arch}/interrupt.rs` 的中断子系统提供跨核 IPI 发送原语
2. 注册时机：在 `device_init` / 中断子系统 init 完成后调用 `tlb::register_tlb_shootdown`
3. 数据结构：`TlbFlushRequest` 通过 per-CPU mailbox 或 IPI payload 传递；目标核在 IPI handler 中执行本地 `arch::flush_tlb_page` / `flush_tlb_all`
4. 同步：发起核是否需要等待其他核 ack（影响是否需要 barrier / 跨核状态位）
5. 与 `update_range_flags` 跨页非原子语义的配合：跨核协议需要考虑"发起核修改了 N 页但其他核可能只看到前 k 页新 flags"的情形

#### 🟢 低优先级：R6 相关

- R6 范围的锁（`dev_mgr` / `ramfs` / `fd` / `virtio_blk` / `mount_table`）锁级别仍用 `UNSPECIFIED`，待 R6 审计统一分配

#### 🟢 低优先级：已标记 TODO（待场景触发时处理）

- `update_range_flags` 单次 walk 批量优化（`crates/paging/src/table.rs:185`）——当前每页独立 walk，引入 mmap / 模块加载大映射场景后再优化

### R8 待办（审计收尾阶段）

- [ ] `CONTRIBUTING.md` — 贡献指南
- [ ] `CODE_OF_CONDUCT.md` — 社区行为准则
- [ ] `SECURITY.md` — 安全漏洞报告流程
- [x] `paging/README.md` — 分页子系统文档（ADR-013 落地时补全）
- [x] `tlb/README.md` — TLB 管理文档（同上）
- [x] `heap/README.md` — 堆分配器文档（同上）
- [x] `memory/AGENTS.md` — 内存门面 crate 文档（同上）

## 已完成的目标

| 日期 | Phase | 内容 |
|------|-------|------|
| 2026-04-03 | R0 | 审查报告 + 基础设施实施（CI/文档/审计基线/依赖/ADR） |
| 2026-04-03 | R1 | 审查报告 + 实施修复（span/config/build_common/memory_types） |
| 2026-04-03 | R3 | 审查报告 + 重构实施（四态 typestate、移除 page_allocator、锁级别、幂等映射） |
| 2026-04-18 | R3 (第二轮) | 精简未用 API + 错误扁平化（commit `8456b3540`，净减 ~540 行） |
| 2026-04-18 | R3 (ADR-013) | 删除 OwnedPages 抽象 + 精简 frame_allocator::init 接口 + 注释清理（commits `9d7ad44c6` / `f36f347de`，净减 ~170 行） |
| 2026-04-18 | R3 (收尾) | 清理低优先级 review 遗留：MMIO 单一入口 + `update_pte` 返回 `()` + range API 形状 doc + `update_range_flags` 跨页非原子 doc；TLB shootdown 降优先级；DMA 权限问题概念澄清保留 |
| 2026-04-18 | R3 (布局修正) | MMIO 子系统从 paging crate 整体迁回 memory crate（`crates/paging/src/mmio.rs` → `crates/memory/src/mmio.rs`）——当年迁入 paging 的动机（消除跨 crate unsafe）已失效，RAM 校验下沉后 ram_range 参数可内部从 MEMORY_INFO 读取；`memory::map_mmio` 门面删除，唯一入口 `memory::MmioRegion::map(paddr, size)`；paging 回归纯页表 crate（少 zerocopy 依赖） |
| 2026-05-07 | R3 (subagent 复审) | 使用 4 个 subagent 只读复审 `memory_types` / `frame_allocator` / `page_table_entry` / `paging` / `tlb` / `heap` / `memory` / `dma`，整理出初始化一次性、权限切换并发、W^X、DMA 真机语义、newtype 不变量、RISC-V MMIO 属性、AArch64 段边界和多段 RAM 等待讨论问题；详细说明见 `docs/audit/2026-05-07-r3-memory-review-findings.md` |
| 2026-05-07 | R3 (第一组修复) | 修复 `VirtAddr::align_down_to` canonical 校验、`Frame::new` 页号范围、`memory::init` 二次调用 fail-fast、`frame_allocator::init reserved` 校验语义；新增对应 QEMU 回归测试和测试清单。 |
| 2026-05-07 | R3 (第二组修复) | 收紧固件 reserved-memory 映射、删除公开 `kernel_rwx()`、将 `PageTable::update_pte()` 收窄为内部机制函数、禁止 frame allocator hard IRQ 分配/释放、对多段 RAM FDT fail-fast，并补对应回归测试。 |
| 2026-05-08 | R3 (文档与 warning 收口) | 删除 R3 roadmap / 依赖图中的 `page_allocator` 旧引用，确认 `crates/memory/AGENTS.md` 作为本地模块说明入口，移除 `src/fdt.rs` 中已过期的 3 个 `#[expect(dead_code)]`。 |
