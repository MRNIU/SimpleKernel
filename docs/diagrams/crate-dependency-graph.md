# Crate 依赖关系图

> **生成日期**：2026-04-03
> **生成方式**：`cargo metadata --no-deps` 自动提取
> **分支**：`feat/rust-SAS`

## Workspace 内部依赖

```mermaid
graph TD
    subgraph kernel["内核主体"]
        simplekernel
    end

    subgraph tests["测试"]
        system-test
        panic-test
    end

    subgraph R1["R1 原语层"]
        config
        span
        memory_types
        build_common
    end

    subgraph R2["R2 同步与 CPU"]
        macros
        per_cpu
        interrupt_state
        sync
        global_tick
        local_tick
    end

    subgraph R3["R3 内存子系统"]
        frame_allocator
        page_table_entry
        tlb
        paging
        heap
        memory
    end

    %% R1 内部
    memory_types --> config
    memory_types --> span

    %% R2 依赖 R1
    per_cpu --> config
    per_cpu --> macros
    interrupt_state --> per_cpu
    sync --> config
    sync --> interrupt_state
    sync --> per_cpu
    local_tick --> per_cpu

    %% R3 依赖 R1 + R2
    frame_allocator --> config
    frame_allocator --> memory_types
    frame_allocator --> sync
    page_table_entry --> memory_types
    tlb --> config
    paging --> config
    paging --> frame_allocator
    paging --> memory_types
    paging --> page_table_entry
    paging --> sync
    paging --> tlb
    heap --> config
    heap --> interrupt_state
    heap --> sync
    memory --> config
    memory --> frame_allocator
    memory --> heap
    memory --> memory_types
    memory --> paging
    memory --> per_cpu
    memory --> sync
    memory --> tlb

    %% 内核主体依赖
    simplekernel --> config
    simplekernel --> global_tick
    simplekernel --> heap
    simplekernel --> interrupt_state
    simplekernel --> local_tick
    simplekernel --> memory
    simplekernel --> memory_types
    simplekernel --> paging
    simplekernel --> per_cpu
    simplekernel --> sync
    simplekernel -.->|build| build_common

    %% 测试依赖
    system-test --> simplekernel
    system-test --> config
    system-test --> global_tick
    system-test --> heap
    system-test --> memory
    system-test --> memory_types
    system-test --> per_cpu
    system-test --> sync
    system-test -.->|build| build_common
    panic-test --> simplekernel
    panic-test --> heap
    panic-test --> memory
    panic-test --> per_cpu
    panic-test -.->|build| build_common
```

## 外部依赖（按 crate）

| Workspace Crate | 外部依赖 |
|----------------|---------|
| `build_common` | cc |
| `config` | log |
| `frame_allocator` | buddy_system_allocator, log |
| `heap` | buddy_system_allocator, log |
| `interrupt_state` | aarch64-cpu, riscv |
| `macros` | proc-macro2, quote, syn |
| `memory` | log, spin, zerocopy |
| `page_table_entry` | bitflags |
| `paging` | log, spin |
| `per_cpu` | aarch64-cpu, tock-registers |
| `simplekernel` | aarch64-cpu, arm-gic, arm-psci, bitfield-struct, bitflags, buddy_system_allocator, cc, elf, fatfs, fdt, gdbstub, hashbrown, heapless, intrusive-collections, log, qemu-exit, riscv, rustc-demangle, sbi-rt, smoltcp, spin, tock-registers, unwinding, virtio-drivers |
| `sync` | *(无直接外部依赖)* |
| `tlb` | aarch64-cpu, riscv, spin |
| `xtask` | clap, xshell |
