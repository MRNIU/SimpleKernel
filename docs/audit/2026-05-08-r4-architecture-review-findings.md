<!-- Copyright The SimpleKernel Contributors -->

# R4 架构层复审问题说明

> 日期：2026-05-08
>
> 范围：`src/arch/`、`src/boot.rs`、`src/main.rs`、`src/timer.rs`、
> `src/tlb_shootdown.rs`、`src/fdt.rs`、`src/task/` 中与架构层直接耦合的入口，
> 以及 `tests/test_harness/`、`xtask/src/qemu.rs` 中会影响 R4 验证可信度的测试基础设施。
>
> 本文最初记录 R4 架构层复审中发现的问题原因、可能触发路径、修复方案和验证方向。
> 后续修复切片会在对应条目下追加修复状态；仍未定案的设计选择保持 ADR 待决。

## 模块概述

R4 覆盖的是“硬件入口到内核抽象”的边界层：架构启动代码负责建立初始栈、解析 boot 参数、
配置异常向量、中断控制器、定时器、IPI 和 SMP；`boot::kernel_init()` 再把这些能力按阶段交给内存、
任务、设备、文件系统等上层模块。它的主要风险不是单个函数写错，而是初始化时序、跨核协议、
中断上下文和 Rust 类型边界之间的不变量没有被代码或测试显式表达。

## 总览

| # | 严重度 | 问题 | 主要风险 | 决策状态 |
|---|--------|------|----------|----------|
| R4-01 | P1 | Timer/IRQ 可能在调度器初始化前触发 | tick 进入未初始化 scheduler 状态，启动期 panic 或状态损坏 | 已修复 |
| R4-02 | P1 | timer preemption 闭环不完整且绕过调度策略 | 时间片语义不生效，FIFO/RR/CFS trait 结果被旁路 | 方案 A 已落地，ADR-016 |
| R4-03 | P1 | RISC-V 从核启动跳过 `gp` 初始化 | 从核访问 small data / 全局对象可能使用错误 `gp` | 已修复 |
| R4-04 | P1 | SMP 假设硬件 core id 稠密且小于 `MAX_CORE_COUNT` | 非稠密 hart/MPIDR 拓扑下栈、per-CPU、IPI 索引越界或错绑 | 平台契约已显式化，ADR-015 |
| R4-05 | P1 | TLB shootdown 发布顺序和等待协议不完整 | 远端核可能看不到请求；广播锁与 IRQ 屏蔽组合可死锁 | 已按 ADR-018 方案 A 收口；RISC-V/AArch64 远端访问强证明已补 |
| R4-06 | P1 | RISC-V hard-float 状态未保存 | 开启 F/D 目标后跨任务浮点寄存器被破坏 | 已修复，ADR-017 |
| R4-07 | P1 | 任务栈未显式保证 16 字节对齐 | ABI 违约，函数 prologue / 保存上下文可能在部分平台异常 | 已修复 |
| R4-08 | P1 | `ArchOps::dtb_addr()` safe API 隐藏裸指针解引用 | safe 调用者无法知道 boot args 的 unsafe 前提 | 已修复 |
| R4-09 | P1 | `should_panic` 测试可能误判通过 | 未触发 panic 的负向测试仍被 xtask 当成成功 | 已修复 |
| R4-10 | P2 | AArch64 `PA_BITS` 与 TCR `IPS` 需要一致 | 物理地址宽度契约与 MMU 配置不一致 | 已修复 |
| R4-11 | P2 | RISC-V PLIC 固定 hart0 S-mode context 且启用顺序过早 | 从核外部中断不可用；早期外部中断可打到未初始化 PLIC | 已修复 |
| R4-12 | P2 | AArch64 SGI 只使用 Aff0 低 4 位 | 多 cluster 或 Aff0 不稠密时 IPI 投递错误 | 平台契约已显式化，ADR-015 |
| R4-13 | P2 | `CORE_COUNT` 语义混淆 discovered/online，SMP 启动 fire-and-forget | 主核继续运行时从核可能尚未上线或已启动失败 | 已修复 |
| R4-14 | P2 | 全局 tick 固定 `core_id == 0` | BSP 不是 0 或 timekeeper 迁移后 tick 失效 | 已修复 |
| R4-15 | P2 | timer 错误处理、零 interval 和漂移语义不闭环 | 定时器静默停止、除零或长期漂移 | ADR-019 方案 B 已落地；方案 C/D 后续回看 |
| R4-16 | P2 | `src/arch` 暴露面偏宽 | APP / 上层模块可绕过 syscall 和任务抽象边界 | 首轮已收窄 |
| R4-17 | P2 | R4 文档、测试与实际代码漂移 | 审计结论和回归证据不足，后续修复缺少可信测试 | R4 交付文档已补；协议测试仍随 ADR |

## 处理顺序

1. **先修测试可信度**：处理 R4-09，否则 `should_panic` 相关回归可能继续给出假阳性。
2. **再收敛启动期低耦合问题**：R4-03、R4-07、R4-08、R4-10、R4-15 中的 fail-fast 部分。
3. **再进入时序和跨核协议**：R4-01、R4-02、R4-04、R4-05、R4-11、R4-12、R4-13、R4-14。
4. **最后补 R4 交付物**：R4-16、R4-17，包括架构扩展指南、启动时序图、中断流程图和 SMP 验证。

## R4-01. Timer/IRQ 可能在调度器初始化前触发

位置：`src/boot.rs:83-91`、`src/timer.rs:16`、`src/task/sched.rs:71`、`src/task/sched.rs:249`

### 问题原因

`kernel_init()` 当前先执行 `Arch::init_timer()` 和 `Arch::init_interrupt()`，再执行 `task::init()`。
如果架构实现中的 timer init 或 interrupt init 使能了本核定时器中断，tick handler 会在 scheduler
全局状态初始化前进入 `task::timer_tick()`。Rust 类型系统无法表达“中断还没有全局打开”这个时序前提，
所以该不变量只能靠初始化顺序、显式状态机或运行时断言保证。

### 可能触发路径

```text
boot::kernel_init(Full)
  -> Arch::init_timer()
  -> Arch::init_interrupt()
     -> 架构层打开 timer/IRQ
        -> timer interrupt
           -> timer::handle_tick()
              -> task::timer_tick()
                 -> scheduler / current task 状态尚未初始化
  -> task::init()
```

在 QEMU 中如果 tick 恰好晚于 `task::init()` 才到达，问题不会暴露；在真机或不同 QEMU timing 下，
中断可能插入到 `init_interrupt()` 与 `task::init()` 之间。

### 修复方案

- 把“配置中断向量/控制器”和“全局打开 IRQ”拆成两个阶段：R4 阶段只配置硬件，等 idle task、
  per-CPU scheduler 和 current task 就绪后再 unmask。
- 或者在 `task::init()` 前建立最小 BSP idle/scheduler 状态，使 timer tick 即使早到也有合法落点。
- 在 `task::timer_tick()` 或 scheduler 入口增加初始化状态断言，启动期 fail-fast，避免静默破坏状态。

### 验证方向

- 增加一个 RISC-V QEMU 启动测试，强制在打开 timer 后、`task::init()` 前制造一次 tick，
  期望要么被 mask，要么落到已初始化的 idle scheduler。
- 启动日志中记录 IRQ 全局打开点和 scheduler ready 点，用于人工确认时序。

### 修复状态

已调整 `boot::kernel_init()` 顺序：主核在 `Arch::init_timer()` / `Arch::init_interrupt()`
之前先完成 `task::init()`，确保 timer IRQ 即使在全局 IRQ 打开后立即到达，也能落到已初始化的
idle/current task 与 per-CPU scheduler 状态上。

## R4-02. Timer preemption 闭环不完整且绕过调度策略

位置：`src/timer.rs:18`、`src/main.rs:59`、`src/main.rs:78`、`src/task/sched.rs:160`、`src/task/sched.rs:251`

### 问题原因

tick handler 当前只设置 `NEED_RESCHED`，真正消费点在主循环或非中断上下文中；同时
`src/task/sched.rs` 禁止在 interrupt context 直接调度。这避免了在硬中断里做上下文切换，
但缺少“IRQ 返回前/返回后”的统一 preempt hook，导致时间片到期不能形成完整闭环。

另一个问题是 `timer::handle_tick()` 无条件设置 resched flag，没有以 scheduler trait 的
`timer_tick()` 结果作为唯一来源。这样会把 FIFO/RR/CFS 等策略的差异旁路掉。

### 可能触发路径

```text
timer interrupt
  -> timer::handle_tick()
     -> task::timer_tick()
     -> NEED_RESCHED = true
  -> 返回被中断任务
     -> 没有统一的 post-IRQ preempt 检查
```

如果当前任务不主动回到 `main` 循环或 syscall-like 网关，调度可能长期不发生。对于 FIFO 或不应被时间片抢占的任务，
无条件置位也可能让策略层的返回值失去意义。

### 修复方案

- 方案 A：在 trap/IRQ 返回路径增加 post-IRQ preempt hook，要求 `HardIrqGuard` 已释放、本核中断状态可恢复，
  再根据 scheduler 结果决定是否调用 `schedule()`。
- 方案 B：让 `task::timer_tick()` 返回是否需要 reschedule，`timer::handle_tick()` 不再直接写全局 flag；
  scheduler policy 成为唯一决策点。
- 方案 C：保持当前延迟调度模型，但文档明确它不是抢占式 timer，只是 cooperative resched flag；
  同时把 FIFO/RR/CFS 的时间片语义降级为未来目标。此方案需在 R5 设计中写清楚。

以上属于 R4/R5 交界设计，应写入 ADR 或任务调度设计文档。

### 修复状态

已按方案 A 落地：`task::timer_tick()` 返回调度策略是否要求抢占，公共 timer 层只在该返回值为 true 时
设置当前核 `need_resched`；RISC-V/AArch64 IRQ handler 在 `HardIrqGuard` 释放后调用
`task::preempt_after_irq()`，由 `preempt::take_irq_exit_preemption_request()` 消费一次 pending flag 并进入
`schedule()`。这保留了“硬中断内不切换上下文”的边界，同时闭合 timer-driven preemption 路径。
该边界已补充为 ADR-016。

### 验证方向

- 增加一个多任务 timer 测试：一个 busy loop 任务不主动 yield，另一个任务依赖 tick 获得运行机会。
- 分别覆盖 FIFO/RR/CFS 的 tick 语义，确认策略返回值没有被公共 timer 层旁路。

## R4-03. RISC-V 从核启动跳过 `gp` 初始化

位置：`src/arch/riscv64/ipi.rs:79`、`src/arch/riscv64/boot.rs:21`

### 问题原因

RISC-V `hart_start(..., opaque=0)` 传给从核的 `a1` 为 0，而启动汇编里 `a1 == 0` 时跳过 `gp`
初始化。`gp` 是 RISC-V psABI 用于 small data 区访问的全局指针寄存器；从核若不初始化，
Rust 编译器生成的全局/静态数据访问可能使用错误基址。

这里类似 C/C++ 裸机启动里没有给每个 CPU 初始化 TLS/global pointer：主核能跑不代表从核 ABI 已满足。

### 可能触发路径

```text
主核:
  riscv64::ipi::start_secondary(hart_id)
    -> sbi::hart_start(hart_id, secondary_entry, opaque = 0)

从核:
  _start(a0 = hart_id, a1 = 0)
    -> 因 a1 == 0 跳过 gp 初始化
    -> secondary_boot()
       -> 访问 static / global 数据
```

只要从核路径访问依赖 `gp` 的数据布局，行为就取决于 reset 后 `gp` 的偶然值。

### 修复方案

- 在 RISC-V 启动汇编中无条件执行 `la gp, __global_pointer$`。
- 或者把 `opaque` 明确定义为 boot args 指针，但 `gp` 初始化仍不应依赖该参数。
- 增加从核启动早期断言或日志，确认 `gp` 已建立后再进入 Rust。

### 验证方向

- 增加 SMP QEMU 测试：从核启动后读写一个会落入 small data 的 per-CPU 或全局静态变量。
- 反汇编检查 `_start` 和从核入口均包含 `gp` 初始化路径。

### 修复记录

2026-05-09 已改为在 RISC-V `_boot` 中无条件初始化 `gp`，不再依赖 `a1/opaque`
是否携带 DTB 地址。RISC-V `arch-test` 已在 2 核 QEMU 中通过，覆盖 Full 初始化和从核启动路径。

## R4-04. SMP 假设硬件 core id 稠密且小于 `MAX_CORE_COUNT`

位置：`src/fdt.rs:75`、`src/init.rs:51`、`src/arch/riscv64/boot.rs:32`、
`src/arch/aarch64/boot.rs:22`、`crates/per_cpu/src/lib.rs:194`

### 问题原因

当前 FDT 只统计 CPU 节点数量并写入 `CORE_COUNT`，而启动栈、per-CPU 存储和若干架构路径会直接使用
硬件 hart id 或 MPIDR Aff0 作为数组索引。该实现隐含了两个未写出的前提：

- 硬件 CPU id 从 0 开始连续递增。
- 最大硬件 id 小于 `MAX_CORE_COUNT`。

这在简单 QEMU virt 平台上通常成立，但不是 RISC-V hart id 或 Arm MPIDR 的通用契约。

### 可能触发路径

```text
FDT:
  cpu@0
  cpu@2

fdt::cpu_count() = 2
CORE_COUNT = 2

从核 hart_id = 2
  -> boot stack[2]
  -> per_cpu[2]
  -> 越过 0..CORE_COUNT 或错过 core 1
```

AArch64 多 cluster 场景下，MPIDR 的 Aff1/Aff2 也可能参与 CPU 唯一标识，单取 Aff0 更容易错绑。

### 设计取舍

当前不引入 `logical_id <-> hardware_id` remap。SimpleKernel 的平台契约是 CPU id 必须为
`0..core_count` dense 编号，RISC-V hart id 与 AArch64 Aff0 都按该契约直接作为 core id 使用。
FDT `/cpus` 表只用于启动期校验和异常诊断：CPU 数量超出 `MAX_CORE_COUNT`、id 重复、缺洞或非 dense
都 fail-fast，并打印表项帮助定位平台描述问题。

### 验证方向

- 构造测试 DTB，包含非稠密 hart id，期望当前实现 fail-fast 并输出 FDT CPU 表。
- SMP 启动测试断言每个在线核的 `current_core_id()` 落在 `0..CORE_COUNT`。

### 修复状态

已收缩为 `cpu_topology` 平台契约校验：FDT `/cpus` 解析出的 id 必须组成 dense `0..core_count`
集合；运行期不维护 remap。`per_cpu`、`hart_start`、SBI IPI、AArch64 SGI 和启动栈继续直接使用
core id；`_boot` 只增加 `core_id < MAX_CORE_COUNT` 的早期边界检查。
该 dense core id / 单 cluster 平台契约已补充为 ADR-015。

## R4-05. TLB shootdown 发布顺序和等待协议不完整

位置：`src/tlb_shootdown.rs:95`、`src/tlb_shootdown.rs:108`、`src/tlb_shootdown.rs:122`、
`src/arch/aarch64/ipi.rs:30`、`src/arch/riscv64/ipi.rs:21`

### 问题原因

TLB shootdown 是跨核协议，不只是“写请求 + 发 IPI”。发起核写入请求后，必须保证远端核在收到 IPI
时能看到请求内容；远端核完成本地 TLB flush 后，也必须以可见顺序发布 ack。当前代码存在两个边界：

- 发布请求与发送 IPI 之间缺少架构级 store barrier 说明。AArch64 只有 `isb`，它不是普通内存写发布屏障；
  RISC-V SBI 调用前后也没有显式 `fence` 契约。
- 广播路径持有 broadcast lock 并等待 ack。如果某个等待者或持锁者处于 IRQ disabled 状态，远端 IPI handler
  可能无法运行，形成等待环。

### 可能触发路径

```text
CPU0:
  acquire broadcast lock
  write request mailbox
  send IPI to CPU1
  wait CPU1 ack

CPU1:
  IRQ disabled 或正在等待 broadcast lock
  IPI handler 不能及时运行

结果:
  CPU0 等 ack，CPU1 无法 ack，系统卡死
```

另一条路径是远端核先收到 IPI，但由于缺少 publish barrier，看见旧请求或未初始化请求。

### 修复方案

- 在发布请求后、发送 IPI 前加入架构屏障：RISC-V 使用合适的 `fence rw, rw`；AArch64 使用
  `dsb ishst` 或与 GIC doorbell 语义匹配的发布屏障。
- 明确调用约束：发起 shootdown 时本核必须允许远端 IPI 被处理，或协议必须能在 IRQ disabled 区域安全运行。
- 方案 A：保留单 broadcast lock，但 acquire 前断言不在 hard IRQ / IRQ disabled 的不可等待区域。
- 方案 B：改为 per-CPU mailbox + sequence counter，避免等待者争用同一把广播锁。
- 方案 C：实现 stop-the-world 风格 shootdown，先让目标核进入 rendezvous，再批量修改/flush。复杂度更高，应 ADR 待决。

### 验证方向

- 增加 SMP paging 测试：CPU0 修改映射属性并发 shootdown，CPU1 在 ack 后立即访问该 VA，
  验证远端 TLB 确实失效。
- 增加负向调试断言：禁止在 hard IRQ 或已禁用中断且可能等待远端 ack 的上下文发起 broadcast。

### 修复状态

2026-05-09 已完成不需要 ADR 的最小硬化：`broadcast()` 继续禁止 hard IRQ 上下文，
并在存在远端目标时要求发起方处于 IRQ enabled 状态；request mailbox 发布后加入 release fence；
RISC-V `send_ipi()` 前加入 `fence rw, rw`；AArch64 写 `ICC_SGI1R_EL1` 前加入 `dsb ishst`。

2026-05-09 已按 ADR-018 采用方案 A：暂时保留单 broadcast lock，但等待远端 ack 改为有限自旋；
超时直接 panic，并打印发起核、目标 mask、缺失 ack mask、generation、request kind 和 request addr。
新增 `paging-test/tlb-shootdown-timeout-panic` should_panic 回归，覆盖 ack 缺失时 fail-fast。

2026-06-08 已新增 `paging-test/tlb-remote-access`：CPU1 先写目标 VA 缓存旧 RW 翻译，
CPU0 将同一页改为 RO 并等待 shootdown ack，随后 CPU1 再写同一 VA 必须触发 RISC-V
store page fault。测试通过 `test-support` 下的精确 fault 恢复钩子匹配 target core、fault VA
和 fault PC，避免吞掉非预期异常。

同日已补 AArch64 同型强证明：`TrapContext` 保存 `FAR_EL1`，测试钩子匹配 target core、
`FAR_EL1`、`ELR_EL1` 和写 data abort，再把 `elr_el1` 跳到测试 resume label。

仍未升级为 per-CPU mailbox、sequence counter 或 stop-the-world rendezvous；这些保留为后续运行期映射变更增多后的演进方向。

## R4-06. RISC-V hard-float 状态未保存

位置：`src/arch/riscv64/context.rs:117`、`src/arch/riscv64/switch.rs:29`、
`src/arch/riscv64/interrupt.rs:168`、`src/arch/aarch64/context.rs:116`

### 问题原因

当前 RISC-V target 是 `riscv64gc-unknown-none-elf`，`gc` 包含 F/D 浮点扩展；上下文切换只保存整数
callee-saved 寄存器，没有保存 `fs0-fs11` 和 `fcsr`。AArch64 路径已经保存 FP/SIMD callee-saved，
两架构语义不一致。

如果内核或未来 APP 编译出浮点指令，RISC-V 跨任务切换会破坏浮点状态。即使当前代码不主动写浮点，
编译目标允许该指令集也意味着边界需要被明确关闭或保存。

### 可能触发路径

```text
Task A:
  使用浮点寄存器 fs0
  被 timer 抢占

Task B:
  使用浮点寄存器 fs0
  切回 Task A

Task A:
  读到被 Task B 覆盖后的 fs0
```

如果 `sstatus.FS` 没有正确启用，另一种触发是执行浮点指令直接陷入 illegal instruction / disabled FP trap。

### 修复方案

- 方案 A：RISC-V 内核明确禁用浮点。修改 target/features 或启动时关闭 `sstatus.FS`，
  并在文档中声明内核/APP 不允许 hard-float。
- 方案 B：保存/恢复 `fs0-fs11 + fcsr`，并正确管理 `sstatus.FS`。实现成本更高，但与 AArch64 语义更接近。
- 方案 C：lazy FPU save/restore。首次浮点 trap 时分配状态并切换 FS；复杂度最高，适合后续性能优化。

该选择影响 ABI 和任务模型，应写入 ADR。

### 验证方向

- 若选择禁用浮点：增加包含浮点指令的负向测试，确认 fail-fast 或编译期禁止。
- 若选择保存浮点：增加两个任务交替写不同浮点值的 RISC-V QEMU 测试。

### 修复状态

2026-05-09 已按方案 B 恢复 RISC-V eager FPU 上下文保存，并补充 ADR-017。
当前实现会在每个 hart 初始化时设置 `sstatus.FS=Dirty`；trap path 保存/恢复
`f0-f31 + fcsr`，任务切换保存/恢复 ABI callee-saved `fs0-fs11`。`arch-test`
新增 RISC-V 浮点运算和跨任务 `fs0` 保存回归：红测曾确认父任务 `fs0` 会被子任务覆盖，
修复后该测试通过。

## R4-07. 任务栈未显式保证 16 字节对齐

位置：`src/boot.rs:10`、`src/task/tcb.rs:20`、`src/task/tcb.rs:33`、`src/task/tcb.rs:128`

### 问题原因

启动栈显式做了 16 字节对齐，但任务栈用 `Vec<u8>` 分配后直接把末尾地址作为栈顶传给架构上下文。
`Vec<u8>` 的元素对齐只有 1，分配器通常会返回更高对齐，但这不是类型契约。RISC-V 和 AArch64
ABI 都要求栈指针在函数调用边界保持 16 字节对齐。

这类似 C 里用 `malloc(size)` 得到“通常够用”的对齐，但又把它当成特殊 ABI 栈使用；在 Rust 中应把对齐要求编码到类型或分配布局里。

### 可能触发路径

```text
TaskControlBlock::new()
  -> Vec<u8> task_stack
  -> stack_top = task_stack.as_ptr() + len
  -> InitTaskContext::new(stack_top)
  -> switch_to()
     -> Rust 函数以未对齐 SP 运行
```

触发后可能表现为函数 prologue 保存寄存器异常、SIMD/FP 访问对齐异常，或只在优化级别变化后出现。

### 修复方案

- 引入 `KernelStack` 类型，内部用显式 `Layout::from_size_align(size, 16)` 或等价 aligned buffer 分配。
- `KernelStack::top()` 返回前执行 `debug_assert_eq!(top % 16, 0)`，在裸机测试中也保留 fail-fast 断言。
- TCB 不直接暴露 `Vec<u8>`，避免调用方绕过栈对齐不变量。

### 验证方向

- 增加任务创建测试，断言所有新任务初始 SP 16 字节对齐。
- 在上下文切换入口增加一次低成本断言，调试期捕获错误栈。

### 修复记录

2026-05-09 已将 `KernelStack` 改为显式 `Layout::from_size_align(..., 16)` 分配，
`top()` 保留 16 字节对齐断言，并在 `arch-test` 中增加栈顶对齐回归。

## R4-08. `ArchOps::dtb_addr()` safe API 隐藏裸指针解引用

位置：`src/arch/mod.rs:17`、`src/arch/aarch64/init.rs:7`、`src/arch/aarch64/init.rs:24`、`src/arch/aarch64/init.rs:34`

### 问题原因

`ArchOps::dtb_addr()` 是 safe trait method，但 AArch64 实现会从 bootloader 传入的 raw `argv`
中解引用并解析 DTB 地址。safe API 表示调用者不需要承担额外 unsafe 前提；这里却要求 boot args
指针有效、布局正确、生命周期足够长。

这会把裸机启动边界的 unsafe 责任藏在 trait 实现内部，不利于审计。C/C++ 中这类函数通常会显式命名为
`parse_boot_args_unchecked` 或在接口文档里要求 caller 传入有效指针；Rust 中更应通过 `unsafe fn`
或已验证类型表达。

### 可能触发路径

```text
main::_start()
  -> Arch::dtb_addr()
     -> AArch64 读取 bootloader argv raw pointer
        -> 指针为空、错位、指向非预期布局
           -> safe API 内部触发 UB 或读出错误 DTB 地址
```

RISC-V 通常由 `a1` 直接传 DTB 地址，AArch64 boot chain 更复杂，因此该问题主要出现在 AArch64。

### 修复方案

- 方案 A：将 `dtb_addr()` 改为 `unsafe fn`，并补充 `# Safety` 文档，明确 boot args 指针和布局前提。
- 方案 B：引入 `BootArgs` newtype，在 `_start` 最早阶段做一次 unsafe 解析和校验；后续 `ArchOps`
  只接收已验证的 safe `BootArgs`。
- 方案 C：架构启动汇编直接规范化 DTB 地址，把 Rust 层入口参数统一为整数地址，减少 raw pointer 暴露面。

### 验证方向

- 增加 AArch64 boot args 解析单元或 QEMU smoke 测试，覆盖空指针/非法 magic 的 fail-fast 路径。
- Rustdoc 中补齐 `# Safety` 或 `BootArgs` 的不变量说明。

### 修复记录

2026-05-09 已将 `ArchOps::dtb_addr()` 改为 `unsafe fn` 并补充 `# Safety` 契约。
AArch64 解析入口增加 `argc < 3` 的早期返回，避免参数数量不足时直接读取 `argv[2]`。

## R4-09. `should_panic` 测试可能误判通过

位置：`tests/test_harness/src/lib.rs:11`、`tests/test_harness/src/lib.rs:15`、
`tests/test_harness/src/lib.rs:131`、`xtask/src/qemu.rs:349`

### 问题原因

测试 harness 文档区分了成功/失败退出码，但当前 RISC-V/AArch64 的 QEMU 退出实现没有把该 code
传递成宿主可见的进程状态。`should_panic` 分支如果未触发 panic，会走失败路径，但 xtask 只看 QEMU
进程退出是否正常，可能把“测试逻辑失败”误判为通过。

这会污染 R3/R4 之后所有 panic 回归。之前 R3 修复已经需要额外 grep 串口 `SHOULD_PANIC OK`
来弥补该缺口。

### 可能触发路径

```text
should_panic 测试:
  run_test()
    -> 没有 panic
  harness failure path
    -> exit_qemu(非零 code)
       -> QEMU 正常退出，宿主进程状态仍为 0
  xtask:
    -> 只看 QEMU status = success
    -> 误判测试通过
```

### 修复方案

- 方案 A：实现 machine-visible exit device/semihosting，让 `exit_qemu(code)` 真正影响宿主退出状态。
- 方案 B：xtask 解析串口 sentinel，要求 normal test 输出 `TEST OK`，should_panic 输出 `SHOULD_PANIC OK`；
  失败 sentinel 或缺失 sentinel 都判失败。
- 方案 C：短期在所有 should_panic 调用点显式传入期望 panic 文案并由 xtask grep；这是过渡方案，
  不能替代统一 harness 语义。

### 验证方向

- 增加一个故意“不 panic”的 should_panic 负向 harness 自测，期望 xtask 返回失败。
- 增加一个真实 panic 的 should_panic 正向自测，确认 sentinel/退出码路径不会误杀。

### 修复记录

2026-05-09 补充测试体开始标记：`should_panic` 只有进入测试函数后发生的 panic 才输出
`SHOULD_PANIC OK`；启动/初始化阶段 panic 统一输出 `TEST PANIC:` 并退出 QEMU。
普通测试 panic 也改为测试专用 `TEST PANIC:` 失败路径，避免生产 panic handler 自旋到 timeout。

## R4-10. AArch64 `PA_BITS` 与 TCR `IPS` 需要一致

位置：`crates/arch/src/aarch64.rs:21`、`src/arch/aarch64/mod.rs:82`

### 问题原因

代码层面的物理地址宽度常量是 48 位，但 AArch64 的 `TCR_EL1.IPS` 决定 stage-1 translation
使用的物理地址尺寸。如果 TCR 没有显式设置 IPS，页表项中 48 位物理地址的契约就没有和 MMU 配置闭环。

### 可能触发路径

```text
memory/page table:
  允许创建 48-bit PA 映射

AArch64 MMU:
  TCR_EL1.IPS 未匹配 48-bit
  -> 高位 PA 被解释错误或触发地址尺寸 fault
```

QEMU virt 的内存通常落在低地址，短期可能看不出问题；真机或更大 PA 布局下会暴露。

### 修复方案

- 方案 A：启动时读取 `ID_AA64MMFR0_EL1.PARange`，选择不超过硬件能力且与 `PA_BITS` 一致的 IPS。
- 方案 B：固定声明 SimpleKernel AArch64 平台要求 48-bit PA，并在启动时断言硬件支持。
- 方案 C：把 `PA_BITS` 从架构常量改为启动期探测结果，但这会影响 `memory_types` 等 crate 的编译期边界设计。

### 验证方向

- AArch64 启动期读取 PARange，若硬件不报告平台约定的 44-bit PA 则在启用 MMU 前 fail-fast。
- TCR 固定写入 `IPS=0b100`，保持 `arch::PA_BITS=44`、AArch64 PTE 输出地址位和 MMU 配置一致。
- 当前 QEMU `cortex-a72` 报告 44-bit PA，因此 AArch64 QEMU 启动测试应通过。

### 修复记录

2026-05-09 首轮按方案 B 收窄为 48-bit PA fail-fast；随后因当前 QEMU `cortex-a72`
仅报告 44-bit PARange，改为方案 A 的最小平台收口：`arch::PA_BITS=44`，
`TCR_EL1.IPS=0b100`，启动期检查硬件 PARange 必须与该平台契约一致。

## R4-11. RISC-V PLIC 固定 hart0 S-mode context 且启用顺序过早

位置：`src/arch/riscv64/interrupt.rs:29`、`src/arch/riscv64/interrupt.rs:36`、
`src/arch/riscv64/interrupt.rs:78`、`src/arch/riscv64/interrupt.rs:110`、
`src/arch/riscv64/interrupt.rs:168`、`src/arch/riscv64/interrupt.rs:172`、
`src/arch/riscv64/interrupt.rs:191`

### 问题原因

PLIC 代码固定使用 hart0 的 S-mode context，claim/complete 和 enable 都只配置这一份 context。
从核启用外部中断后没有对应 context 配置，外部中断语义不完整。

同时，全局 SIE 打开点早于 `plic_init()` 完成。如果外部中断在这个窗口到达，handler 可能调用尚未初始化的 PLIC
全局对象并 panic。

### 可能触发路径

```text
主核:
  enable SIE
  尚未 plic_init()
  外部中断到达
    -> handler claim
       -> plic() 发现 Once 未初始化
       -> panic

从核:
  init_interrupt_smp()
    -> enable SEIE
    -> 没有配置 hartN S-mode context
  外部中断投递到从核
    -> claim/complete 使用 hart0 context 或没有有效 context
```

### 修复方案

- PLIC context 根据 hart id 计算，常见 QEMU virt S-mode context 可表达为 `2 * hart_id + 1`，
  但应由平台描述或文档约束确认。
- 初始化顺序调整为：设置 trap vector → 映射/初始化 PLIC → 配置 context enable/priority/threshold →
  设置 `sie` 位 → 最后打开全局 SIE。
- 从核如果暂不支持外部中断，应显式保持 SEIE masked，而不是打开未配置路径。

### 验证方向

- SMP QEMU 中让从核执行一次 external interrupt enable 路径，断言使用的 PLIC context 与 hart id 匹配。
- 增加早期中断顺序测试或 debug 断言，禁止 PLIC 未初始化时 claim。

### 修复状态

已改为按当前硬件 hart id 计算 PLIC S-mode context（`2 * hart_id + 1`），主核/从核分别配置本核 context
的 enable/threshold；主核初始化顺序调整为设置 trap vector、映射并初始化 PLIC、配置 context 后再打开
`sie` 和全局 SIE。

## R4-12. AArch64 SGI 只使用 Aff0 低 4 位

位置：`src/arch/aarch64/ipi.rs:23`、`src/arch/aarch64/ipi.rs:27`、`src/arch/aarch64/ipi.rs:85`

### 问题原因

GIC SGI 的目标不是简单的 `core_id & 0xf`。完整目标需要 MPIDR affinity fields：
Aff0/Aff1/Aff2/Aff3。当前只使用 Aff0 低 4 位，隐含“单 cluster 且 Aff0 连续小于 16”的平台前提。

### 可能触发路径

```text
CPU topology:
  CPU0: Aff1=0, Aff0=0
  CPU1: Aff1=1, Aff0=0

send_ipi(CPU1)
  -> 只编码 Aff0=0
  -> IPI 可能发到 CPU0 或错误 cluster
```

### 设计取舍

- 当前不支持多 cluster / 非 Aff0 dense 平台；AArch64 CPU id 取 MPIDR Aff0。
- 启动期通过 FDT CPU 表校验该平台契约，发现完整 MPIDR 不是 dense `0..core_count` 时 fail-fast。
- SGI 发送保留 TargetList 编码，并断言 `cpu_id < 16`。

### 验证方向

- AArch64 SMP 启动时记录每个在线核 MPIDR，断言当前 SGI 编码前提成立。
- 后续可用模拟多 cluster DT/MPIDR 的单元测试覆盖编码函数。

### 修复状态

已按 R4-04 的设计取舍收缩：当前 AArch64 只支持单 cluster、Aff0 dense 且 `< 16` 的平台。
`send_ipi(cpu_id)` 保持 TargetList 编码，并增加 `cpu_id < 16` 断言；FDT CPU 表如果出现多 cluster
MPIDR 编码，会在 topology 校验阶段 fail-fast，而不是运行期 remap。
该平台契约与 R4-04 合并记录在 ADR-015。

## R4-13. `CORE_COUNT` 语义混淆 discovered/online，SMP 启动 fire-and-forget

位置：`src/lib.rs:10`、`src/boot.rs:100`、`src/arch/riscv64/ipi.rs:83`、`src/arch/aarch64/ipi.rs:113`

### 问题原因

`CORE_COUNT` 的文档语义像“在线核心数”，但当前写入的是 FDT 发现的 CPU 数。`wake_secondary_cores()`
发起启动后主核继续运行，RISC-V/AArch64 路径对启动失败多为记录日志或局部处理，没有统一等待所有预期从核上线的 barrier。

### 可能触发路径

```text
FDT 发现 4 核:
  CORE_COUNT = 4

boot:
  wake_secondary_cores()
  主核继续 device/fs/task 后续路径

某个从核:
  启动失败或尚未完成 init_smp()

上层:
  按 CORE_COUNT 遍历 4 个核做 IPI / TLB shootdown / scheduler 初始化
  -> 目标核未在线，ack 永远不到或状态未初始化
```

### 修复方案

- 拆分 `discovered_core_count()` 和 `online_core_count()`。
- `wake_secondary_cores()` 后等待从核进入明确的 online 状态，带超时和失败诊断。
- 上层跨核协议只遍历 online mask；需要“所有发现核心必须上线”的测试场景则在 boot barrier 处 fail-fast。

### 验证方向

- R4 SMP test 断言：发现核数、上线核数、online mask 一致；从核均执行过 `kernel_init_smp()`。
- 注入一个启动失败路径，验证主核能超时并打印失败 hart/MPIDR。

### 修复状态

本轮已拆出 discovered topology 摘要（`cpu_topology::topology().discovered_core_count()`）和 FDT dense
校验；TLB shootdown 继续使用 online mask。2026-05-09 已在 `kernel_init(Full)` 的
`Arch::wake_secondary_cores()` 后加入 `wait_for_all_discovered_cores_online()`，从核启动失败直接
fail-fast，Full 初始化返回时所有 FDT discovered CPU 必须 online。

## R4-14. 全局 tick 固定 `core_id == 0`

位置：`src/timer.rs:11`、`src/timer.rs:13`、`src/main.rs:18`、`src/arch/riscv64/ipi.rs:57`

### 问题原因

timer 层用 `core_id == 0` 判断全局 tick 归属，但 primary/BSP 不一定是硬件 id 0。
RISC-V 启动路径也承认任意 hart 可能成为 primary。这里把“逻辑主核”“硬件 id 0”“timekeeper core”
三个概念混在一起。

### 可能触发路径

```text
启动 hart_id = 2 成为 primary

timer interrupt on hart 2:
  core_id != 0
  -> 不执行 global tick

系统:
  sleeps/timeouts/jiffies 依赖 global tick
  -> 时间不推进
```

### 修复方案

- 启动时记录 `TIMEKEEPER_CORE_ID`，timer 层使用该值，而不是硬编码 0。
- 如果未来支持 timekeeper 迁移，抽象成 atomic owner；当前阶段可以固定为 primary core。

### 验证方向

- 增加启动断言：primary core id 与 timer global tick owner 一致。
- 在 RISC-V QEMU 中尝试非 0 boot hart 配置时，验证 global tick 仍推进。

### 修复状态

已增加 `timer::init_timekeeper()` / `timer::timekeeper_core_id()`，topology 初始化时把 primary core
记录为 timekeeper；`handle_timer_common()` 用该 core id 推进全局 tick，不再硬编码 `core_id == 0`。

## R4-15. Timer 错误处理、零 interval 和漂移语义不闭环

位置：`src/arch/riscv64/timer.rs:38`、`src/arch/riscv64/timer.rs:46`、
`src/arch/riscv64/timer.rs:61`、`src/arch/riscv64/timer.rs:77`、
`src/arch/riscv64/timer.rs:79`、`src/arch/aarch64/timer.rs:18`、`src/arch/aarch64/timer.rs:64`

### 问题原因

RISC-V timer 对 `set_timer` 的错误使用 `.ok()` 丢弃，导致 SBI timer 失败时系统静默失去 tick。
频率校验只保证 `freq > 0`，但没有保证 `freq / TICK_HZ > 0`；如果 `TICK_HZ` 高于 timer freq，
interval 会变成 0。AArch64 也存在除法前缺少频率/interval 保护的问题。

此外，RISC-V 使用 `read_time() + interval` 重新设置下一次 tick，AArch64 使用相对 `CNTV_TVAL_EL0`。
如果 handler 延迟较大，这种相对重装会累积漂移。

### 可能触发路径

```text
低频 timer:
  freq < TICK_HZ
  interval = freq / TICK_HZ = 0
  -> timer 设置为当前时间或无效值

SBI set_timer 失败:
  .ok() 丢弃错误
  -> 没有 tick，系统无诊断

handler 延迟:
  next = now + interval
  -> 每次延迟都进入下一周期，长期漂移
```

### 修复方案

- 对 timer freq 和 interval 做 fail-fast：`freq >= TICK_HZ` 且 `interval > 0`。
- `set_timer` 失败直接 panic 或返回 `KResult` 到初始化层；内核不能静默继续。
- 使用 per-core absolute deadline：每次 tick 后 `deadline += interval`，若已经落后则追赶到未来。
- AArch64 可改用 `CNTV_CVAL_EL0` 表达绝对 deadline，RISC-V 继续用 SBI absolute time。

### 验证方向

- 增加 timer init 单元测试或架构 mock，覆盖 `freq = 0`、`freq < TICK_HZ`、`set_timer` 失败。
- 增加 QEMU 长时间 tick 统计，确认 tick 间隔不会因 handler 延迟持续漂移。

### 修复记录

2026-05-09 已补齐 fail-fast 部分：公共 `checked_tick_interval()` 会拒绝
`freq < TIMER_FREQ_HZ` 或零 interval；RISC-V `set_timer` 失败不再 `.ok()` 丢弃，
而是携带 deadline/error/value panic。绝对 deadline 和长期漂移语义仍留到
R4/R5 timer-preemption 设计阶段处理。
已补充 ADR-019 提议稿，列出相对重装、absolute deadline、missed tick 补记和 tickless
方向的取舍。2026-05-09 项目作者已同意暂定采用方案 B：后续先改为 per-core absolute
deadline，晚到时跳到未来但只记一个逻辑 tick；missed tick 补记和 tickless one-shot 后续回看。
同日后续已执行方案 B：`timer::next_absolute_deadline()` 编码“跳到未来但只记一个逻辑 tick”的纯逻辑；
RISC-V 每核保存 `NEXT_DEADLINE` 并继续用 SBI absolute `set_timer()`；AArch64 每核保存
`NEXT_DEADLINE` 并改用 `CNTV_CVAL_EL0`。方案 C 的 missed tick 补记仍留作后续升级。

## R4-16. `src/arch` 暴露面偏宽

位置：`src/lib.rs:15`、`src/arch/mod.rs:15`、`src/arch/mod.rs:50`、`src/arch/mod.rs:64`

### 问题原因

SAS 架构下，APP crate 的隔离依赖 Rust 可见性和 `src/syscall/` 作为唯一跨模块 API 网关。
如果 `src/arch` 公开过多底层 context、switch、interrupt 或 MMIO 能力，上层模块甚至未来 APP
可能绕过 syscall/task 抽象，直接触碰架构内部机制。

### 可能触发路径

```text
simplekernel::arch::* 为 public
  -> 上层或 APP crate 直接调用 arch interrupt/context/switch 能力
  -> 绕过 syscall API 网关和任务状态机
  -> SAS 隔离只剩约定，不再是编译期边界
```

当前主要是可维护性和架构边界风险；到 R7 可见性审计时会变成必须收敛的公共 API 问题。

### 修复方案

- 扫描 `pub` 暴露，区分三类：跨 crate 必须公开、内核 crate 内部 `pub(crate)`、模块私有。
- `context` / `switch` / `interrupt` / `timer` 等底层能力优先收窄为 `pub(crate)`，通过 task/syscall 提供上层入口。
- 对确实需要跨 crate 的架构能力，建立小而稳定的 facade，并补 doc comment 契约。

### 验证方向

- R7 前增加可见性清单：所有 `simplekernel::arch::*` public item 逐项说明消费者。
- 用编译检查确认 APP-like 测试 crate 不能访问架构内部 switch/interrupt 符号。

### 修复状态

2026-05-09 已将根 `arch` 模块收窄为 `pub(crate)`，并将 `ArchOps`、`Arch`、
`CalleeSavedContext`、`switch_to` facade 收窄为 crate 内部可见。当前独立测试 crate 不再能通过
`simplekernel::arch::*` 直接访问底层架构能力。R7 前仍应做全仓库 public API 清单，确认其他模块
是否还存在类似越层暴露。

## R4-17. R4 文档、测试与实际代码漂移

位置：`docs/audit/review-roadmap.md:190`、`docs/design/00-概述.md:139`、
`src/boot.rs:79`、`tests/arch-test/src/main.rs:7`、`tests/paging-test/src/tlb_shootdown.rs:18`、
根 `AGENTS.md` 的 R4 code map / QEMU timeout 说明

### 问题原因

R4 Roadmap 要求产出架构 trait 文档、启动时序图、中断处理流程图、架构扩展指南和 SMP 启动验证；
当前代码已经进入更复杂的 SMP/timer/TLB shootdown 状态，但文档仍有旧启动顺序、模块路径或测试策略描述。

测试侧曾存在覆盖不足：`arch-test` 主要覆盖 AArch64 FP，`paging-test` 的旧 TLB shootdown 测试偏 PTE
检查。后续已补 `paging-test/tlb-remote-access` 覆盖 RISC-V/AArch64 远端 ack 后访问语义。

### 可能触发路径

```text
后续修复 R4:
  依据旧设计文档或旧 code map 判断初始化顺序
  -> 修改点落在错误模块
  -> 测试只验证局部 PTE/FP，不验证跨核协议
  -> regression 进入主线
```

### 修复方案

- 补 R4 专用文档：
  - 架构抽象 trait 接口文档。
  - 启动时序图：`_start -> bootstrap -> kernel_init -> SMP online`。
  - 中断处理流程图：trap/IRQ entry、guard、handler、post-IRQ preempt。
  - 架构扩展指南：新增架构需要实现哪些 trait、汇编入口、timer/interrupt/IPI 契约。
- 更新旧文档中与当前代码冲突的启动顺序和路径描述。
- 补 R4 测试：
  - SMP 上线验证。
  - IPI 投递和 ack。
  - timer delivery 与 preemption hook。
  - TLB shootdown 远端访问验证（RISC-V/AArch64 已补）。
- 统一 QEMU timeout 说明：交互命令 30 秒、CI 单测超时 120 秒、xtask 默认值是否需要保留 300 秒应写清楚。

### 验证方向

- 文档中的关键函数名和路径通过脚本或 CI 检查存在性。
- R4 修复完成后，至少运行 `cargo xtask test --arch riscv64 --name <R4 test> --timeout 30` 的目标回归，
  涉及 QEMU 时使用 30 秒超时并清理残留进程。

### 修复状态

2026-05-09 已补充 R4 当前设计文档：

- `docs/design/R4-arch-boot-sequence.md`
- `docs/design/R4-interrupt-timer-flow.md`
- `docs/design/R4-architecture-porting-guide.md`

同时在 `docs/README.md` 增加当前设计入口，并在 `arch-test` 中补充 Full 初始化返回后的
all-discovered-cores-online 合约断言。2026-06-08 已补 RISC-V/AArch64 `paging-test/tlb-remote-access`
作为 TLB shootdown 远端访问强证明。

## 设计讨论点

### 1. RISC-V 浮点策略

- 方案 A：禁用 hard-float。优点是上下文切换简单，缺点是限制内核/APP 可用指令集。
- 方案 B：保存/恢复浮点 callee-saved 状态。优点是 ABI 更完整，缺点是每次切换成本和代码复杂度增加。
- 方案 C：lazy FPU。优点是按需付费，缺点是 trap 路径、状态机和测试复杂。

ADR：已补充 ADR-017。当前选择方案 B，后续如改 lazy FPU 需新 ADR。

### 2. CPU topology 模型

- 方案 A（已采用）：短期断言硬件 id 稠密。优点是改动小，缺点是平台假设窄。
- 方案 B：引入 logical/hardware id 映射。优点是能覆盖 RISC-V 非稠密 hart 和 AArch64 MPIDR，
  缺点是需要改 per-CPU、IPI、boot stack 和 scheduler 调用面。
- 方案 C：每个架构各自维护映射。优点是局部实现快，缺点是上层跨核协议容易重复处理拓扑差异。

ADR：已补充 ADR-015，用于固化 dense CPU id 平台契约和后续 online barrier 语义。

### 3. TLB shootdown 协议

- 方案 A：单 broadcast lock + 明确上下文断言 + 架构 barrier。优点是当前结构改动小，缺点是协议吞吐和嵌套限制明显。
- 方案 B：per-CPU mailbox + sequence counter。优点是减少全局锁等待，缺点是实现和调试成本更高。
- 方案 C：stop-the-world rendezvous。优点是语义清晰，缺点是对调度/中断时序侵入大。

ADR：已补充 ADR-018，并已接受方案 A；per-CPU mailbox / rendezvous 保留为后续演进。

### 4. Timer 与调度边界

- 方案 A（已落地）：R4 提供 post-IRQ preempt hook，R5 scheduler policy 决定是否切换。
- 方案 B：R4 只置位 resched flag，R5 在安全点消费，文档明确这是协作式调度。
- 方案 C：把 timer tick 完全下沉给 task scheduler，R4 只负责硬件 ack/rearm。

ADR：已补充 ADR-016，用于固化“硬中断内不 schedule、IRQ exit 消费 need_resched”的边界。

### 5. Timer absolute deadline 与 tick 漂移

- 方案 A：保留相对重装，把 tick 定义为 best-effort heartbeat。优点是实现简单，缺点是长期漂移。
- 方案 B：使用 per-core absolute deadline，晚到时跳到未来但只记一个逻辑 tick。优点是硬件触发点不继续漂移，
  缺点是逻辑 tick 仍可能少记。
- 方案 C：使用 absolute deadline 并补记 missed ticks。优点是 sleep/timeout 更贴近真实时间，
  缺点是公共 timer 和 scheduler 记账都要支持批量 tick。
- 方案 D：未来 tickless one-shot deadline。优点是空闲时减少 tick，缺点是超出当前 R4 范围。

ADR：已补充 ADR-019，方案 B 已落地；missed tick 补记和 tickless one-shot 留作后续回看。

## 文档产出建议

- `docs/design/R4-arch-boot-sequence.md`：启动与 SMP 上线时序图。
- `docs/design/R4-interrupt-timer-flow.md`：中断、timer、preemption 流程图。
- `docs/design/R4-architecture-porting-guide.md`：新增架构指南。
- ADR：RISC-V 浮点策略（已补 ADR-017）。
- ADR：CPU topology 平台契约（已补 ADR-015）。
- ADR：TLB shootdown 协议（已补 ADR-018，方案 A 已接受，RISC-V/AArch64 远端访问强证明已补）。
- ADR：timer/preemption 边界（已补 ADR-016）。
- ADR：timer absolute deadline / tick 漂移语义（已补 ADR-019，方案 B 已落地，方案 C/D 后续回看）。

## 本轮验证

本文最初是文档化审计结论；后续修复切片按 TDD 补充 `arch-test` 回归，并在容器内验证：
`cargo xtask check --arch riscv64`、`cargo xtask check --arch aarch64`、
`cargo xtask test --arch riscv64 --name arch-test --timeout 30`。完整格式检查和最终回归记录见
`docs/audit/audit-progress.md`。
