<!-- Copyright The SimpleKernel Contributors -->

# ADR-018: R4 TLB shootdown 完整协议

## 状态

**提议**

## 日期

2026-05-09

## 审计阶段

R4 — 架构层

## 涉及模块

`src/tlb_shootdown.rs`、`crates/tlb/`、`src/arch/riscv64/ipi.rs`、
`src/arch/aarch64/ipi.rs`、`crates/paging/`、`src/boot.rs`

## 背景

TLB shootdown 的目标不是“发一个 IPI”，而是保证页表变更后，所有可能缓存旧翻译的目标 CPU
都完成本地 TLB 失效，并且发起方能可靠知道这一点。当前 R4 修复已经完成最小硬化：

- 发起方写 request mailbox 后执行 release fence。
- RISC-V `send_ipi()` 前执行 `fence rw, rw`。
- AArch64 写 `ICC_SGI1R_EL1` 前执行 `dsb ishst`。
- `broadcast()` 禁止 hard IRQ 上下文发起。
- 存在远端目标时要求发起方处于 IRQ enabled 状态。
- `kernel_init(Full)` 返回前等待所有 FDT discovered CPU online。

这些修复解决了最明显的发布顺序和启动期 online 缺口，但没有回答完整协议问题：

- 单个全局 `BROADCAST_LOCK` 是否是长期模型？
- 发起方等待 ack 时，如果目标核长时间无法处理 IPI，如何诊断？
- 将来若运行期页表权限切换、DMA 属性切换或跨核映射变更增多，是否需要更细粒度 mailbox？
- shootdown 与页表写锁、调度锁、中断屏蔽之间的锁序是否需要写入类型或运行时断言？

## 当前协议边界

当前实现可以描述为“单发起方、全局 mailbox、online 目标集合、同步等待 ack”：

```text
CPU0:
  flush local TLB
  acquire BROADCAST_LOCK
  hold local IRQ
  write REQUEST_ADDR / REQUEST_KIND
  increment REQUEST_GENERATION
  release fence
  send IPI to every online target CPU
  spin until every target ACK_GENERATION == REQUEST_GENERATION
  release BROADCAST_LOCK

CPU1:
  receive IPI
  acquire REQUEST_GENERATION
  read REQUEST_KIND / REQUEST_ADDR
  flush local TLB
  release ACK_GENERATION
```

这个协议成立依赖以下前提：

- 发起方进入等待前，目标 CPU 已 online 且能接收 IPI。
- 发起方等待期间，目标 CPU 的中断不会长期关闭。
- 发起方不会在 hard IRQ 中等待远端 ack。
- 同一时间只有一个发起方写全局 request mailbox。
- 页表修改本身由调用方保证和 shootdown 调用顺序正确。

## 备选方案

### 方案 A：保留单 broadcast lock，补齐契约和诊断

继续使用当前全局 `BROADCAST_LOCK + REQUEST_* + ACK_GENERATION[]` 结构，但把协议边界写死：

- 只允许非 hard IRQ、IRQ enabled 的上下文发起跨核 shootdown。
- 等待 ack 时增加有限自旋上限；超时直接 panic，打印目标 mask、缺失 ack mask、generation 和 request。
- 把 `REQUEST_KIND/ADDR/GENERATION` 的内存序约束集中封装，禁止调用点绕过。
- 明确页表修改者必须先完成 PTE 写入，再调用 shootdown；若未来运行期共享写者增多，再引入页表写锁或 token。

**优点**:
- 与当前代码最接近，改动小。
- 适合 R4 当前用途：启动期和低频页表属性更新。
- 失败时可诊断，不会无限 spin。

**缺点**:
- 所有 shootdown 串行化，扩展性有限。
- 发起方仍同步等待目标核，长尾延迟会阻塞当前 CPU。
- 不能自然支持多个发起方并发请求。
- 仍依赖“目标核能及时处理中断”的运行时前提。

### 方案 B：per-CPU mailbox + sequence counter

为每个目标 CPU 维护独立 mailbox：

```text
MAILBOX[cpu].kind
MAILBOX[cpu].addr
MAILBOX[cpu].request_seq
MAILBOX[cpu].ack_seq
```

发起方按目标 CPU 写入对应 mailbox 并递增 request sequence；目标 CPU 的 IPI handler 只读取自己的 mailbox，
flush 后写 ack sequence。可以保留一个较小的 initiator lock，也可以后续扩展为多发起方合并请求。

**优点**:
- 每个目标 CPU 的状态独立，诊断更精确。
- 更容易增加 per-core timeout、pending 状态和统计。
- 后续支持请求合并或减少全局锁更自然。
- 避免所有目标共享一个 `REQUEST_KIND/ADDR` 带来的扩展限制。

**缺点**:
- 代码和状态明显增加。
- 需要定义多个发起方同时写同一目标 mailbox 时的合并或排队规则。
- IPI handler 需要处理 sequence 追赶和重复 IPI。
- 仍然需要明确页表写者锁序和 IRQ enabled 等发起上下文约束。

### 方案 C：stop-the-world rendezvous

将 TLB shootdown 提升为更强的跨核 rendezvous：

1. 发起方请求目标 CPU 进入 rendezvous。
2. 目标 CPU 到达安全点后停止执行普通任务，并 ack “已暂停”。
3. 发起方执行页表变更或确认变更已完成。
4. 所有 CPU flush TLB。
5. 发起方释放目标 CPU 继续运行。

**优点**:
- 对大范围页表重构、地址空间切换或未来复杂内存管理语义更清晰。
- 可以把“页表写入”和“远端停止执行旧映射”绑定成一个协议。
- 适合后续如果引入运行期映射结构重排。

**缺点**:
- 对 scheduler、IPI、preemption 和中断状态侵入很大。
- 延迟最高，容易影响实时性。
- 对当前 R4 的最小页表更新需求偏重。
- 需要更多测试基础设施才能证明所有 CPU 都进入/退出 rendezvous。

### 方案 D：只做本地 flush，不等待远端

页表修改后只 flush 本核，远端 CPU 在下一次上下文切换或自然 TLB 失效时再看到新映射。

**优点**:
- 实现最简单。
- 没有跨核等待和 IPI 协议复杂度。

**缺点**:
- 对 shared kernel page table 不成立，远端可继续使用旧权限或旧映射。
- W^X、MMIO 属性、unmap 等安全边界无法保证。
- 不适合作为 SimpleKernel 的内核页表一致性策略。

## 决策

**待项目作者决策。**

需要在以下方向中选择一个：

- 选择方案 A：短期保留当前单 broadcast lock，补 timeout 和诊断，作为 R4/R5 的正式协议。
- 选择方案 B：现在进入 per-CPU mailbox + sequence counter 重构。
- 选择方案 C：把 shootdown 和运行期页表修改提升为 stop-the-world rendezvous 设计。
- 方案 D 不建议作为正式选择，只作为反例记录其风险。

## 决策问题

项目作者需要明确以下问题：

1. 当前 R4/R5 是否只需要低频页表权限更新，还是要为后续运行期映射变更预留并发协议？
2. shootdown 超时应当 `panic!()` fail-fast，还是记录错误后继续运行？按当前内核错误策略，协议失效更接近内核 bug。
3. 是否接受“发起跨核 shootdown 必须 IRQ enabled”的 API 约束？
4. 是否需要在 R4 阶段就把页表写者锁序写成代码约束，还是只在 ADR 和测试中约束？
5. TLB shootdown 的目标集合是当前 online CPU，还是所有 discovered CPU？当前代码已经选择 online CPU，boot barrier 保证 Full 初始化后二者一致。

## 理由

这里暂不替项目作者选择方案。技术上，方案 A 是当前代码的自然延续；方案 B 是中期扩展性更好的协议；
方案 C 是最强一致性模型，但需要和调度器、页表写锁、抢占边界一起设计。

无论选择哪一项，都必须保留已经落地的基本不变量：request 发布先于 IPI、远端 flush 先于 ack、
发起方等待时不能处于 hard IRQ，且失败路径必须可诊断。

## 影响

- **代码变更**:
  - 方案 A：主要增加 timeout、缺失 ack 诊断和文档化锁序。
  - 方案 B：重构 `src/tlb_shootdown.rs` 的全局 mailbox 为 per-CPU mailbox。
  - 方案 C：新增 rendezvous 状态机，并联动 scheduler / interrupt 边界。
- **API 变更**: 可能需要把发起上下文约束写进 `tlb` crate 的回调契约。
- **测试**:
  - 必须补远端访问强证明：CPU0 改权限或映射后 shootdown，CPU1 在 ack 后访问目标 VA。
  - 必须补 timeout/诊断测试或可控故障注入。
  - 方案 B/C 需要额外覆盖并发发起方、重复 IPI 和目标核延迟 ack。
- **文档**: `docs/design/R4-interrupt-timer-flow.md` 需要按最终方案更新。
- **当前设计同步**: 若方案 B/C 被接受，需要同步更新 R4 架构移植指南和 paging 设计文档。

## 参考

- `docs/audit/2026-05-08-r4-architecture-review-findings.md` — R4-05
- `docs/design/R4-interrupt-timer-flow.md` — 当前最小 shootdown 协议
- `src/tlb_shootdown.rs` — 当前实现
