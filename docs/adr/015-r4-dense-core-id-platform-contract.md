<!-- Copyright The SimpleKernel Contributors -->

# ADR-015: R4 dense core id 平台契约

## 状态

**提议**

## 日期

2026-05-09

## 审计阶段

R4 — 架构层

## 涉及模块

`src/cpu_topology.rs`、`src/fdt.rs`、`src/boot.rs`、`src/arch/riscv64/`、
`src/arch/aarch64/`、`crates/per_cpu/`、`src/tlb_shootdown.rs`

## 背景

R4 复审发现两个相邻问题：

- R4-04：SMP 路径把硬件 hart id / MPIDR Aff0 直接当作数组索引，隐含 CPU id 必须稠密。
- R4-12：AArch64 SGI 只编码 Aff0 低 4 位，隐含单 cluster 且 Aff0 稠密。

SimpleKernel 当前目标是 QEMU virt 和教学/审计场景。若现在引入 logical id 到 hardware id 的完整映射，
需要同时改启动栈、per-CPU、IPI、TLB shootdown、scheduler 和 timer owner，改动面会超过 R4 修复切片。

## 备选方案

### 方案 A：显式声明 dense core id 平台契约

RISC-V hart id 与 AArch64 MPIDR Aff0 必须组成 `0..core_count` 的稠密集合。FDT CPU 表只做启动期校验和诊断，
运行期继续直接使用 core id。

**优点**:
- 改动面小，符合当前 QEMU virt 平台。
- 启动期 fail-fast，避免非稠密平台静默错绑 per-CPU 或 IPI。
- 保留现有跨核协议和调度接口。

**缺点**:
- 不支持 RISC-V 非稠密 hart id。
- 不支持 AArch64 多 cluster / Aff0 重复平台。
- 后续真机适配时仍需重新设计 topology 映射。

### 方案 B：引入 logical id 到 hardware id 的全局映射

FDT 解析阶段建立逻辑 CPU id，所有上层协议使用 logical id，架构层在 IPI/启动时转换到硬件 id。

**优点**:
- 能覆盖更广泛的 RISC-V hart id 和 AArch64 MPIDR 拓扑。
- 上层跨核协议不直接暴露硬件 id。

**缺点**:
- 需要大范围修改现有 per-CPU、启动栈、IPI、TLB shootdown 和 scheduler 调用面。
- R4 当前问题会扩散成全系统拓扑重构。

### 方案 C：各架构内部维护局部映射

RISC-V 和 AArch64 分别在自己的 IPI/启动路径维护映射，上层继续使用 core id。

**优点**:
- 单架构改动较局部。
- 可按硬件需求分阶段扩展。

**缺点**:
- 上层仍难判断 core id 的真实语义。
- 多处映射容易漂移，TLB shootdown 和 scheduler 仍可能重复处理拓扑差异。

## 决策

选择 **方案 A**。

短期将 dense core id 作为 SimpleKernel 当前平台契约，并在启动期用 FDT CPU 表强制校验。

## 理由

当前 R4 目标是修复启动和跨核协议的真实风险，不是扩展硬件覆盖面。dense 契约把隐含假设变成显式平台边界：
QEMU virt 和当前测试路径继续直接使用 core id；不满足契约的平台在启动期 fail-fast，而不是运行到 IPI 或
per-CPU 数组访问时才表现为随机错误。

这个选择依赖的假设是：R4 阶段支持的平台 CPU id 都可表示为 `0..core_count`。当需要支持非稠密 hart id、
AArch64 多 cluster 或 Aff0 重复平台时，必须用新的 ADR 重新讨论 logical/hardware id 映射。

## 影响

- **代码变更**: `cpu_topology` 校验 CPU id 稠密性；RISC-V/AArch64 启动和 IPI 路径保留 direct core id。
- **API 变更**: 不引入新的 public topology remap API。
- **测试**: `arch-test` 覆盖 discovered core count、primary/timekeeper core 和 Full 初始化后的 online barrier。
- **文档**: R4 审计报告和架构移植指南记录当前平台契约。
- **当前设计同步**: 后续如扩展真机拓扑，需要更新 R4 设计文档并新增 ADR。

## 参考

- `docs/audit/2026-05-08-r4-architecture-review-findings.md` — R4-04、R4-12、R4-13
- `docs/design/R4-arch-boot-sequence.md` — 当前 SMP online barrier
- `docs/design/R4-architecture-porting-guide.md` — 新架构 topology 契约
