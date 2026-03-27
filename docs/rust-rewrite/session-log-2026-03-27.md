# Session Log 2026-03-27: Code Review + Theseus Patterns + Multi-Crate Restructure

## 13 commits, 128 files, +2384/-38089 lines

| # | Commit | Type | Description |
|---|--------|------|-------------|
| 1 | `7845f56` | refactor | Rust best practices + fix timer interrupt storm |
| 2 | `fec72df` | fix | task_tick integration, W^X mapping, sleep/block_on race fix, +8 tests |
| 3 | `a58b4aa` | feat | Theseus patterns: HeldInterrupts, TaskBuilder, MmioRegion |
| 4 | `6ff6de1` | feat | MappedPages affine type (compile-time use-after-unmap) |
| 5 | `81155e7` | refactor | Extract leaf crates (error, config, compat, address) |
| 6 | `1719b33` | refactor | Remove old C++ tests |
| 7 | `911bef2` | refactor | Extract arch-traits crate (break sync↔arch↔per_cpu) |
| 8 | `15b1d81` | refactor | Extract sync, per-cpu, boot-info crates |
| 9 | `feb4544` | refactor | Break memory↔arch cycle (PTE migration + init split) |
| 10 | `29f5b7f` | refactor | Break task↔arch cycle (callbacks + bootstrap move) |
| 11 | `503f737` | refactor | Extract memory crate |
| 12 | `551417a` | refactor | Add CalleeSavedContext placeholder to arch-traits |
| 13 | `1097fbc` | fix | W^X mapping fix (.boot section has mixed code+data) |

## Decision Records

### 1. `cfg(target_os = "none")` vs `cfg(not(test))`

**Problem**: `cfg(test)` only applies to the crate being tested. When kernel test binary links arch-traits as a dependency, arch-traits' `cfg(test)` is false → privileged asm (`msr daifset`) executes in user mode → SIGILL. Similarly, memory crate's `#[global_allocator]` conflicts with host system allocator.

**Decision**: All bare-metal-only code (privileged instructions, `#[global_allocator]`, `init()`) gated by `cfg(target_os = "none")`. Other internal logic (test mocks, host placeholders) continues using `cfg(not(test))`.

**Rationale**: `target_os` is a compilation target attribute, unaffected by `--test` flag. Bare-metal target triples (`riscv64gc-unknown-none-elf`) have `target_os = "none"`, host has `"linux"`/`"macos"` — correct behavior in all compilation scenarios.

### 2. Re-export stub zero-breakage migration

**Problem**: Extracting modules to independent crates would require modifying all `use crate::xxx` imports.

**Decision**: Replace original `src/xxx.rs` with one-line `pub use xxx_crate::*;` re-export. All existing `use crate::xxx::Type` imports remain unchanged.

**Rationale**: Glob re-export imports sub-modules (`pub mod spinlock`), types, and functions into the current module namespace, achieving fully transparent path compatibility. Verified that deep paths like `crate::sync::spinlock::lock_level` still work.

### 3. Cycle-breaking strategies

**Problem**: Three major cycles — sync↔arch↔per_cpu, memory↔arch, task↔arch.

**Decisions**:
- **arch-traits leaf crate**: Extract 4 CPU primitive functions (core_id, irq_enabled/disable/enable) as cfg-gated inline asm. sync and per_cpu depend on arch-traits instead of arch → cycle eliminated.
- **PTE encoding migration**: Move impl blocks from arch/*/pte.rs into memory/page_table.rs cfg-gated modules → memory no longer depends on arch.
- **init() split**: memory::init() returns PageTable without calling activate_page_table → main.rs orchestrates arch and memory.
- **Callback function pointer**: arch timer handler calls task module through `arch_traits::call_timer_tick()` → arch no longer directly calls task.
- **kernel_thread_bootstrap to main.rs**: It's `#[no_mangle] extern "C"`, switch.S finds it by symbol name regardless of which crate defines it.

**Rationale**: Each approach chose the minimal change path. Callback function pointer has indirect call overhead, but only triggers at timer tick (100Hz) — negligible performance impact.

### 4. LockStack moved from sync to per-cpu

**Problem**: sync (SpinLock) needs `per_cpu::current_per_cpu()` to access LockStack; per_cpu struct contains LockStack → cycle.

**Decision**: LockStack is a per-CPU data structure, semantically belongs to per-cpu rather than sync. After move: sync → per-cpu (one-directional).

**Rationale**: LockStack is only used by SpinLock's push/pop/check, but its storage location is the PerCpu struct. Ownership follows storage location.

### 5. W^X mapping and .boot section

**Problem**: Linker script .boot section mixes `.text.boot` + `.data.boot` + `.bss.boot`, all before `__etext`. Mapping as pure RX causes store page fault when writing `.data.boot`, but stvec is not set → silent hang.

**Decision**: `[mem_start, __etext)` mapped as RWX (accommodate mixed .boot section), `[__etext, mem_end)` mapped as RW.

**Rationale**: Ideal solution would modify linker script to move .data.boot after __etext, but that requires changes to boot.S data references. Current approach still enforces W^X protection for all memory after __etext (.rodata, .data, .bss, heap, free frames) without touching the linker script.

### 6. task/arch crate extraction deferred

**Problem**: task depends on `arch::CalleeSavedContext` (`#[repr(C)]` register layout struct, completely different for riscv64 and aarch64). TCB holds it via `SyncUnsafeCell<CalleeSavedContext>`, needs to know exact size.

**Decision**: arch-traits provides host placeholder type. On bare metal, task continues referencing kernel-internal arch::CalleeSavedContext. All cycles already broken — physical extraction can be done independently later.

**Rationale**: Clean extraction needs generic TCB (`TaskControlBlock<C: Context>`) or trait objectification — a larger API change. Current cycles are eliminated, crate DAG is established, extraction is just file moves + import changes, not blocking any feature development.

## Current Workspace Structure

```
crates/
├── error/        ✅ ErrorCode, KResult
├── config/       ✅ Constants + PT_LEVELS
├── compat/       ✅ alloc/std conditional imports
├── address/      ✅ PhysAddr, VirtAddr
├── arch-traits/  ✅ CPU primitives + tick + callbacks + CalleeSavedContext placeholder
├── per-cpu/      ✅ PerCpu, PreemptState, LockStack
├── sync/         ✅ SpinLock, HeldInterrupts
├── boot-info/    ✅ BasicInfo
├── memory/       ✅ PageTable, FrameTracker, MappedPages, MmioRegion, heap
└── (kernel)      task + arch remain in kernel binary

Dependency DAG (no cycles):
  error, config, compat                    (leaf)
  address → config
  arch-traits → config + riscv
  per-cpu → arch-traits, config
  sync → per-cpu, arch-traits, spin
  boot-info → address, spin
  memory → error, config, address, arch-traits, sync, per-cpu, boot-info
  kernel → ALL + task(internal) + arch(internal)
```

## Test Results

76 tests pass, 0 failures. riscv64 QEMU full boot verified. aarch64 cross-compilation verified.
