# Device Directory Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure `src/device/` so each driver lives in its own subdirectory with an
independent `ADD_LIBRARY(xxx_driver INTERFACE)` CMake target; eliminate `detail/`; make the
`device` target STATIC with PRIVATE driver dependencies so consumers can only access hardware
through `device_manager`.

**Architecture:** Each driver directory (`ns16550a/`, `pl011/`, `acpi/`, `virtio/`) becomes a
self-contained CMake INTERFACE library. The `device` STATIC target links them PRIVATELY and
exposes only `include/` (framework headers) as PUBLIC. External modules that need direct driver
access (arch code) must explicitly `TARGET_LINK_LIBRARIES(... xxx_driver)`.

**Tech Stack:** C++23, CMake 3.x (UPPERCASE style), ETL, no new `.cpp` files (only
`device.cpp` and `virtio_driver.cpp` are allowed per `src/device/AGENTS.md`).

---

## Pre-flight: understand the pre-existing breakage

**Before touching any file**, confirm the aarch64 build is broken with these specific errors:

```bash
cd build_aarch64 && make SimpleKernel 2>&1 | grep "error:" | head -10
```

Expected: errors about `Pl011Device`, `Pl011Singleton`, `Open(OpenFlags`, `HandleInterrupt`.
These are pre-existing bugs from the prior refactor — this plan fixes them as part of Task 7.

---

## Task 1: Create driver subdirectories and move ns16550a files

**Files:**
- Create: `src/device/ns16550a/` (directory)
- Create: `src/device/ns16550a/mmio_accessor.hpp` (moved from `include/driver/detail/mmio_accessor.hpp`)
- Create: `src/device/ns16550a/ns16550a.hpp` (moved from `include/driver/detail/ns16550a/ns16550a.hpp`)
- Create: `src/device/ns16550a/ns16550a_driver.hpp` (moved from `include/driver/ns16550a_driver.hpp`)

**Step 1: Copy mmio_accessor.hpp to ns16550a/**

New file `src/device/ns16550a/mmio_accessor.hpp`:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_NS16550A_MMIO_ACCESSOR_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_NS16550A_MMIO_ACCESSOR_HPP_

#include <cstddef>
#include <cstdint>

namespace detail {

/**
 * @brief 通用 MMIO 寄存器访问器
 *
 * 封装 volatile 指针的 MMIO 读写操作，消除各驱动中重复的
 * reinterpret_cast<volatile T*> 样板代码。
 *
 * 支持任意宽度的寄存器访问（uint8_t / uint16_t / uint32_t / uint64_t）。
 */
class MmioAccessor {
 public:
  explicit MmioAccessor(uint64_t base = 0) : base_(base) {}

  template <typename T>
  [[nodiscard]] auto Read(size_t offset) const -> T {
    return *reinterpret_cast<volatile T*>(base_ + offset);
  }

  template <typename T>
  auto Write(size_t offset, T val) const -> void {
    *reinterpret_cast<volatile T*>(base_ + offset) = val;
  }

  [[nodiscard]] auto base() const -> uint64_t { return base_; }

 private:
  uint64_t base_;
};

}  // namespace detail

#endif  // SIMPLEKERNEL_SRC_DEVICE_NS16550A_MMIO_ACCESSOR_HPP_
```

**Step 2: Copy ns16550a.hpp to ns16550a/**

New file `src/device/ns16550a/ns16550a.hpp`:
- Copy verbatim from `src/device/include/driver/detail/ns16550a/ns16550a.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_NS16550A_NS16550A_HPP_`
- Change `#include "driver/detail/mmio_accessor.hpp"` → `#include "ns16550a/mmio_accessor.hpp"`

**Step 3: Copy ns16550a_driver.hpp to ns16550a/**

New file `src/device/ns16550a/ns16550a_driver.hpp`:
- Copy verbatim from `src/device/include/driver/ns16550a_driver.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_NS16550A_NS16550A_DRIVER_HPP_`
- Change `#include "driver/detail/ns16550a/ns16550a.hpp"` → `#include "ns16550a/ns16550a.hpp"`
- All other includes stay the same (they resolve relative to `src/device/include/` which
  remains on the include path)

**Step 4: Verify no typos**

```bash
head -10 src/device/ns16550a/ns16550a.hpp
head -10 src/device/ns16550a/ns16550a_driver.hpp
grep "mmio_accessor" src/device/ns16550a/ns16550a.hpp
```

---

## Task 2: Create pl011 subdirectory and files

**Files:**
- Create: `src/device/pl011/` (directory)
- Create: `src/device/pl011/pl011.hpp` (moved from `include/driver/detail/pl011/pl011.hpp`)
- Create: `src/device/pl011/pl011_driver.hpp` (replaces `include/driver/pl011_driver.hpp`)

**Step 1: Copy pl011.hpp to pl011/**

New file `src/device/pl011/pl011.hpp`:
- Copy verbatim from `src/device/include/driver/detail/pl011/pl011.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_HPP_`
- Change namespace from `detail::pl011` → `pl011`
- Change `#include "driver/detail/mmio_accessor.hpp"` → `#include "ns16550a/mmio_accessor.hpp"`
- Change `detail::MmioAccessor mmio_` → `detail::MmioAccessor mmio_` (namespace is unchanged
  for MmioAccessor — it stays in `namespace detail`)
- **Add** the `HandleInterrupt` method (needed by aarch64 interrupt_main.cpp):

```cpp
  /**
   * @brief 处理串口接收中断，对每个收到的字符调用回调
   * @param callback 字符接收回调，签名 void(uint8_t ch)
   */
  template <typename Callback>
  auto HandleInterrupt(Callback&& callback) -> void {
    while (HasData()) {
      callback(GetChar());
    }
  }
```

**Step 2: Create pl011_driver.hpp**

New file `src/device/pl011/pl011_driver.hpp`:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief PL011 UART driver public interface
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_

#include "pl011/pl011.hpp"

namespace pl011 {
/// Type alias for singleton usage (etl::singleton<pl011::Pl011Device>)
using Pl011Device = Pl011;
}  // namespace pl011

#endif  // SIMPLEKERNEL_SRC_DEVICE_PL011_PL011_DRIVER_HPP_
```

**Step 3: Verify**

```bash
grep "Pl011Device\|HandleInterrupt\|namespace pl011" src/device/pl011/pl011.hpp
grep "Pl011Device" src/device/pl011/pl011_driver.hpp
```

---

## Task 3: Create acpi subdirectory and files

**Files:**
- Create: `src/device/acpi/` (directory)
- Create: `src/device/acpi/acpi.hpp` (moved from `include/driver/detail/acpi/acpi.hpp`)
- Create: `src/device/acpi/acpi_driver.hpp` (replaces `include/driver/acpi_driver.hpp`)

**Step 1: Copy acpi.hpp to acpi/**

New file `src/device/acpi/acpi.hpp`:
- Copy verbatim from `src/device/include/driver/detail/acpi/acpi.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_ACPI_ACPI_HPP_`
- Change namespace from `detail::acpi` → `acpi`

**Step 2: Create acpi_driver.hpp**

New file `src/device/acpi/acpi_driver.hpp`:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ACPI driver public interface
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_ACPI_ACPI_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_ACPI_ACPI_DRIVER_HPP_

#include "acpi/acpi.hpp"

#endif  // SIMPLEKERNEL_SRC_DEVICE_ACPI_ACPI_DRIVER_HPP_
```

---

## Task 4: Create virtio subdirectory and files

**Files:**
- Create: `src/device/virtio/` (directory)
- Move entire `src/device/include/driver/detail/virtio/` subtree into `src/device/virtio/`
- Create: `src/device/virtio/virtio_driver.hpp` (replaces `include/driver/virtio_driver.hpp`)
- Move: `src/device/virtio_driver.cpp` → `src/device/virtio/virtio_driver.cpp`

**Step 1: Copy virtio/ subtree**

Copy these files, preserving subdirectory structure:
- `src/device/include/driver/detail/virtio/defs.h` → `src/device/virtio/defs.h`
- `src/device/include/driver/detail/virtio/traits.hpp` → `src/device/virtio/traits.hpp`
  - Change include guard: `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_TRAITS_HPP_`
- `src/device/include/driver/detail/virtio/platform_config.hpp` → `src/device/virtio/platform_config.hpp`
  - Change include guard accordingly
- All files under `src/device/include/driver/detail/virtio/device/` → `src/device/virtio/device/`
- All files under `src/device/include/driver/detail/virtio/transport/` → `src/device/virtio/transport/`
- All files under `src/device/include/driver/detail/virtio/virt_queue/` → `src/device/virtio/virt_queue/`

For each copied file:
- Update include guards: replace `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_` with
  `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_`
- Update internal `#include "driver/detail/virtio/...` → `#include "virtio/..."`

**Step 2: Create virtio_driver.hpp**

New file `src/device/virtio/virtio_driver.hpp`:
- Copy verbatim from `src/device/include/driver/virtio_driver.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_VIRTIO_VIRTIO_DRIVER_HPP_`
- Change includes:
  - `#include "driver/detail/virtio/device/virtio_blk.hpp"` → `#include "virtio/device/virtio_blk.hpp"`
  - `#include "driver/detail/virtio/device/virtio_blk_vfs_adapter.hpp"` → `#include "virtio/device/virtio_blk_vfs_adapter.hpp"`
  - `#include "driver/detail/virtio/traits.hpp"` → `#include "virtio/traits.hpp"`

**Step 3: Create virtio/virtio_driver.cpp**

Copy `src/device/virtio_driver.cpp` to `src/device/virtio/virtio_driver.cpp`.
Update its includes:
- `#include "driver/virtio_driver.hpp"` (if present) → `#include "virtio/virtio_driver.hpp"`

**Step 4: Read the virtio subtree files to find all internal cross-references**

```bash
grep -r '"driver/detail/virtio/' src/device/include/driver/detail/virtio/
```

Update every `"driver/detail/virtio/X"` → `"virtio/X"` in the copied files.

---

## Task 5: Add traits.hpp and mmio_accessor.hpp shared locations

**Files:**
- Create: `src/device/traits.hpp` (moved from `include/driver/traits.hpp`)

The `traits.hpp` at `src/device/include/driver/traits.hpp` is framework-level (used by
`virtio_driver.hpp`). Move it to `src/device/traits.hpp` and update its include guard.

New file `src/device/traits.hpp`:
- Copy verbatim from `src/device/include/driver/traits.hpp`
- Change include guard to: `SIMPLEKERNEL_SRC_DEVICE_TRAITS_HPP_`

Update any `#include "driver/traits.hpp"` in virtio headers → `#include "traits.hpp"`.

> Note: `mmio_accessor.hpp` is placed in `ns16550a/mmio_accessor.hpp` since only ns16550a and
> pl011 use it, and both live at the device root level. pl011 includes it as
> `"ns16550a/mmio_accessor.hpp"` — this is intentional (shared utility, documented exception).

---

## Task 6: Create CMakeLists.txt for each driver

**Step 1: `src/device/ns16550a/CMakeLists.txt`**

```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (ns16550a_driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (ns16550a_driver
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/..)
```

> The include root is `src/device/` so consumers include as `"ns16550a/ns16550a.hpp"`.

**Step 2: `src/device/pl011/CMakeLists.txt`**

```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (pl011_driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (pl011_driver
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/..)
```

**Step 3: `src/device/acpi/CMakeLists.txt`**

```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (acpi_driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (acpi_driver
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/..)
```

**Step 4: `src/device/virtio/CMakeLists.txt`**

```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (virtio_driver INTERFACE)

TARGET_INCLUDE_DIRECTORIES (virtio_driver
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/..)

TARGET_SOURCES (virtio_driver INTERFACE virtio_driver.cpp)
```

> `virtio_driver.cpp` is the one permitted `.cpp` for VirtIO logic. It becomes an INTERFACE
> source so it is compiled into whatever target links `virtio_driver`.

---

## Task 7: Rewrite src/device/CMakeLists.txt

Replace the current 8-line file with:

```cmake
# Copyright The SimpleKernel Contributors

ADD_SUBDIRECTORY (ns16550a)
ADD_SUBDIRECTORY (pl011)
ADD_SUBDIRECTORY (acpi)
ADD_SUBDIRECTORY (virtio)

ADD_LIBRARY (device STATIC device.cpp)

TARGET_INCLUDE_DIRECTORIES (device
    PUBLIC  ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})

TARGET_LINK_LIBRARIES (device
    PRIVATE kernel_link_libraries
            ns16550a_driver
            pl011_driver
            acpi_driver
            virtio_driver)
```

> The PRIVATE `${CMAKE_CURRENT_SOURCE_DIR}` exposes the root of `src/device/` (where
> `traits.hpp`, `ns16550a/`, `pl011/`, `acpi/`, `virtio/` live) to `device.cpp` only.
> External consumers only see `include/` (device framework headers).

**Verify the file looks correct:**

```bash
cat src/device/CMakeLists.txt
```

---

## Task 8: Update device.cpp includes

Read `src/device/device.cpp` and update any includes that reference old paths:
- `"driver/ns16550a_driver.hpp"` → `"ns16550a/ns16550a_driver.hpp"`
- `"driver/pl011_driver.hpp"` → `"pl011/pl011_driver.hpp"`
- `"driver/virtio_driver.hpp"` → `"virtio/virtio_driver.hpp"`
- `"driver/acpi_driver.hpp"` → `"acpi/acpi_driver.hpp"`

Also check `src/device/virtio_driver.cpp` (at root) — it will be **deleted** after the move to
`virtio/` in Task 4. Confirm its replacement is at `src/device/virtio/virtio_driver.cpp`.

---

## Task 9: Update src/filesystem/CMakeLists.txt

**File:** `src/filesystem/CMakeLists.txt`

Current line 6:
```cmake
TARGET_INCLUDE_DIRECTORIES (
    filesystem INTERFACE include ${CMAKE_SOURCE_DIR}/src/device/include)
```

Change to (remove hardcoded device include path — `device` target exposes it via PUBLIC):
```cmake
TARGET_INCLUDE_DIRECTORIES (
    filesystem INTERFACE include)
```

Add `device` to link libraries (line 14):
```cmake
TARGET_LINK_LIBRARIES (filesystem INTERFACE vfs ramfs fatfs device)
```

> `device` is PUBLIC so `filesystem` consumers also get `src/device/include/`. The FatFS
> diskio.cpp includes `block_device_provider.hpp` which lives in `src/device/include/` —
> it will resolve via the transitive PUBLIC include path from `device`.

---

## Task 10: Update arch includes — aarch64

These files currently include old paths. Fix them.

### `src/arch/aarch64/include/pl011_singleton.h`

Change:
```cpp
#include "driver/pl011_driver.hpp"
```
To:
```cpp
#include "pl011/pl011_driver.hpp"
```

### `src/arch/aarch64/early_console.cpp`

Current (broken):
```cpp
#include "driver/pl011_driver.hpp"
...
pl011::Pl011Device* pl011 = nullptr;
...
auto result = pl011->Open(OpenFlags{OpenFlags::kReadWrite});
if (result) {
  sk_putchar = console_putchar;
}
```

Fixed version:
```cpp
#include "pl011/pl011_driver.hpp"
...
pl011::Pl011Device* pl011_uart = nullptr;  // rename to avoid shadowing namespace
...
void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011_uart) {
    pl011_uart->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011_uart = &Pl011Singleton::instance();
    sk_putchar = console_putchar;  // Pl011 ctor already initializes HW, no Open()
  }
};
```

> `Pl011::Pl011(uint64_t dev_addr, ...)` already does full hardware init in the constructor.
> There is no `Open()` method — the prior code was leftover from an older interface.

### `src/arch/aarch64/interrupt_main.cpp`

Change:
```cpp
#include "driver/pl011_driver.hpp"
```
To:
```cpp
#include "pl011/pl011_driver.hpp"
```

The `HandleInterrupt` call on line 144:
```cpp
Pl011Singleton::instance().HandleInterrupt(
    [](uint8_t ch) { sk_putchar(ch, nullptr); });
```
This now resolves correctly because `HandleInterrupt(Callback&&)` was added to `Pl011` in Task 2.

---

## Task 11: Update arch includes — riscv64

### `src/arch/riscv64/interrupt_main.cpp`

Change:
```cpp
#include "driver/detail/ns16550a/ns16550a.hpp"
#include "driver/virtio_driver.hpp"
```
To:
```cpp
#include "ns16550a/ns16550a.hpp"
#include "virtio/virtio_driver.hpp"
```

Line 22, the singleton type alias:
```cpp
using Ns16550aSingleton = etl::singleton<detail::ns16550a::Ns16550a>;
```
The namespace is unchanged: `detail::ns16550a::Ns16550a` still exists in `ns16550a.hpp`
(only the include path changed). ✓

---

## Task 12: Update src/arch CMakeLists to link driver targets

**File:** `src/arch/CMakeLists.txt`

Read this file first. Arch code (riscv64, aarch64) needs direct driver access as a documented
exception. Add explicit `TARGET_LINK_LIBRARIES` for the arch INTERFACE targets.

Find where `arch` target is defined. If there is a per-arch target (e.g. `arch_aarch64`,
`arch_riscv64`), add:

```cmake
# aarch64 arch needs direct PL011 access for early_console and interrupt_main
TARGET_LINK_LIBRARIES (arch_aarch64 INTERFACE pl011_driver)

# riscv64 arch needs direct NS16550A and VirtIO access for interrupt_main
TARGET_LINK_LIBRARIES (arch_riscv64 INTERFACE ns16550a_driver virtio_driver)
```

If there is a single `arch` INTERFACE target that includes all arches, the correct approach is:
```cmake
# Documented exception: arch code needs direct driver access
# - aarch64: pl011_driver (early_console, interrupt_main)
# - riscv64: ns16550a_driver, virtio_driver (interrupt_main)
TARGET_LINK_LIBRARIES (arch INTERFACE pl011_driver ns16550a_driver virtio_driver)
```

> Including all three in one INTERFACE is safe since unused INTERFACE libraries add zero
> overhead in a freestanding kernel (header-only).

---

## Task 13: First build verification — x86_64

x86_64 does not use PL011 or NS16550A directly in arch code — it's the simplest to verify first.

```bash
cmake --preset build_x86_64
cd build_x86_64 && make SimpleKernel -j$(nproc) 2>&1 | tail -40
```

Expected: **clean build, exit 0**.

If there are include errors, they will point to old path references. Fix them by searching:
```bash
grep -r '"driver/detail/' src/ --include="*.hpp" --include="*.cpp" --include="*.h"
grep -r '"driver/ns16550a\|driver/pl011\|driver/acpi\|driver/virtio' src/ --include="*.hpp" --include="*.cpp" --include="*.h"
```

---

## Task 14: Second build verification — riscv64

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel -j$(nproc) 2>&1 | tail -40
```

Expected: **clean build, exit 0**.

Common failure point: `interrupt_main.cpp` uses `detail::ns16550a::Ns16550a` — confirm the
type is still in that namespace after moving to `ns16550a/ns16550a.hpp`.

---

## Task 15: Third build verification — aarch64

```bash
cmake --preset build_aarch64
cd build_aarch64 && make SimpleKernel -j$(nproc) 2>&1 | tail -40
```

Expected: **clean build, exit 0**.

The pre-existing `Pl011Device` / `Open()` / `HandleInterrupt` errors should all be gone
after Tasks 2, 10, 12.

---

## Task 16: Delete old files

Only delete after all three builds pass.

Files/directories to delete:
- `src/device/include/driver/detail/` (entire directory — all contents moved)
- `src/device/include/driver/ns16550a_driver.hpp`
- `src/device/include/driver/pl011_driver.hpp`
- `src/device/include/driver/acpi_driver.hpp`
- `src/device/include/driver/virtio_driver.hpp`
- `src/device/include/driver/traits.hpp`
- `src/device/virtio_driver.cpp` (at root — moved to `virtio/`)

```bash
rm -rf src/device/include/driver/detail/
rm src/device/include/driver/ns16550a_driver.hpp
rm src/device/include/driver/pl011_driver.hpp
rm src/device/include/driver/acpi_driver.hpp
rm src/device/include/driver/virtio_driver.hpp
rm src/device/include/driver/traits.hpp
rm src/device/virtio_driver.cpp
```

After deletion, verify the `include/driver/` directory is now empty or gone:
```bash
ls src/device/include/driver/
```

Expected: empty directory (or you can `rmdir src/device/include/driver/` if empty).

---

## Task 17: Final build verification — all three arches

Run all three back-to-back:

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel -j$(nproc) 2>&1 | tail -5 && cd ..
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel -j$(nproc) 2>&1 | tail -5 && cd ..
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel -j$(nproc) 2>&1 | tail -5 && cd ..
```

All three must exit 0.

---

## Task 18: Run unit tests

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test -j$(nproc) 2>&1 | tail -20
```

Expected: all tests pass (or pre-existing failures only — do NOT fix pre-existing test failures
as part of this refactor).

---

## Task 19: Update src/device/AGENTS.md

Update `src/device/AGENTS.md` to reflect the new structure. Replace the STRUCTURE section with:

```markdown
## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
ns16550a/
  CMakeLists.txt        # ADD_LIBRARY(ns16550a_driver INTERFACE)
  mmio_accessor.hpp     # Shared MMIO register accessor
  ns16550a.hpp          # NS16550A UART hardware implementation
  ns16550a_driver.hpp   # NS16550A DriverEntry + Probe/Remove
pl011/
  CMakeLists.txt        # ADD_LIBRARY(pl011_driver INTERFACE)
  pl011.hpp             # PL011 UART hardware implementation + Pl011Device alias
  pl011_driver.hpp      # pl011::Pl011Device type alias (for singleton usage)
acpi/
  CMakeLists.txt        # ADD_LIBRARY(acpi_driver INTERFACE)
  acpi.hpp              # ACPI table structures
  acpi_driver.hpp       # ACPI driver interface
virtio/
  CMakeLists.txt        # ADD_LIBRARY(virtio_driver INTERFACE) + virtio_driver.cpp
  virtio_driver.hpp     # Unified VirtIO driver header
  virtio_driver.cpp     # VirtIO device-id dispatch (the only other .cpp)
  defs.h / traits.hpp / platform_config.hpp
  device/ transport/ virt_queue/
traits.hpp              # EnvironmentTraits / BarrierTraits / DmaTraits concepts
include/                # PUBLIC framework API (device_manager, driver_registry, bus, ...)
  device_manager.hpp / driver_registry.hpp / platform_bus.hpp / bus.hpp
  device_node.hpp / block_device_provider.hpp / mmio_helper.hpp
```
```

Also update the WHERE TO LOOK section to reference new paths.

---

## Task 20: Commit

```bash
git add src/device/ src/arch/ src/filesystem/CMakeLists.txt
git status
git commit -m "refactor(device): restructure drivers into subdirectories

- Each driver (ns16550a, pl011, acpi, virtio) now lives in its own
  subdirectory with an independent ADD_LIBRARY(xxx_driver INTERFACE) target
- Delete detail/ directory; files moved to driver subdirectories
- device target is now STATIC with PRIVATE driver dependencies;
  consumers access hardware only through device_manager
- Fix pre-existing aarch64 breakage: add Pl011Device alias, HandleInterrupt,
  remove stale Open() call in early_console.cpp
- Arch code links driver targets explicitly as documented exception

Signed-off-by: $(git config user.name) <$(git config user.email)>" --signoff
```

---

## Summary

| Task | Action |
|------|--------|
| 1 | Create `ns16550a/` with moved headers, updated include paths |
| 2 | Create `pl011/` with moved headers, add `HandleInterrupt`, `Pl011Device` alias |
| 3 | Create `acpi/` with moved headers |
| 4 | Create `virtio/` with moved subtree + `virtio_driver.cpp` |
| 5 | Move `traits.hpp` to `src/device/traits.hpp` |
| 6 | Create `CMakeLists.txt` for each driver subdirectory |
| 7 | Rewrite `src/device/CMakeLists.txt` (STATIC + PRIVATE links) |
| 8 | Update `device.cpp` includes |
| 9 | Update `src/filesystem/CMakeLists.txt` |
| 10 | Fix aarch64 arch includes + early_console.cpp + pl011_singleton.h |
| 11 | Fix riscv64 arch includes |
| 12 | Add `TARGET_LINK_LIBRARIES` in arch CMakeLists for documented exceptions |
| 13 | Build x86_64 ✓ |
| 14 | Build riscv64 ✓ |
| 15 | Build aarch64 ✓ |
| 16 | Delete old files |
| 17 | Final build verification — all 3 arches ✓ |
| 18 | Run unit tests |
| 19 | Update `src/device/AGENTS.md` |
| 20 | Commit |
