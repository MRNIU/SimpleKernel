# ETL io_port Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all `io::In`/`io::Out` calls (from `src/include/io.hpp`) with `etl::io_port_ro/wo/rw<T>` (pattern B — runtime address), then delete `src/include/io.hpp`.

> **前置条件：** 本计划依赖 `2026-02-27-etl-delegate-fsm-flags.md` 中 Task 1.1 已完成（`ETL_USE_ASSERT_FUNCTION` 已启用，`KernelEtlAssertHandler` 已注册）。ETL 内部检查已激活，`etl::io_port` 的边界断言将调用内核 assert 处理器。

**Architecture:** All five affected sites use runtime-resolved MMIO base addresses (from FDT or ACPI), so compile-time address template parameter is not used. Every call site is replaced with a locally-constructed `etl::io_port_rw/ro/wo<uint32_t>` instance passed a `volatile uint32_t*` pointer. The `io.hpp` header is removed only after all references are eliminated.

**Tech Stack:** C++23, ETL (`3rd/etl/include/etl/io_port.h`), `etl_profile.h`, Google C++ style, `.clang-format`, `pre-commit`

---

## Background

`src/include/io.hpp` provides two thin wrappers:

```cpp
namespace io {
template <typename T>
auto In(uint64_t addr) -> T {
  return *reinterpret_cast<volatile T*>(addr);
}
template <typename T>
void Out(uint64_t addr, T val) {
  *reinterpret_cast<volatile T*>(addr) = val;
}
}  // namespace io
```

The ETL replacement pattern (mode B — runtime address, no template address parameter):

```cpp
// Include order: system → third-party → project
#include <etl/io_port.h>   // add to existing includes section
// "io.hpp" line is REMOVED

// Read-only access
etl::io_port_ro<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(addr)};
uint32_t val = reg.read();

// Write-only access
etl::io_port_wo<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(addr)};
reg.write(val);

// Read-write access (used when the same address is both read and written)
etl::io_port_rw<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(addr)};
uint32_t old = reg.read();
reg.write(new_val);
```

`etl_profile.h` is already included transitively through `io.hpp` in most files, but after removal of `io.hpp` it may need to be added explicitly. Check per file.

> **Note:** `etl::io_port.h` is a template header — it does **not** need a corresponding `.cpp`. No CMake changes are needed.

---

## Files Touched

| # | File | Change |
|---|------|--------|
| 1 | `src/arch/aarch64/interrupt.cpp` | Remove `#include "io.hpp"` (unused) |
| 2 | `src/arch/aarch64/interrupt_main.cpp` | Remove `#include "io.hpp"` (unused) |
| 3 | `src/arch/aarch64/gic/include/gic.h` | Replace `io::In/Out` in inline `Read/Write` helpers |
| 4 | `src/arch/x86_64/apic/io_apic.cpp` | Replace `io::In/Out` in `Read()`/`Write()` |
| 5 | `src/arch/x86_64/apic/local_apic.cpp` | Replace all `io::In/Out` in xAPIC branches |
| 6 | `src/device/include/driver/virtio_blk_driver.hpp` | Replace 2× `io::In` in `Probe()` |
| 7 | `src/include/io.hpp` | Delete file |

---

## Task 1: Remove unused `#include "io.hpp"` from aarch64 interrupt files

**Files:**
- Modify: `src/arch/aarch64/interrupt.cpp`
- Modify: `src/arch/aarch64/interrupt_main.cpp`

These two files include `io.hpp` but never call `io::In` or `io::Out`. The include is dead code.

**Step 1: Edit `interrupt.cpp` — remove the dead include**

Current content (lines 5–10):
```cpp
#include "interrupt.h"

#include "io.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"
```

Replace with:
```cpp
#include "interrupt.h"

#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"
```

**Step 2: Verify `interrupt_main.cpp` has the same dead include**

Open `src/arch/aarch64/interrupt_main.cpp` and confirm `#include "io.hpp"` appears with no `io::` calls in the file. If confirmed, remove that line.

**Step 3: Run format check on both files**

```bash
pre-commit run --files src/arch/aarch64/interrupt.cpp src/arch/aarch64/interrupt_main.cpp
```

Expected: no changes needed (or auto-fixed with exit 0 on re-run).

**Step 4: Commit**

```bash
git add src/arch/aarch64/interrupt.cpp src/arch/aarch64/interrupt_main.cpp
git commit --signoff -m "refactor(aarch64): remove unused io.hpp include from interrupt files"
```

---

## Task 2: Migrate AArch64 GIC — `gic.h` inline Read/Write helpers

**Files:**
- Modify: `src/arch/aarch64/gic/include/gic.h`

`gic.h` is a header with two `__always_inline` private helper methods inside `Gicd` and `Gicr` nested classes. These are the **only** sites in this file that call `io::In`/`io::Out`.

### `Gicd::Read/Write` (lines ~348–354)

Current:
```cpp
__always_inline auto Read(uint32_t off) const -> uint32_t {
  return io::In<uint32_t>(base_addr_ + off);
}

__always_inline void Write(uint32_t off, uint32_t val) const {
  io::Out<uint32_t>(base_addr_ + off, val);
}
```

Replace with:
```cpp
__always_inline auto Read(uint32_t off) const -> uint32_t {
  etl::io_port_ro<uint32_t> reg{
      reinterpret_cast<volatile uint32_t*>(base_addr_ + off)};
  return reg.read();
}

__always_inline void Write(uint32_t off, uint32_t val) const {
  etl::io_port_wo<uint32_t> reg{
      reinterpret_cast<volatile uint32_t*>(base_addr_ + off)};
  reg.write(val);
}
```

### `Gicr::Read/Write` (lines ~557–564)

Current:
```cpp
__always_inline auto Read(uint32_t cpuid, uint32_t off) const -> uint32_t {
  return io::In<uint32_t>(base_addr_ + cpuid * kSTRIDE + off);
}

__always_inline void Write(uint32_t cpuid, uint32_t off,
                           uint32_t val) const {
  io::Out<uint32_t>(base_addr_ + cpuid * kSTRIDE + off, val);
}
```

Replace with:
```cpp
__always_inline auto Read(uint32_t cpuid, uint32_t off) const -> uint32_t {
  etl::io_port_ro<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(
      base_addr_ + cpuid * kSTRIDE + off)};
  return reg.read();
}

__always_inline void Write(uint32_t cpuid, uint32_t off,
                           uint32_t val) const {
  etl::io_port_wo<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(
      base_addr_ + cpuid * kSTRIDE + off)};
  reg.write(val);
}
```

### Include changes at top of `gic.h`

Current includes (lines ~8–13):
```cpp
#include <array>
#include <cstdint>

#include "io.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
```

Replace with (remove `io.hpp`, add `etl/io_port.h` in the third-party section):
```cpp
#include <array>
#include <cstdint>

#include <etl/io_port.h>

#include "kernel_log.hpp"
#include "per_cpu.hpp"
```

> **Note:** `etl_profile.h` does not need to be explicitly included here — `etl/io_port.h` includes it via `<etl/platform.h>`. Verify this by checking `3rd/etl/include/etl/io_port.h` line 1-10.

**Step 1: Apply all three edits to `gic.h`**

(Use the Edit tool: replace include block, replace `Gicd::Read`, replace `Gicd::Write`, replace `Gicr::Read`, replace `Gicr::Write`)

**Step 2: Run format check**

```bash
pre-commit run --files src/arch/aarch64/gic/include/gic.h
```

**Step 3: Attempt build for aarch64 (optional if cross-toolchain available)**

```bash
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel 2>&1 | head -30
```

**Step 4: Commit**

```bash
git add src/arch/aarch64/gic/include/gic.h
git commit --signoff -m "refactor(aarch64/gic): migrate io::In/Out to etl::io_port in GIC Read/Write helpers"
```

---

## Task 3: Migrate x86_64 IO APIC — `io_apic.cpp`

**Files:**
- Modify: `src/arch/x86_64/apic/io_apic.cpp`

Only two private methods use `io::In`/`io::Out`:

```cpp
// io_apic.cpp lines 94–106
uint32_t IoApic::Read(uint32_t reg) const {
  io::Out<uint32_t>(base_address_ + kRegSel, reg);     // write selector
  return io::In<uint32_t>(base_address_ + kRegWin);    // read window
}

void IoApic::Write(uint32_t reg, uint32_t value) const {
  io::Out<uint32_t>(base_address_ + kRegSel, reg);     // write selector
  io::Out<uint32_t>(base_address_ + kRegWin, value);   // write window
}
```

Replace with:

```cpp
uint32_t IoApic::Read(uint32_t reg) const {
  etl::io_port_wo<uint32_t> sel{
      reinterpret_cast<volatile uint32_t*>(base_address_ + kRegSel)};
  sel.write(reg);
  etl::io_port_ro<uint32_t> win{
      reinterpret_cast<volatile uint32_t*>(base_address_ + kRegWin)};
  return win.read();
}

void IoApic::Write(uint32_t reg, uint32_t value) const {
  etl::io_port_wo<uint32_t> sel{
      reinterpret_cast<volatile uint32_t*>(base_address_ + kRegSel)};
  sel.write(reg);
  etl::io_port_wo<uint32_t> win{
      reinterpret_cast<volatile uint32_t*>(base_address_ + kRegWin)};
  win.write(value);
}
```

### Include changes

Current (lines 5–8):
```cpp
#include "io_apic.h"

#include "io.hpp"
#include "kernel_log.hpp"
```

Replace with:
```cpp
#include "io_apic.h"

#include <etl/io_port.h>

#include "kernel_log.hpp"
```

**Step 1: Apply edits to `io_apic.cpp`**

**Step 2: Run format check**

```bash
pre-commit run --files src/arch/x86_64/apic/io_apic.cpp
```

**Step 3: Attempt build for x86_64**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | head -30
```

**Step 4: Commit**

```bash
git add src/arch/x86_64/apic/io_apic.cpp
git commit --signoff -m "refactor(x86_64/apic): migrate io::In/Out to etl::io_port in IoApic::Read/Write"
```

---

## Task 4: Migrate x86_64 Local APIC — `local_apic.cpp`

**Files:**
- Modify: `src/arch/x86_64/apic/local_apic.cpp`

This is the largest migration. All `io::In`/`io::Out` calls are in the `else` (xAPIC) branches of `if (is_x2apic_mode_)` guards. Complete inventory (459 lines total):

| Lines | Function | Access type | Address expression |
|-------|----------|-------------|-------------------|
| 31 | `Init` | `io::In` | `apic_base_ + kXApicSivrOffset` |
| 42 | `Init` | `io::Out` | `apic_base_ + kXApicSivrOffset` |
| 56–59 | `Init` | `io::Out` ×4 | LVT offsets |
| 69 | `GetApicVersion` | `io::In` | `apic_base_ + kXApicVersionOffset` |
| 78 | `SendEoi` | `io::Out` | `apic_base_ + kXApicEoiOffset` |
| 98 | `SendIpi` | `io::Out` | `apic_base_ + kXApicIcrHighOffset` |
| 102 | `SendIpi` | `io::Out` | `apic_base_ + kXApicIcrLowOffset` |
| 104 | `SendIpi` | `io::In` (poll) | `apic_base_ + kXApicIcrLowOffset` |
| 126 | `BroadcastIpi` | `io::Out` | `apic_base_ + kXApicIcrHighOffset` |
| 130 | `BroadcastIpi` | `io::Out` | `apic_base_ + kXApicIcrLowOffset` |
| 132 | `BroadcastIpi` | `io::In` (poll) | `apic_base_ + kXApicIcrLowOffset` |
| 144 | `SetTaskPriority` | `io::Out` | `apic_base_ + kXApicTprOffset` |
| 153 | `GetTaskPriority` | `io::In` | `apic_base_ + kXApicTprOffset` |
| 172 | `EnableTimer` | `io::Out` | `apic_base_ + kXApicTimerDivideOffset` |
| 180 | `EnableTimer` | `io::Out` | `apic_base_ + kXApicLvtTimerOffset` |
| 183 | `EnableTimer` | `io::Out` | `apic_base_ + kXApicTimerInitCountOffset` |
| 194 | `DisableTimer` | `io::In` | `apic_base_ + kXApicLvtTimerOffset` |
| 196 | `DisableTimer` | `io::Out` | `apic_base_ + kXApicLvtTimerOffset` |
| 197 | `DisableTimer` | `io::Out` | `apic_base_ + kXApicTimerInitCountOffset` |
| 206 | `GetTimerCurrentCount` | `io::In` | `apic_base_ + kXApicTimerCurrCountOffset` |
| 260 | `SendInitIpi` | `io::Out` | `apic_base_ + kXApicIcrHighOffset` |
| 264 | `SendInitIpi` | `io::Out` | `apic_base_ + kXApicIcrLowOffset` |
| 266 | `SendInitIpi` | `io::In` (poll) | `apic_base_ + kXApicIcrLowOffset` |
| 290 | `SendStartupIpi` | `io::Out` | `apic_base_ + kXApicIcrHighOffset` |
| 294 | `SendStartupIpi` | `io::Out` | `apic_base_ + kXApicIcrLowOffset` |
| 296 | `SendStartupIpi` | `io::In` (poll) | `apic_base_ + kXApicIcrLowOffset` |
| 315 | `ConfigureLvtEntries` | `io::Out` | LVT offsets ×3 |
| 333 | `ReadErrorStatus` | `io::Out` | `apic_base_ + kXApicEsrOffset` |
| 334 | `ReadErrorStatus` | `io::In` | `apic_base_ + kXApicEsrOffset` |
| 363 | `PrintInfo` | `io::In` | `apic_base_ + kXApicSivrOffset` |
| 367 | `PrintInfo` | `io::In` | `apic_base_ + kXApicLvtTimerOffset` |
| 370 | `PrintInfo` | `io::In` | `apic_base_ + kXApicLvtLint0Offset` |
| 373 | `PrintInfo` | `io::In` | `apic_base_ + kXApicLvtLint1Offset` |
| 376 | `PrintInfo` | `io::In` | `apic_base_ + kXApicLvtErrorOffset` |

### Conversion rules

- `io::In<uint32_t>(addr)` → declare `etl::io_port_ro<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(addr)};` then `reg.read()`
- `io::Out<uint32_t>(addr, val)` → declare `etl::io_port_wo<uint32_t> reg{reinterpret_cast<volatile uint32_t*>(addr)};` then `reg.write(val)`
- When the **same address** is both read and written in the **same scope** (e.g., `DisableTimer` reads then writes `kXApicLvtTimerOffset`; `ReadErrorStatus` writes-then-reads `kXApicEsrOffset`) → use `etl::io_port_rw<uint32_t>` for both operations on that address

### Example conversions

**Single write (e.g., `SendEoi`):**

Before:
```cpp
io::Out<uint32_t>(apic_base_ + kXApicEoiOffset, 0);
```
After:
```cpp
etl::io_port_wo<uint32_t> eoi_reg{
    reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicEoiOffset)};
eoi_reg.write(0);
```

**Single read (e.g., `GetApicVersion`):**

Before:
```cpp
return io::In<uint32_t>(apic_base_ + kXApicVersionOffset);
```
After:
```cpp
etl::io_port_ro<uint32_t> ver_reg{
    reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicVersionOffset)};
return ver_reg.read();
```

**Poll loop (e.g., `SendIpi`):**

Before:
```cpp
while ((io::In<uint32_t>(apic_base_ + kXApicIcrLowOffset) &
        kIcrDeliveryStatusBit) != 0) {
  ;
}
```
After:
```cpp
{
  etl::io_port_ro<uint32_t> icr_low{
      reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrLowOffset)};
  while ((icr_low.read() & kIcrDeliveryStatusBit) != 0) {
    ;
  }
}
```

**Read-modify-write same address (e.g., `DisableTimer`):**

Before:
```cpp
auto lvt_timer = io::In<uint32_t>(apic_base_ + kXApicLvtTimerOffset);
lvt_timer |= kLvtMaskBit;
io::Out<uint32_t>(apic_base_ + kXApicLvtTimerOffset, lvt_timer);
```
After:
```cpp
etl::io_port_rw<uint32_t> lvt_timer_reg{
    reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicLvtTimerOffset)};
auto lvt_timer = lvt_timer_reg.read();
lvt_timer |= kLvtMaskBit;
lvt_timer_reg.write(lvt_timer);
```

**Write-then-read same address (e.g., `ReadErrorStatus`):**

Before:
```cpp
io::Out<uint32_t>(apic_base_ + kXApicEsrOffset, 0);
return io::In<uint32_t>(apic_base_ + kXApicEsrOffset);
```
After:
```cpp
etl::io_port_rw<uint32_t> esr_reg{
    reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicEsrOffset)};
esr_reg.write(0);
return esr_reg.read();
```

### Include changes

Current (lines 5–7):
```cpp
#include "apic.h"
#include "io.hpp"
#include "kernel_log.hpp"
```

Replace with:
```cpp
#include "apic.h"

#include <etl/io_port.h>

#include "kernel_log.hpp"
```

**Step 1: Apply all edits to `local_apic.cpp`**

Work function-by-function, in file order. Apply every `io::In`/`io::Out` → `etl::io_port_*` conversion listed above.

**Step 2: Run format check**

```bash
pre-commit run --files src/arch/x86_64/apic/local_apic.cpp
```

**Step 3: Build for x86_64**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | head -50
```

Expected: clean build, 0 errors.

**Step 4: Run unit tests (host-only)**

```bash
cd build_x86_64 && make unit-test 2>&1 | tail -20
```

Expected: all tests pass (or only pre-existing failures).

**Step 5: Commit**

```bash
git add src/arch/x86_64/apic/local_apic.cpp
git commit --signoff -m "refactor(x86_64/apic): migrate io::In/Out to etl::io_port in LocalApic xAPIC branches"
```

---

## Task 5: Migrate VirtIO Block Driver — `virtio_blk_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

Two `io::In<uint32_t>` calls in `Probe()` (lines 49 and 57–58):

```cpp
auto magic = io::In<uint32_t>(base);
// ...
auto device_id = io::In<uint32_t>(
    base + device_framework::virtio::MmioTransport<>::MmioReg::kDeviceId);
```

Replace with:

```cpp
etl::io_port_ro<uint32_t> magic_reg{
    reinterpret_cast<volatile uint32_t*>(base)};
auto magic = magic_reg.read();
// ...
etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<volatile uint32_t*>(
    base + device_framework::virtio::MmioTransport<>::MmioReg::kDeviceId)};
auto device_id = device_id_reg.read();
```

### Include changes

Current (lines 14–15):
```cpp
#include "io.hpp"
#include "kernel_log.hpp"
```

Replace with:
```cpp
#include <etl/io_port.h>

#include "kernel_log.hpp"
```

> `etl/io_port.h` is a system/third-party header — it belongs before project headers per Google style.

**Step 1: Apply edits to `virtio_blk_driver.hpp`**

**Step 2: Run format check**

```bash
pre-commit run --files src/device/include/driver/virtio_blk_driver.hpp
```

**Step 3: Build (any arch that includes VirtIO — typically riscv64 or x86_64)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | grep -E "error:|warning:" | head -20
```

**Step 4: Commit**

```bash
git add src/device/include/driver/virtio_blk_driver.hpp
git commit --signoff -m "refactor(device/virtio): migrate io::In to etl::io_port in VirtioBlkDriver::Probe"
```

---

## Task 6: Delete `src/include/io.hpp`

**Files:**
- Delete: `src/include/io.hpp`

Before deleting, verify zero remaining references:

**Step 1: Grep for any remaining `io.hpp` includes**

```bash
grep -r '"io.hpp"' src/ --include="*.hpp" --include="*.h" --include="*.cpp"
```

Expected output: **empty** (no matches).

**Step 2: Grep for remaining `io::In` or `io::Out` calls**

```bash
grep -rn 'io::In\|io::Out' src/ --include="*.hpp" --include="*.h" --include="*.cpp"
```

Expected output: **empty** (no matches).

**Step 3: Delete the file**

```bash
git rm src/include/io.hpp
```

**Step 4: Build all three architectures**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | grep -c "error:"
```

Expected: 0 errors.

```bash
cd .. && cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel 2>&1 | grep -c "error:"
```

Expected: 0 errors.

```bash
cd .. && cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | grep -c "error:"
```

Expected: 0 errors.

**Step 5: Run unit tests one final time**

```bash
cd build_x86_64 && make unit-test 2>&1 | tail -5
```

**Step 6: Run pre-commit on all changed files**

```bash
pre-commit run --all-files
```

**Step 7: Commit**

```bash
git commit --signoff -m "refactor: delete io.hpp after full migration to etl::io_port"
```

---

## Checklist (completion criteria)

- [ ] Task 1: `interrupt.cpp` and `interrupt_main.cpp` no longer include `io.hpp`
- [ ] Task 2: `gic.h` `Gicd::Read/Write` and `Gicr::Read/Write` use `etl::io_port_ro/wo`
- [ ] Task 3: `io_apic.cpp` `Read()`/`Write()` use `etl::io_port_wo/ro`
- [ ] Task 4: `local_apic.cpp` all xAPIC branches use `etl::io_port_ro/wo/rw`
- [ ] Task 5: `virtio_blk_driver.hpp` `Probe()` uses `etl::io_port_ro`
- [ ] Task 6: `io.hpp` deleted, zero remaining references
- [ ] All three arch builds succeed
- [ ] Unit tests pass
- [ ] `pre-commit run --all-files` clean

---

## Reference: ETL io_port API

```cpp
// etl::io_port_ro<T> — read-only
template <typename T, uintptr_t Address = 0>
class io_port_ro {
 public:
  explicit io_port_ro(volatile T* addr);  // runtime mode B
  T read() const;
};

// etl::io_port_wo<T> — write-only
template <typename T, uintptr_t Address = 0>
class io_port_wo {
 public:
  explicit io_port_wo(volatile T* addr);  // runtime mode B
  void write(T value) const;
};

// etl::io_port_rw<T> — read-write
template <typename T, uintptr_t Address = 0>
class io_port_rw {
 public:
  explicit io_port_rw(volatile T* addr);  // runtime mode B
  T read() const;
  void write(T value) const;
};
```

Header: `#include <etl/io_port.h>`

All types live in namespace `etl`. No initialization cost; constructing a local instance is equivalent to storing the pointer.
