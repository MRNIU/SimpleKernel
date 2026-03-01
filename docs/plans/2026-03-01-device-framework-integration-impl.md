# device_framework Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the `3rd/device_framework` submodule and inline all its headers into `src/device/include/driver/`, eliminating the `device_framework::` namespace wrapper entirely.

**Architecture:** Copy every submodule header into `src/device/include/driver/`, strip `device_framework::` from all namespaces, replace `device_framework::Error/Expected<T>/ErrorCode` with kernel-native `::Error/::Expected<T>/::ErrorCode`, update all callers, remove submodule, delete CMake/gitmodule entries.

**Tech Stack:** C++23 header-only, clang-format, cmake, git submodule

---

## Transformation Rules (apply to every copied file)

These rules apply uniformly across all files. Reference them per-task instead of repeating:

| Old | New |
|-----|-----|
| `/** @copyright Copyright The device_framework Contributors */` | `/** @copyright Copyright The SimpleKernel Contributors */` |
| `#include "device_framework/expected.hpp"` | `#include "expected.hpp"` |
| `#include "device_framework/platform_config.hpp"` | *(delete line entirely)* |
| `#include "device_framework/defs.h"` | `#include "device_node.hpp"` |
| `#include "device_framework/traits.hpp"` | `#include "driver/traits.hpp"` |
| `#include "device_framework/ops/device_ops_base.hpp"` | `#include "driver/ops/device_ops_base.hpp"` |
| `#include "device_framework/ops/char_device.hpp"` | `#include "driver/ops/char_device.hpp"` |
| `#include "device_framework/ops/block_device.hpp"` | `#include "driver/ops/block_device_ops.hpp"` |
| `#include "device_framework/detail/X"` | `#include "driver/detail/X"` |
| `DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_` prefix in header guards | `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_` |
| `namespace device_framework {` / `}  // namespace device_framework` | *(delete both lines — promote contents to root namespace)* |
| `namespace device_framework::detail {` | `namespace detail {` |
| `namespace device_framework::detail::ns16550a {` | `namespace detail::ns16550a {` |
| `namespace device_framework::detail::pl011 {` | `namespace detail::pl011 {` |
| `namespace device_framework::detail::virtio {` | `namespace detail::virtio {` |
| `namespace device_framework::detail::virtio::blk {` | `namespace detail::virtio::blk {` |
| `namespace device_framework::detail::acpi {` | `namespace detail::acpi {` |
| `friend class ::device_framework::DeviceOperationsBase` | `friend class ::DeviceOperationsBase` |
| `friend class ::device_framework::CharDevice` | `friend class ::CharDevice` |

Header guard format: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_<PATH>_<EXT>_`
where `<PATH>` is the file path with `/` → `_` and `.` → `_`, uppercased.

---

## Task 1: Create directory skeleton

**Files:**
- Create directories under `src/device/include/driver/`

**Step 1: Create all new subdirectories**

```bash
mkdir -p src/device/include/driver/ops
mkdir -p src/device/include/driver/detail/ns16550a
mkdir -p src/device/include/driver/detail/pl011
mkdir -p src/device/include/driver/detail/virtio/transport
mkdir -p src/device/include/driver/detail/virtio/virt_queue
mkdir -p src/device/include/driver/detail/virtio/device
mkdir -p src/device/include/driver/detail/acpi
```

Run: `ls src/device/include/driver/`
Expected: `detail/  ns16550a_driver.hpp  ops/  virtio_blk_driver.hpp`

**Step 2: Commit**

```bash
git add -A
git commit --signoff -m "chore(device): create driver/ subdirectory skeleton"
```

---

## Task 2: Copy and adapt `traits.hpp`

**Files:**
- Create: `src/device/include/driver/traits.hpp`
- Source: `3rd/device_framework/include/device_framework/traits.hpp`

**Step 1: Write the adapted file**

Copy `traits.hpp`, apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_TRAITS_HPP_`
- Namespace: Remove `namespace device_framework { ... }` wrapper (promote `EnvironmentTraits`, `BarrierTraits`, `DmaTraits`, `SpinWaitTraits`, `NullTraits` to root namespace)
- No includes to change (only `<concepts>`, `<cstddef>`, `<cstdint>`)

**Step 2: Run clang-format**

```bash
clang-format -i src/device/include/driver/traits.hpp
```

**Step 3: Commit**

```bash
git add src/device/include/driver/traits.hpp
git commit --signoff -m "feat(device): add traits.hpp inlined from device_framework"
```

---

## Task 3: Copy and adapt `ops/device_ops_base.hpp`

**Files:**
- Create: `src/device/include/driver/ops/device_ops_base.hpp`
- Source: `3rd/device_framework/include/device_framework/ops/device_ops_base.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_OPS_DEVICE_OPS_BASE_HPP_`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- Remove `namespace device_framework { ... }` wrapper
- `OpenFlags`, `ProtFlags`, `MapFlags`, `DeviceOperationsBase<Derived>` are now at root namespace
- All `Error{...}`, `Expected<void>`, `ErrorCode::k...` references stay as-is (they resolve to root-namespace types)

**Step 2: Run clang-format and verify no device_framework references remain**

```bash
clang-format -i src/device/include/driver/ops/device_ops_base.hpp
grep "device_framework" src/device/include/driver/ops/device_ops_base.hpp
```
Expected: no output from grep.

**Step 3: Commit**

```bash
git add src/device/include/driver/ops/device_ops_base.hpp
git commit --signoff -m "feat(device): add ops/device_ops_base.hpp inlined from device_framework"
```

---

## Task 4: Copy and adapt `ops/char_device.hpp`

**Files:**
- Create: `src/device/include/driver/ops/char_device.hpp`
- Source: `3rd/device_framework/include/device_framework/ops/char_device.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_OPS_CHAR_DEVICE_HPP_`
- `#include "device_framework/defs.h"` → `#include "device_node.hpp"`
- `#include "device_framework/ops/device_ops_base.hpp"` → `#include "driver/ops/device_ops_base.hpp"`
- Remove `namespace device_framework { ... }` wrapper
- `PollEvents` and `CharDevice<Derived>` are now at root namespace

**Step 2: Run clang-format and verify**

```bash
clang-format -i src/device/include/driver/ops/char_device.hpp
grep "device_framework" src/device/include/driver/ops/char_device.hpp
```
Expected: no output.

**Step 3: Commit**

```bash
git add src/device/include/driver/ops/char_device.hpp
git commit --signoff -m "feat(device): add ops/char_device.hpp inlined from device_framework"
```

---

## Task 5: Copy and adapt `ops/block_device_ops.hpp`

**Files:**
- Create: `src/device/include/driver/ops/block_device_ops.hpp`
- Source: `3rd/device_framework/include/device_framework/ops/block_device.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_OPS_BLOCK_DEVICE_OPS_HPP_`
- `#include "device_framework/defs.h"` → `#include "device_node.hpp"`
- `#include "device_framework/ops/device_ops_base.hpp"` → `#include "driver/ops/device_ops_base.hpp"`
- Remove `namespace device_framework { ... }` wrapper
- **Rename class**: `template <class Derived> class BlockDevice` → `template <class Derived> class BlockDeviceOps`
  (avoids collision with `vfs::BlockDevice`)
- `BlockDeviceOps<Derived>` is now at root namespace

**Step 2: Run clang-format and verify**

```bash
clang-format -i src/device/include/driver/ops/block_device_ops.hpp
grep "device_framework\|class BlockDevice[^O]" src/device/include/driver/ops/block_device_ops.hpp
```
Expected: no output.

**Step 3: Commit**

```bash
git add src/device/include/driver/ops/block_device_ops.hpp
git commit --signoff -m "feat(device): add ops/block_device_ops.hpp inlined from device_framework"
```

---

## Task 6: Copy and adapt `detail/mmio_accessor.hpp`

**Files:**
- Create: `src/device/include/driver/detail/mmio_accessor.hpp`
- Source: `3rd/device_framework/include/device_framework/detail/mmio_accessor.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_MMIO_ACCESSOR_HPP_`
- Namespace: `namespace device_framework::detail {` → `namespace detail {`
- No includes to change (only `<cstddef>`, `<cstdint>`)

**Step 2: Commit**

```bash
clang-format -i src/device/include/driver/detail/mmio_accessor.hpp
git add src/device/include/driver/detail/mmio_accessor.hpp
git commit --signoff -m "feat(device): add detail/mmio_accessor.hpp inlined from device_framework"
```

---

## Task 7: Copy and adapt `detail/storage.hpp`

**Files:**
- Create: `src/device/include/driver/detail/storage.hpp`
- Source: `3rd/device_framework/include/device_framework/detail/storage.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_STORAGE_HPP_`
- Namespace: `namespace device_framework::detail {` → `namespace detail {`
- No includes to change (only `<cstdint>`, `<utility>`)

**Step 2: Commit**

```bash
clang-format -i src/device/include/driver/detail/storage.hpp
git add src/device/include/driver/detail/storage.hpp
git commit --signoff -m "feat(device): add detail/storage.hpp inlined from device_framework"
```

---

## Task 8: Copy and adapt `detail/uart_device.hpp`

**Files:**
- Create: `src/device/include/driver/detail/uart_device.hpp`
- Source: `3rd/device_framework/include/device_framework/detail/uart_device.hpp`

**Step 1: Write the adapted file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_UART_DEVICE_HPP_`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- `#include "device_framework/ops/char_device.hpp"` → `#include "driver/ops/char_device.hpp"`
- Namespace: `namespace device_framework::detail {` → `namespace detail {`
- Friend declarations:
  - `friend class ::device_framework::DeviceOperationsBase;` → `friend class ::DeviceOperationsBase;`
  - `friend class ::device_framework::CharDevice;` → `friend class ::CharDevice;`

**Step 2: Verify**

```bash
clang-format -i src/device/include/driver/detail/uart_device.hpp
grep "device_framework" src/device/include/driver/detail/uart_device.hpp
```
Expected: no output.

**Step 3: Commit**

```bash
git add src/device/include/driver/detail/uart_device.hpp
git commit --signoff -m "feat(device): add detail/uart_device.hpp inlined from device_framework"
```

---

## Task 9: Copy and adapt `detail/ns16550a/`

**Files:**
- Create: `src/device/include/driver/detail/ns16550a/ns16550a.hpp`
- Create: `src/device/include/driver/detail/ns16550a/ns16550a_device.hpp`
- Sources: `3rd/device_framework/include/device_framework/detail/ns16550a/`

**Step 1: Adapt `ns16550a.hpp`**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_NS16550A_NS16550A_HPP_`
- `#include "device_framework/detail/mmio_accessor.hpp"` → `#include "driver/detail/mmio_accessor.hpp"`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- Namespace: `namespace device_framework::detail::ns16550a {` → `namespace detail::ns16550a {`

**Step 2: Adapt `ns16550a_device.hpp`**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_NS16550A_NS16550A_DEVICE_HPP_`
- `#include "device_framework/detail/ns16550a/ns16550a.hpp"` → `#include "driver/detail/ns16550a/ns16550a.hpp"`
- `#include "device_framework/detail/uart_device.hpp"` → `#include "driver/detail/uart_device.hpp"`
- Namespace: `namespace device_framework::detail::ns16550a {` → `namespace detail::ns16550a {`

**Step 3: Verify and commit**

```bash
clang-format -i src/device/include/driver/detail/ns16550a/ns16550a.hpp \
               src/device/include/driver/detail/ns16550a/ns16550a_device.hpp
grep -r "device_framework" src/device/include/driver/detail/ns16550a/
git add src/device/include/driver/detail/ns16550a/
git commit --signoff -m "feat(device): add detail/ns16550a/ inlined from device_framework"
```

---

## Task 10: Copy and adapt `detail/pl011/`

**Files:**
- Create: `src/device/include/driver/detail/pl011/pl011.hpp`
- Create: `src/device/include/driver/detail/pl011/pl011_device.hpp`
- Sources: `3rd/device_framework/include/device_framework/detail/pl011/`

**Step 1: Adapt `pl011.hpp`**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_PL011_PL011_HPP_`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- `#include "device_framework/detail/mmio_accessor.hpp"` → `#include "driver/detail/mmio_accessor.hpp"`
- Namespace: `namespace device_framework::detail::pl011 {` → `namespace detail::pl011 {`

**Step 2: Adapt `pl011_device.hpp`**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_PL011_PL011_DEVICE_HPP_`
- `#include "device_framework/detail/pl011/pl011.hpp"` → `#include "driver/detail/pl011/pl011.hpp"`
- `#include "device_framework/detail/uart_device.hpp"` → `#include "driver/detail/uart_device.hpp"`
- Namespace: `namespace device_framework::detail::pl011 {` → `namespace detail::pl011 {`

**Step 3: Verify and commit**

```bash
clang-format -i src/device/include/driver/detail/pl011/pl011.hpp \
               src/device/include/driver/detail/pl011/pl011_device.hpp
grep -r "device_framework" src/device/include/driver/detail/pl011/
git add src/device/include/driver/detail/pl011/
git commit --signoff -m "feat(device): add detail/pl011/ inlined from device_framework"
```

---

## Task 11: Copy and adapt `detail/virtio/` flat files

**Files:**
- Create: `src/device/include/driver/detail/virtio/defs.h`
- Create: `src/device/include/driver/detail/virtio/traits.hpp`
- Sources: `3rd/device_framework/include/device_framework/detail/virtio/`

**Step 1: Adapt `defs.h`**

- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEFS_H_`
- Namespace: `namespace device_framework::detail::virtio {` → `namespace detail::virtio {`
- No includes to change

**Step 2: Adapt `traits.hpp`**

- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_TRAITS_HPP_`
- `#include "device_framework/traits.hpp"` → `#include "driver/traits.hpp"`
- Namespace: `namespace device_framework::detail::virtio {` → `namespace detail::virtio {`
- Change base type in `NullVirtioTraits`:
  `using NullVirtioTraits = device_framework::NullTraits;` → `using NullVirtioTraits = NullTraits;`

**Step 3: Commit**

```bash
clang-format -i src/device/include/driver/detail/virtio/traits.hpp
git add src/device/include/driver/detail/virtio/
git commit --signoff -m "feat(device): add detail/virtio flat headers inlined from device_framework"
```

---

## Task 12: Copy and adapt `detail/virtio/transport/`

**Files:**
- Create: `src/device/include/driver/detail/virtio/transport/transport.hpp`
- Create: `src/device/include/driver/detail/virtio/transport/mmio.hpp`
- Create: `src/device/include/driver/detail/virtio/transport/pci.hpp`
- Sources: `3rd/device_framework/include/device_framework/detail/virtio/transport/`

**Step 1: Adapt each file**

For all three files, apply transformation rules:
- Update header guards with `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_TRANSPORT_*`
- `#include "device_framework/detail/X"` → `#include "driver/detail/X"`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- Remove `#include "device_framework/platform_config.hpp"`
- Namespace: `namespace device_framework::detail::virtio {` → `namespace detail::virtio {`

**Step 2: Verify and commit**

```bash
clang-format -i src/device/include/driver/detail/virtio/transport/*.hpp
grep -r "device_framework" src/device/include/driver/detail/virtio/transport/
git add src/device/include/driver/detail/virtio/transport/
git commit --signoff -m "feat(device): add detail/virtio/transport/ inlined from device_framework"
```

---

## Task 13: Copy and adapt `detail/virtio/virt_queue/`

**Files:**
- Create: `src/device/include/driver/detail/virtio/virt_queue/misc.hpp`
- Create: `src/device/include/driver/detail/virtio/virt_queue/virtqueue_base.hpp`
- Create: `src/device/include/driver/detail/virtio/virt_queue/split.hpp`
- Sources: `3rd/device_framework/include/device_framework/detail/virtio/virt_queue/`

**Step 1: Adapt each file**

Apply transformation rules for all three:
- Update header guards with `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_VIRT_QUEUE_*`
- `#include "device_framework/detail/X"` → `#include "driver/detail/X"`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- Remove `#include "device_framework/platform_config.hpp"`
- Namespace: `namespace device_framework::detail::virtio {` → `namespace detail::virtio {`

**Step 2: Verify and commit**

```bash
clang-format -i src/device/include/driver/detail/virtio/virt_queue/*.hpp
grep -r "device_framework" src/device/include/driver/detail/virtio/virt_queue/
git add src/device/include/driver/detail/virtio/virt_queue/
git commit --signoff -m "feat(device): add detail/virtio/virt_queue/ inlined from device_framework"
```

---

## Task 14: Copy and adapt `detail/virtio/device/`

**Files:**
- Create: `src/device/include/driver/detail/virtio/device/device_initializer.hpp`
- Create: `src/device/include/driver/detail/virtio/device/virtio_blk_defs.h`
- Create: `src/device/include/driver/detail/virtio/device/virtio_blk.hpp`
- Create: `src/device/include/driver/detail/virtio/device/virtio_blk_device.hpp`
- Create: `src/device/include/driver/detail/virtio/device/virtio_console.h`
- Create: `src/device/include/driver/detail/virtio/device/virtio_gpu.h`
- Create: `src/device/include/driver/detail/virtio/device/virtio_input.h`
- Create: `src/device/include/driver/detail/virtio/device/virtio_net.h`
- Sources: `3rd/device_framework/include/device_framework/detail/virtio/device/`

**Step 1: Adapt each file**

Apply transformation rules for all files:
- Update header guards
- `#include "device_framework/detail/X"` → `#include "driver/detail/X"`
- Remove `#include "device_framework/expected.hpp"`, add `#include "expected.hpp"`
- Remove `#include "device_framework/platform_config.hpp"`
- `#include "device_framework/ops/block_device.hpp"` → `#include "driver/ops/block_device_ops.hpp"` (in `virtio_blk_device.hpp`)
- Namespace: `namespace device_framework::detail::virtio::blk {` → `namespace detail::virtio::blk {`
- Namespace: `namespace device_framework::detail::virtio {` → `namespace detail::virtio {`
- In `virtio_blk_device.hpp`: The class inherits from `BlockDevice<Derived>` → change to `BlockDeviceOps<Derived>`

**Step 2: Verify and commit**

```bash
clang-format -i src/device/include/driver/detail/virtio/device/*.hpp \
               src/device/include/driver/detail/virtio/device/*.h 2>/dev/null || true
grep -r "device_framework\|class BlockDevice[^O]" src/device/include/driver/detail/virtio/device/
git add src/device/include/driver/detail/virtio/device/
git commit --signoff -m "feat(device): add detail/virtio/device/ inlined from device_framework"
```

---

## Task 15: Copy and adapt `detail/acpi/`

**Files:**
- Create: `src/device/include/driver/detail/acpi/acpi.hpp`
- Source: `3rd/device_framework/include/device_framework/detail/acpi/acpi.hpp`

**Step 1: Adapt the file**

Apply transformation rules:
- Header guard: `SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_ACPI_ACPI_HPP_`
- Namespace: `namespace device_framework::detail::acpi {` → `namespace detail::acpi {`
- No includes to change

**Step 2: Commit**

```bash
clang-format -i src/device/include/driver/detail/acpi/acpi.hpp
git add src/device/include/driver/detail/acpi/
git commit --signoff -m "feat(device): add detail/acpi/ inlined from device_framework"
```

---

## Task 16: Create new public entry header `pl011_driver.hpp`

**Files:**
- Create: `src/device/include/driver/pl011_driver.hpp`
- Source: pattern from `device_framework/pl011.hpp` + new Pl011Driver wrapper

**Step 1: Write the file**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief PL011 UART public driver entry point
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_

#include "driver/detail/pl011/pl011_device.hpp"

/// Re-export detail::pl011 types at the pl011:: namespace level
namespace pl011 {
using namespace detail::pl011;  // NOLINT(google-build-using-namespace)
}  // namespace pl011

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_PL011_DRIVER_HPP_
```

**Step 2: Commit**

```bash
git add src/device/include/driver/pl011_driver.hpp
git commit --signoff -m "feat(device): add driver/pl011_driver.hpp public entry point"
```

---

## Task 17: Create new public entry header `acpi_driver.hpp`

**Files:**
- Create: `src/device/include/driver/acpi_driver.hpp`

**Step 1: Write the file**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ACPI public driver entry point
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_ACPI_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_ACPI_DRIVER_HPP_

#include "driver/detail/acpi/acpi.hpp"

/// Re-export detail::acpi types at the acpi:: namespace level
namespace acpi {
using namespace detail::acpi;  // NOLINT(google-build-using-namespace)
}  // namespace acpi

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_ACPI_DRIVER_HPP_
```

**Step 2: Commit**

```bash
git add src/device/include/driver/acpi_driver.hpp
git commit --signoff -m "feat(device): add driver/acpi_driver.hpp public entry point"
```

---

## Task 18: Update `src/device/include/driver/ns16550a_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/ns16550a_driver.hpp`

**Step 1: Apply changes**

Replace:
```cpp
#include <device_framework/ns16550a.hpp>
```
with:
```cpp
#include "driver/detail/ns16550a/ns16550a_device.hpp"

/// Re-export detail::ns16550a at ns16550a:: level for callers using
/// ns16550a::Ns16550aDevice directly (e.g. arch interrupt code)
namespace ns16550a {
using namespace detail::ns16550a;  // NOLINT(google-build-using-namespace)
}  // namespace ns16550a
```

Replace:
```cpp
using Ns16550aType = device_framework::ns16550a::Ns16550a;
```
with:
```cpp
using Ns16550aType = detail::ns16550a::Ns16550a;
```

**Step 2: Verify**

```bash
grep "device_framework" src/device/include/driver/ns16550a_driver.hpp
```
Expected: no output.

**Step 3: Commit**

```bash
git add src/device/include/driver/ns16550a_driver.hpp
git commit --signoff -m "refactor(device): update ns16550a_driver.hpp to use inlined headers"
```

---

## Task 19: Update `src/device/include/driver/virtio_blk_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

**Step 1: Replace includes**

Replace:
```cpp
#include <device_framework/detail/storage.hpp>
#include <device_framework/virtio_blk.hpp>
```
with:
```cpp
#include "driver/detail/storage.hpp"
#include "driver/detail/virtio/device/virtio_blk_device.hpp"
#include "driver/detail/virtio/traits.hpp"
#include "driver/detail/virtio/transport/mmio.hpp"

namespace virtio {
using namespace detail::virtio;  // NOLINT(google-build-using-namespace)
}  // namespace virtio
namespace virtio::blk {
using namespace detail::virtio::blk;  // NOLINT(google-build-using-namespace)
}  // namespace virtio::blk
```

**Step 2: Replace all `device_framework::` prefixes**

| Old | New |
|-----|-----|
| `device_framework::virtio::blk::VirtioBlk<>` | `virtio::blk::VirtioBlk<>` |
| `device_framework::virtio::kMmioMagicValue` | `detail::virtio::kMmioMagicValue` |
| `device_framework::virtio::MmioTransport::MmioReg::kDeviceId` | `detail::virtio::MmioTransport::MmioReg::kDeviceId` |
| `device_framework::virtio::blk::BlkFeatureBit::kSegMax` | `virtio::blk::BlkFeatureBit::kSegMax` |
| `device_framework::virtio::blk::BlkFeatureBit::kSizeMax` | `virtio::blk::BlkFeatureBit::kSizeMax` |
| `device_framework::virtio::blk::BlkFeatureBit::kBlkSize` | `virtio::blk::BlkFeatureBit::kBlkSize` |
| `device_framework::virtio::blk::BlkFeatureBit::kFlush` | `virtio::blk::BlkFeatureBit::kFlush` |
| `device_framework::virtio::blk::BlkFeatureBit::kGeometry` | `virtio::blk::BlkFeatureBit::kGeometry` |
| `device_framework::detail::Storage<VirtioBlkType>` | `detail::Storage<VirtioBlkType>` |

**Step 3: Verify**

```bash
grep "device_framework" src/device/include/driver/virtio_blk_driver.hpp
```
Expected: no output.

**Step 4: Commit**

```bash
git add src/device/include/driver/virtio_blk_driver.hpp
git commit --signoff -m "refactor(device): update virtio_blk_driver.hpp to use inlined headers"
```

---

## Task 20: Update arch files (aarch64)

**Files:**
- Modify: `src/arch/aarch64/early_console.cpp`
- Modify: `src/arch/aarch64/include/pl011_singleton.h`
- Modify: `src/arch/aarch64/interrupt_main.cpp`

**Step 1: Update `early_console.cpp`**

Replace:
```cpp
#include <device_framework/pl011.hpp>
```
with:
```cpp
#include "driver/pl011_driver.hpp"
```

Replace:
```cpp
device_framework::pl011::Pl011Device* pl011 = nullptr;
```
with:
```cpp
pl011::Pl011Device* pl011 = nullptr;
```

Replace:
```cpp
pl011->Open(device_framework::OpenFlags(device_framework::OpenFlags::kReadWrite));
```
with:
```cpp
pl011->Open(OpenFlags(OpenFlags::kReadWrite));
```

**Step 2: Update `pl011_singleton.h`**

Replace:
```cpp
#include <device_framework/pl011.hpp>
using Pl011Singleton = etl::singleton<device_framework::pl011::Pl011Device>;
```
with:
```cpp
#include "driver/pl011_driver.hpp"
using Pl011Singleton = etl::singleton<pl011::Pl011Device>;
```

**Step 3: Update `interrupt_main.cpp` (aarch64)**

Replace:
```cpp
#include <device_framework/pl011.hpp>
```
with:
```cpp
#include "driver/pl011_driver.hpp"
```
(No other `device_framework::` references in aarch64 interrupt_main.cpp — it uses `Pl011Singleton` from the singleton header.)

**Step 4: Verify**

```bash
grep "device_framework" src/arch/aarch64/early_console.cpp \
  src/arch/aarch64/include/pl011_singleton.h \
  src/arch/aarch64/interrupt_main.cpp
```
Expected: no output.

**Step 5: Commit**

```bash
git add src/arch/aarch64/early_console.cpp \
        src/arch/aarch64/include/pl011_singleton.h \
        src/arch/aarch64/interrupt_main.cpp
git commit --signoff -m "refactor(aarch64): update pl011 includes to use inlined driver headers"
```

---

## Task 21: Update arch files (riscv64)

**Files:**
- Modify: `src/arch/riscv64/interrupt_main.cpp`

**Step 1: Apply changes**

Replace:
```cpp
#include <device_framework/ns16550a.hpp>
```
with:
```cpp
#include "driver/ns16550a_driver.hpp"
```

Replace:
```cpp
using Ns16550aSingleton =
    etl::singleton<device_framework::ns16550a::Ns16550aDevice>;
```
with:
```cpp
using Ns16550aSingleton = etl::singleton<ns16550a::Ns16550aDevice>;
```

Replace:
```cpp
[](void* /*token*/, device_framework::ErrorCode status) {
  if (status != device_framework::ErrorCode::kSuccess) {
```
with:
```cpp
[](void* /*token*/, ErrorCode status) {
  if (status != ErrorCode::kSuccess) {
```

Replace:
```cpp
auto uart_result = device_framework::ns16550a::Ns16550aDevice::Create(base);
```
with:
```cpp
auto uart_result = ns16550a::Ns16550aDevice::Create(base);
```

Replace:
```cpp
[](device_framework::Error e) -> device_framework::Expected<void> {
  klog::Err(
      "Failed to open device_framework::ns16550a::Ns16550aDevice: %d\n",
      static_cast<int>(e.code));
  return device_framework::Expected<void>{};
```
with:
```cpp
[](Error e) -> Expected<void> {
  klog::Err(
      "Failed to open ns16550a::Ns16550aDevice: %d\n",
      static_cast<int>(e.code));
  return Expected<void>{};
```

**Step 2: Verify**

```bash
grep "device_framework" src/arch/riscv64/interrupt_main.cpp
```
Expected: no output.

**Step 3: Commit**

```bash
git add src/arch/riscv64/interrupt_main.cpp
git commit --signoff -m "refactor(riscv64): update ns16550a includes to use inlined driver headers"
```

---

## Task 22: CMake and gitmodules cleanup

**Files:**
- Modify: `cmake/3rd.cmake` (line 87)
- Modify: `cmake/compile_config.cmake` (line 168)
- Modify: `.gitmodules` (lines 40-42)

**Step 1: Remove from `cmake/3rd.cmake`**

Delete these two lines (around line 86-87):
```cmake
# https://github.com/MRNIU/device_framework.git
ADD_SUBDIRECTORY (3rd/device_framework)
```

**Step 2: Remove from `cmake/compile_config.cmake`**

Delete the `device_framework` entry from the `TARGET_LINK_LIBRARIES` block (line 168).

**Step 3: Remove from `.gitmodules`**

Delete:
```ini
[submodule "3rd/device_framework"]
    path = 3rd/device_framework
    url = https://github.com/MRNIU/device_framework.git
```

**Step 4: Commit**

```bash
git add cmake/3rd.cmake cmake/compile_config.cmake .gitmodules
git commit --signoff -m "chore(cmake): remove device_framework submodule from build system"
```

---

## Task 23: Remove submodule and delete hack bridge

**Step 1: Deinit and remove the submodule**

```bash
git submodule deinit -f 3rd/device_framework
git rm -f 3rd/device_framework
rm -rf .git/modules/3rd/device_framework
```

**Step 2: Delete the hack bridge directory**

```bash
rm -rf src/device/include/device_framework/
git add -A
```

**Step 3: Commit**

```bash
git commit --signoff -m "chore(device): remove device_framework submodule and platform_config.hpp hack"
```

---

## Task 24: Build verification

**Step 1: Configure and build riscv64**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```
Expected: `Linking CXX executable bin/SimpleKernel` with exit code 0.

**Step 2: Configure and build aarch64**

```bash
cmake --preset build_aarch64
cd build_aarch64 && make SimpleKernel 2>&1 | tail -20
```
Expected: exit code 0.

**Step 3: Configure and build x86_64**

```bash
cmake --preset build_x86_64
cd build_x86_64 && make SimpleKernel 2>&1 | tail -20
```
Expected: exit code 0.

**Step 4: Run unit tests (on host)**

```bash
cmake --preset build_x86_64
cd build_x86_64 && make unit-test 2>&1 | tail -20
```
Expected: all tests PASS.

**Step 5: Run clang-format check**

```bash
pre-commit run --all-files
```
Expected: all hooks pass.

**Step 6: Final commit (if pre-commit auto-fixed anything)**

```bash
git add -A
git diff --cached --stat
git commit --signoff -m "style(device): clang-format fixes after device_framework integration" || echo "nothing to commit"
```
