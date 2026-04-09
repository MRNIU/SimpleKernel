# BusyBox 支持路线图

> 从 SAS 内核到运行 BusyBox shell 的完整路径。

## 架构决策

采用**方案 C：混合模型**——内核模块间保持 SAS 直接调用，用户程序通过 ecall/svc trap 进入内核。

```
┌──────────────────────────────────────────────────────┐
│  User Programs (U-mode / EL0)                        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ BusyBox  │  │  init    │  │  ...     │           │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘           │
│       │ecall/svc     │              │                 │
├───────┼──────────────┼──────────────┼─────────────────┤
│  Kernel (S-mode / EL1)                                │
│  ┌─────────────────────────────────────────────┐      │
│  │  syscall/abi.rs  (trap dispatch)            │      │
│  └────┬──────────────────────────┬─────────────┘      │
│       │ 直接调用                  │ 直接调用            │
│  ┌────▼────┐  ┌────────┐  ┌────▼────┐  ┌────────┐   │
│  │  task/  │  │ memory/│  │  fs/    │  │ device/│   │
│  │ (SAS)   │  │ (SAS)  │  │ (SAS)  │  │ (SAS)  │   │
│  └─────────┘  └────────┘  └────────┘  └────────┘   │
└──────────────────────────────────────────────────────┘
```

## 阶段依赖图

```
P8 ──→ P9 ──→ P10 ──→ P11 ──→ P12
                │               │
                └───────────────┘ (P10+P11 可部分并行)
```

## 阶段总览

| 阶段 | 文档 | 核心交付物 | 代码量 |
|------|------|-----------|--------|
| **P8** | [p8-umode-syscall-foundation.md](2026-04-09-p8-umode-syscall-foundation.md) | ecall/svc dispatch + U-mode hello world | ~300 行 |
| **P9** | [p9-elf-loader-process-model.md](2026-04-09-p9-elf-loader-process-model.md) | ELF64 加载器 + per-process 页表 + fork/execve | ~3000 行 |
| **P10** | [p10-core-syscalls.md](2026-04-09-p10-core-syscalls.md) | ~50 个 Linux syscall (内存/文件/进程/信号/系统) | ~4000 行 |
| **P11** | [p11-devices-pseudofs.md](2026-04-09-p11-devices-pseudofs.md) | /dev/{null,zero,console} + /proc + TTY 基础 | ~2000 行 |
| **P12** | [p12-busybox-integration.md](2026-04-09-p12-busybox-integration.md) | BusyBox 交叉编译 + initramfs + 启动调试 | ~1500 行 |
| | | **合计** | **~10,000-12,000 行** |

## 关键里程碑

| # | 里程碑 | 验证方式 | 对应阶段 |
|---|--------|---------|---------|
| M0 | ecall dispatch 不 panic | QEMU 测试：U-mode ecall write + exit | P8 |
| M1 | 加载并运行 musl hello world ELF | QEMU 测试：printf("hello") | P9 |
| M2 | fork + execve 组合工作 | QEMU 测试：fork 后 exec 子程序 | P9 |
| M3 | musl libc 完整初始化 | set_tid_address + mmap(TLS) + sigaction 不崩溃 | P10 |
| M4 | /dev/console + fd 0/1/2 | 用户程序 write(1,...) 输出到终端 | P11 |
| M5 | BusyBox `sh` 启动 | QEMU 出现 `#` 提示符 | P12 |
| M6 | `echo hello` 工作 | 输出 `hello` | P12 |
| M7 | `ls /` 工作 | 显示目录内容 | P12 |

## 已确认的基础设施就绪状态

以下组件已完成，P8-P12 直接复用：

- [x] 汇编 trap entry/return（两个架构，完整支持 U-mode）
- [x] PTE flags 的 user_* preset（user_rw, user_rx, user_ro, user_rwx）
- [x] TrapContext 保存所有寄存器（含 sscratch/sp_el0/ttbr0_el1）
- [x] PageTable::create/map_page/unmap_page（支持创建独立页表）
- [x] 帧分配器 + typestate 生命周期追踪
- [x] VFS FileSystem trait + RamFS + 挂载表 + 路径解析
- [x] 任务调度（FIFO/RR/CFS）+ SMP + 任务窃取
- [x] 信号编号 + 位图 + 投递机制
- [x] FD 表 + open/close/read/write
- [x] `elf` crate 已在依赖中（ELF 解析）

## 注意事项

1. **AArch64 页表模型待确认**：当前内核是否同时使用 TTBR0+TTBR1？若只用 TTBR0，则内核映射需复制到每个用户页表。若用 TTBR0(用户)+TTBR1(内核) 分离，则更简洁。P9 实施前需确认。

2. **RISC-V SV39 地址空间布局**：当前内核在低地址（0x80000000 identity mapping）。用户程序也在低地址空间。两者共存于同一页表时通过 U-bit 隔离，但地址范围可能重叠。P9 需要仔细规划地址分配。

3. **帧零化**：ELF BSS 段依赖帧被零化。需确认 buddy allocator 分配的帧是否已清零，若未清零则在 ELF loader 中显式 memset。

4. **用户指针验证**：P8/P9 暂不做完整验证（简化），P10 需要实现 `copy_from_user`/`copy_to_user` 安全访问用户内存。

5. **SAS 设计文档更新**：方案 C 引入 U-mode 后，`docs/design/SAS-架构设计.md` 需要更新，说明混合模型的设计理由。建议写一个 ADR。
