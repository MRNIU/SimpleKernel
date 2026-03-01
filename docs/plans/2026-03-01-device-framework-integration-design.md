# device_framework Integration Design

**Date**: 2026-03-01
**Status**: Approved
**Approach**: B — Fully flattened, no additional namespace

## Goal

Internalize the `3rd/device_framework` submodule into the kernel source tree under
`src/device/include/driver/`, eliminate the `device_framework::` namespace layer entirely,
and remove the submodule from the repository.

## Background

`device_framework` was an external submodule providing the driver abstraction layer
(CRTP device ops bases, UART/virtio driver implementations, MMIO helpers). It used its
own `device_framework::Error`/`Expected<T>` types that required a hack bridge at
`src/device/include/device_framework/platform_config.hpp` to alias them to kernel-native
types before the submodule headers were included. This design is being eliminated in
favour of inlining the headers directly into the kernel with native types used throughout.

## Section 1: Directory Structure

### Before

```
3rd/device_framework/include/device_framework/   ← submodule, external
src/device/include/
  device_framework/platform_config.hpp           ← hack bridge
  driver/
    ns16550a_driver.hpp
    virtio_blk_driver.hpp
```

### After

```
src/device/include/driver/
  defs.h                          ← DeviceType enum
  ops/
    device_ops_base.hpp           ← DeviceOperationsBase<Derived>
    char_device.hpp               ← CharDevice<Derived>
    block_device_ops.hpp          ← renamed from block_device.hpp (avoids vfs::BlockDevice clash)
  detail/
    storage.hpp                   ← Storage<T> placement-new helper
    mmio_accessor.hpp             ← MMIO access helper
    uart_device.hpp               ← UartDevice<Derived, DriverType>
    ns16550a/
      ns16550a.hpp
      ns16550a_device.hpp
    pl011/
      pl011.hpp
      pl011_device.hpp
    virtio/
      defs.h
      traits.hpp
      transport/
      virt_queue/
      device/
  ns16550a_driver.hpp             ← updated public entry
  pl011_driver.hpp                ← new public entry (replaces device_framework/pl011.hpp)
  virtio_blk_driver.hpp           ← updated public entry
  acpi_driver.hpp                 ← from device_framework/acpi.hpp
```

### Deleted

- `3rd/device_framework/` (submodule removed entirely)
- `src/device/include/device_framework/platform_config.hpp` (hack bridge gone)

## Section 2: Include Path & Namespace Changes

All `device_framework::` references are eliminated. Changes fall into four groups:

### Group 1 — Arch code

| File | Old include | New include |
|------|-------------|-------------|
| `src/arch/aarch64/early_console.cpp` | `<device_framework/pl011.hpp>` | `"driver/pl011_driver.hpp"` |
| `src/arch/aarch64/include/pl011_singleton.h` | `<device_framework/pl011.hpp>` | `"driver/pl011_driver.hpp"` |
| `src/arch/aarch64/interrupt_main.cpp` | `<device_framework/pl011.hpp>` | `"driver/pl011_driver.hpp"` |
| `src/arch/riscv64/interrupt_main.cpp` | `<device_framework/ns16550a.hpp>` | `"driver/ns16550a_driver.hpp"` |

### Group 2 — Driver headers (updated during file copy)

| Old | New |
|-----|-----|
| `#include <device_framework/expected.hpp>` | deleted — use kernel `#include "expected.hpp"` |
| `#include <device_framework/defs.h>` | `#include "driver/defs.h"` (relative) |
| `#include <device_framework/detail/...>` | `#include "detail/..."` (relative, within driver/) |
| `namespace device_framework {` wrappers | removed entirely |
| `device_framework::Error` / `Expected<T>` | `::Error` / `::Expected<T>` |

### Group 3 — `block_device.hpp` rename

`device_framework::BlockDevice<Derived>` is renamed to `BlockDeviceOps<Derived>` in
`src/device/include/driver/ops/block_device_ops.hpp` to avoid collision with
`vfs::BlockDevice`. All internal virtio code referencing it is updated accordingly.

### Group 4 — `platform_config.hpp` elimination

The hack at `src/device/include/device_framework/platform_config.hpp` (which aliased
`device_framework::ErrorCode = ::ErrorCode` before submodule headers were included) is
deleted. New driver headers use `::ErrorCode` directly.

## Section 3: CMake Changes

### `cmake/3rd.cmake`
Remove:
```cmake
ADD_SUBDIRECTORY(3rd/device_framework)
```

### `cmake/compile_config.cmake`
Remove from `TARGET_LINK_LIBRARIES(...)`:
```cmake
device_framework
```

### `.gitmodules`
Remove the `device_framework` submodule entry block.

### `src/device/CMakeLists.txt`
No changes needed. The `driver/` subdirectory is already under `src/device/include/`,
which is already on the include path via:
```cmake
TARGET_INCLUDE_DIRECTORIES(device BEFORE INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)
```

Arch targets already link against `device`, so the new headers are reachable with no
additional CMake changes.

## Section 4: Submodule Removal & Verification

### Git operations (in order)
1. `git submodule deinit -f 3rd/device_framework`
2. `git rm -f 3rd/device_framework`
3. `rm -rf .git/modules/3rd/device_framework`
4. Remove entry from `.gitmodules`

### Verification steps
1. `pre-commit run --all-files` — clang-format passes on all changed headers
2. `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel` — builds cleanly
3. `cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel` — builds cleanly
4. `cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel` — builds cleanly
5. `make unit-test` on host — existing device tests pass

No new tests are needed — this is a structural refactor with no behaviour change.

## Constraints

- All headers remain header-only (no new `.cpp` files introduced)
- No exceptions, RTTI, `dynamic_cast`, `typeid`
- No heap allocation before memory subsystem init
- No standard library dynamic containers — use `sk_`-prefixed versions
- Header guard format: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_FILENAME_HPP_`
- Copyright: `/** @copyright Copyright The SimpleKernel Contributors */`
- Style: Google C++ via `.clang-format`
