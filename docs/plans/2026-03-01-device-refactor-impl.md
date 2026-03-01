# Device Framework Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate Traits template explosion and dual error systems in the device subsystem, saving ~218 lines net.

**Architecture:** `platform_config.hpp` global Traits injection — `device_framework` submodule includes a placeholder header that the using project (SimpleKernel) overrides by placing its concrete version earlier on the CMake include path. All `<Traits>` template parameters are removed from internal device_framework classes; types come from the globally injected `PlatformEnvironment`, `PlatformBarrier`, `PlatformDma`, and `PlatformErrorCode`.

**Tech Stack:** C++23 freestanding, no RTTI/exceptions, ETL (`etl::flat_map`), Google C++ style, header-only device framework.

**Design reference:** `docs/plans/2026-03-01-device-refactor-design.md`

---

## Phase 1 — device_framework submodule changes

> All files live under `3rd/device_framework/include/device_framework/`.
> NS16550A and PL011 use `EnvironmentTraits` only. VirtIO uses `VirtioTraits = Env + Barrier + DMA`.
> **None of these classes currently use `<Traits>` as template parameters** — they are already non-templated (NS16550A, PL011) or use `template <VirtioTraits Traits = NullVirtioTraits>` (VirtIO).
> The goal is to replace **all Traits call sites** (`Traits::Log(...)`, `Traits::Wmb()`, etc.) with the injected types.

### Task 1: Create `platform_config.hpp` placeholder in device_framework

**Files:**
- Create: `3rd/device_framework/include/device_framework/platform_config.hpp`

**Step 1: Write the file**

```cpp
/**
 * @copyright Copyright The device_framework Contributors
 */

#ifndef DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_
#define DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_

// This file is intentionally left as a placeholder.
//
// The using project MUST provide a concrete implementation of this header
// by placing it earlier on the compiler include search path than
// device_framework/include/.
//
// Required declarations:
//
//   // Must satisfy EnvironmentTraits concept (device_framework/traits.hpp)
//   using PlatformEnvironment = <YourEnvType>;
//
//   // Must satisfy BarrierTraits concept
//   using PlatformBarrier = <YourBarrierType>;
//
//   // Must satisfy DmaTraits concept
//   using PlatformDma = <YourDmaType>;
//
//   // Must be integer-compatible (enum class or integral type)
//   using PlatformErrorCode = <YourErrorCodeType>;
//
// These types must be defined in the namespace in which platform_config.hpp
// is included (i.e., at global or device_framework namespace scope as needed).
//
// Failure to provide a concrete implementation results in a compile-time error.
//
// This file intentionally provides no fallback: every project integrating
// device_framework must explicitly supply platform-specific types.

#endif /* DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_ */
```

**Step 2: Verify file exists with correct path**

Check: `ls 3rd/device_framework/include/device_framework/platform_config.hpp`

**Step 3: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/platform_config.hpp
git commit --signoff -m "feat(platform): add platform_config.hpp placeholder for host project injection"
```

---

### Task 2: Create `detail/storage.hpp` in device_framework

Move the "hand-rolled optional" pattern out of SimpleKernel's `df_bridge.hpp` into `device_framework` as an internal utility.

**Files:**
- Create: `3rd/device_framework/include/device_framework/detail/storage.hpp`

**Step 1: Write the file**

```cpp
/**
 * @copyright Copyright The device_framework Contributors
 */

#ifndef DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_DETAIL_STORAGE_HPP_
#define DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_DETAIL_STORAGE_HPP_

#include <cstdint>
#include <utility>

namespace device_framework::detail {

/**
 * @brief Freestanding-compatible placement-new optional storage
 *
 * Provides aligned storage for a single value of type T without requiring
 * heap allocation. Suitable for freestanding/bare-metal environments where
 * std::optional may pull in unwanted dependencies.
 *
 * @tparam T Value type to store
 *
 * @pre  T must be move-constructible
 * @post HasValue() returns true only after a successful Emplace()
 */
template <typename T>
class Storage {
 public:
  Storage() = default;
  ~Storage() { Reset(); }

  Storage(const Storage&) = delete;
  auto operator=(const Storage&) -> Storage& = delete;

  Storage(Storage&& other) noexcept : initialized_(other.initialized_) {
    if (initialized_) {
      new (buf_) T(std::move(*other.Ptr()));
      other.Reset();
    }
  }

  auto operator=(Storage&& other) noexcept -> Storage& {
    if (this != &other) {
      Reset();
      if (other.initialized_) {
        new (buf_) T(std::move(*other.Ptr()));
        initialized_ = true;
        other.Reset();
      }
    }
    return *this;
  }

  /**
   * @brief Construct a T in-place
   *
   * @pre HasValue() == false
   * @post HasValue() == true
   */
  template <typename... Args>
  auto Emplace(Args&&... args) -> T& {
    Reset();
    new (buf_) T(static_cast<Args&&>(args)...);
    initialized_ = true;
    return *Ptr();
  }

  /**
   * @brief Destroy the stored value (if any)
   *
   * @post HasValue() == false
   */
  auto Reset() -> void {
    if (initialized_) {
      Ptr()->~T();
      initialized_ = false;
    }
  }

  /// @return true if a value is currently stored
  [[nodiscard]] auto HasValue() const -> bool { return initialized_; }

  /// @pre HasValue() == true
  [[nodiscard]] auto Value() -> T& { return *Ptr(); }
  [[nodiscard]] auto Value() const -> const T& { return *Ptr(); }

 private:
  [[nodiscard]] auto Ptr() -> T* {
    return reinterpret_cast<T*>(buf_);  // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
  }
  [[nodiscard]] auto Ptr() const -> const T* {
    return reinterpret_cast<const T*>(buf_);  // NOLINT(cppcoreguidelines-pro-type-reinterpret-cast)
  }

  alignas(T) uint8_t buf_[sizeof(T)] = {};
  bool initialized_ = false;
};

}  // namespace device_framework::detail

#endif /* DEVICE_FRAMEWORK_INCLUDE_DEVICE_FRAMEWORK_DETAIL_STORAGE_HPP_ */
```

**Step 2: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/storage.hpp
git commit --signoff -m "feat(detail): add Storage<T> freestanding optional for driver use"
```

---

### Task 3: Update `expected.hpp` — inject `PlatformErrorCode`

**Files:**
- Modify: `3rd/device_framework/include/device_framework/expected.hpp`

Current state (lines 1–201): defines `enum class ErrorCode { ... }` with ~25 values, then `struct Error { ErrorCode code; }`, then `template <typename T> using Expected = std::expected<T, Error>`.

**Step 1: Read the current file**

```bash
cat 3rd/device_framework/include/device_framework/expected.hpp
```

**Step 2: Add `#include "device_framework/platform_config.hpp"` after existing includes**

After the existing `#include` block (before the `namespace device_framework {` line), insert:

```cpp
#include "device_framework/platform_config.hpp"
```

**Step 3: Replace `enum class ErrorCode` with `using ErrorCode = PlatformErrorCode`**

The current block looks like:
```cpp
enum class ErrorCode : int {
  kSuccess = 0,
  kDeviceError = -1,
  ...
};
```

Replace the **entire `enum class ErrorCode` block** with:
```cpp
/// @brief Error code type, provided by platform_config.hpp.
/// The using project must define PlatformErrorCode with at minimum:
///   kSuccess, kDeviceError, kInvalidArgument, kDeviceNotSupported,
///   kDevicePermissionDenied, kDeviceBusy, kTransportNotInitialized,
///   kFeatureNegotiationFailed, kQueueNotAvailable, kQueueTooLarge,
///   kNoFreeDescriptors, kNoUsedBuffers, kInvalidDescriptor
using ErrorCode = PlatformErrorCode;
```

**Step 4: Verify — the rest of `expected.hpp` uses `ErrorCode::kXxx` values; all those call sites must remain unchanged (they will now resolve against `PlatformErrorCode`)**

**Step 5: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/expected.hpp
git commit --signoff -m "refactor(expected): replace ErrorCode enum with PlatformErrorCode from platform_config.hpp"
```

---

### Task 4: Remove `<Traits>` from `Transport` base class

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/transport/transport.hpp`

Current line 84: `template <VirtioTraits Traits = NullVirtioTraits>`

The `Transport<Traits>` base class itself has **no direct Traits call sites** — it only uses `Traits` in its template parameter to pass down to subclasses. The class body (lines 85–152) calls `self.GetStatus()`, `self.GetInterruptStatus()`, etc. via Deducing this, but does **not** call `Traits::Log()` or similar.

**Step 1: Remove the `template <VirtioTraits Traits = NullVirtioTraits>` line from the class**

The class declaration becomes:
```cpp
class Transport {
```

**Step 2: Update the include — add `#include "device_framework/platform_config.hpp"` after `traits.hpp` include**

The include of `traits.hpp` on line 9 stays (it defines `VirtioTraits` concept, still needed for subclasses and `TransportConcept`). Add `platform_config.hpp` include.

**Step 3: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/virtio/transport/transport.hpp
git commit --signoff -m "refactor(transport): remove Traits template parameter from Transport base class"
```

---

### Task 5: Remove `<Traits>` from `MmioTransport` and replace call sites

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/transport/mmio.hpp`

Current state (398 lines):
- Line 59: `template <VirtioTraits Traits = NullVirtioTraits>`
- Line 60: `class MmioTransport final : public Transport<Traits> {`
- `Traits::Log(...)` called at lines ~128, ~141, ~170, ~184, ~205, ~215, ~232, ~250, ~264, ~278, ~296, ~310, ~323, ~337, ~358, ~373 (any line with `Traits::Log`)
- `Traits::Mb()`, `Traits::Rmb()`, `Traits::Wmb()` not called directly in MmioTransport (only in Virtqueue)
- `Traits::VirtToPhys()`, `Traits::PhysToVirt()` not called in MmioTransport

**Step 1: Add `#include "device_framework/platform_config.hpp"` after existing includes (before namespace)**

**Step 2: Replace `template <VirtioTraits Traits = NullVirtioTraits>` with nothing (class becomes non-template)**

Line 59 → delete the `template <...>` line
Line 60 → `class MmioTransport final : public Transport {`

**Step 3: Find and replace all `Traits::Log(` with `PlatformEnvironment::Log(`**

Use grep to confirm exact call sites:
```bash
grep -n "Traits::" 3rd/device_framework/include/device_framework/detail/virtio/transport/mmio.hpp
```

Replace every `Traits::Log(` → `PlatformEnvironment::Log(`

**Step 4: Update all internal uses of `MmioTransport<Traits>` inside the file to just `MmioTransport`**

Check for occurrences of `MmioTransport<Traits>` and `MmioTransport<` and replace.

**Step 5: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/virtio/transport/mmio.hpp
git commit --signoff -m "refactor(mmio): remove Traits template param, use PlatformEnvironment::Log"
```

---

### Task 6: Remove `<Traits>` from `VirtqueueBase` and `SplitVirtqueue`

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/virt_queue/virtqueue_base.hpp`
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/virt_queue/split.hpp`

**`virtqueue_base.hpp` changes:**
- Line 42: `template <VirtioTraits Traits = NullVirtioTraits>` → delete
- Line 43: `class VirtqueueBase {` (already correct after template removed)
- Line 63: `Traits::Wmb();` → `PlatformBarrier::Wmb();`
- Line 67: `Traits::Mb();` → `PlatformBarrier::Mb();`
- Line 89: `Traits::Rmb();` → `PlatformBarrier::Rmb();`
- Add `#include "device_framework/platform_config.hpp"` after existing includes

**`split.hpp` changes:**
- Line 39: `template <VirtioTraits Traits = NullVirtioTraits>` → delete
- Line 40: `class SplitVirtqueue final : public VirtqueueBase<Traits> {` → `class SplitVirtqueue final : public VirtqueueBase {`
- Line 387: `Traits::Wmb();` → `PlatformBarrier::Wmb();`
- Line 510: `Traits::Wmb();` → `PlatformBarrier::Wmb();`
- Move constructor line 644: `VirtqueueBase<Traits>(std::move(other))` → `VirtqueueBase(std::move(other))`
- Add `#include "device_framework/platform_config.hpp"` after existing includes

Verify with grep:
```bash
grep -n "Traits::" 3rd/device_framework/include/device_framework/detail/virtio/virt_queue/virtqueue_base.hpp
grep -n "Traits::" 3rd/device_framework/include/device_framework/detail/virtio/virt_queue/split.hpp
```

**Step: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/virtio/virt_queue/virtqueue_base.hpp \
        include/device_framework/detail/virtio/virt_queue/split.hpp
git commit --signoff -m "refactor(virtqueue): remove Traits template params, use PlatformBarrier"
```

---

### Task 7: Remove `<Traits>` from `DeviceInitializer`

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/device/device_initializer.hpp`

Current state:
- Line 45: `template <VirtioTraits Traits, TransportConcept TransportImpl>`
- Many `Traits::Log(...)` call sites throughout Init(), SetupQueue(), Activate()

**Step 1: Replace `template <VirtioTraits Traits, TransportConcept TransportImpl>` with `template <TransportConcept TransportImpl>`**

**Step 2: Replace all `Traits::Log(` with `PlatformEnvironment::Log(`**

Verify:
```bash
grep -n "Traits::" 3rd/device_framework/include/device_framework/detail/virtio/device/device_initializer.hpp
```

**Step 3: Add `#include "device_framework/platform_config.hpp"` after existing includes**

**Step 4: Commit**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/virtio/device/device_initializer.hpp
git commit --signoff -m "refactor(device_initializer): remove Traits template param, use PlatformEnvironment"
```

---

### Task 8: Remove `<Traits>` from `VirtioBlk` and `VirtioBlkDevice`

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk.hpp`
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk_device.hpp`

**`virtio_blk.hpp` changes** (first 60 lines read; full file needed):

Read the full file first:
```bash
cat 3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk.hpp
```

Expected changes:
- Remove `template <VirtioTraits Traits = NullVirtioTraits, ...>` — keep only `template <template <class> class TransportT = MmioTransport, template <class> class VirtqueueT = SplitVirtqueue>`

  Wait — the template parameter packs are intertwined. Specifically, `TransportT` and `VirtqueueT` are `template <class>` templates that currently take a Traits argument. After removing Traits, `MmioTransport` and `SplitVirtqueue` are non-templates. So the `template <class>` template-template parameters also need to change.

  **Revised approach:** Since `MmioTransport` and `SplitVirtqueue` become non-template classes, `VirtioBlk` and `VirtioBlkDevice` can use them directly:

  ```cpp
  // Before:
  template <VirtioTraits Traits = NullVirtioTraits,
            template <class> class TransportT = MmioTransport,
            template <class> class VirtqueueT = SplitVirtqueue>
  class VirtioBlk {
    using TransportType = TransportT<Traits>;
    using VirtqueueType = VirtqueueT<Traits>;

  // After:
  template <typename TransportT = MmioTransport,
            typename VirtqueueT = SplitVirtqueue>
  class VirtioBlk {
    using TransportType = TransportT;
    using VirtqueueType = VirtqueueT;
  ```

- Replace all `Traits::Log(`, `Traits::VirtToPhys(`, `Traits::PhysToVirt(` with `PlatformEnvironment::Log(`, `PlatformDma::VirtToPhys(`, `PlatformDma::PhysToVirt(`

- Add `#include "device_framework/platform_config.hpp"` after existing includes

**`virtio_blk_device.hpp` changes:**
- Line 42–44:
  ```cpp
  // Before:
  template <VirtioTraits Traits = NullVirtioTraits,
            template <class> class TransportT = MmioTransport,
            template <class> class VirtqueueT = SplitVirtqueue>
  class VirtioBlkDevice
      : public BlockDevice<VirtioBlkDevice<Traits, TransportT, VirtqueueT>> {
  // After:
  template <typename TransportT = MmioTransport,
            typename VirtqueueT = SplitVirtqueue>
  class VirtioBlkDevice
      : public BlockDevice<VirtioBlkDevice<TransportT, VirtqueueT>> {
  ```
- Line 49: `using DriverType = VirtioBlk<Traits, TransportT, VirtqueueT>;` → `using DriverType = VirtioBlk<TransportT, VirtqueueT>;`
- All other references to `VirtioBlkDevice<Traits, TransportT, VirtqueueT>` → `VirtioBlkDevice<TransportT, VirtqueueT>`

**Step: Commit after both files**

```bash
cd 3rd/device_framework
git add include/device_framework/detail/virtio/device/virtio_blk.hpp \
        include/device_framework/detail/virtio/device/virtio_blk_device.hpp
git commit --signoff -m "refactor(virtio_blk): remove Traits template param, use platform_config.hpp types"
```

---

### Task 9: Update `detail/virtio/traits.hpp` — concepts stay, but `NullVirtioTraits` may be simplified

**Files:**
- Modify: `3rd/device_framework/include/device_framework/detail/virtio/traits.hpp`

`VirtioTraits` concept is still useful for testing/standalone use. `NullVirtioTraits` alias can stay.

No structural changes needed — the concepts remain valuable for documentation and unit testing device_framework in isolation. Just verify nothing imports `Traits` directly as a template parameter type.

```bash
grep -rn "VirtioTraits Traits" 3rd/device_framework/include/ | grep -v "concept VirtioTraits"
```

If any remaining `template <VirtioTraits Traits` exist, fix them. Commit if any changes made.

---

### Task 10: Update public entry headers

**Files:**
- Modify: `3rd/device_framework/include/device_framework/virtio_blk.hpp`
- Modify: `3rd/device_framework/include/device_framework/ns16550a.hpp`
- Modify: `3rd/device_framework/include/device_framework/pl011.hpp`

The public entry headers currently just re-export via `using namespace detail::...`. After Traits removal, verify:

1. `virtio_blk.hpp`: the exported `VirtioBlkDevice` no longer needs `<MyTraits>` as first template arg. Update doc comment example from `VirtioBlkDevice<MyTraits>` → `VirtioBlkDevice<>` or `VirtioBlkDevice`.

2. `ns16550a.hpp` and `pl011.hpp`: NS16550A and PL011 were never templated on Traits at the device level. Verify no changes needed.

**Step: Commit any doc-comment updates**

```bash
cd 3rd/device_framework
git add include/device_framework/virtio_blk.hpp \
        include/device_framework/ns16550a.hpp \
        include/device_framework/pl011.hpp
git commit --signoff -m "docs(virtio_blk): update usage example after Traits template removal"
```

---

## Phase 2 — SimpleKernel changes

> All files under `src/device/`.

### Task 11: Create `src/device/include/device_framework/platform_config.hpp` (concrete impl)

This file overrides the device_framework placeholder by being earlier on the include path.

**Files:**
- Create: `src/device/include/device_framework/platform_config.hpp`

**Step 1: Read the existing `platform_traits.hpp` to understand what PlatformTraits provides**

```bash
cat src/device/include/platform_traits.hpp
```

(Already read: `PlatformTraits` satisfies `EnvironmentTraits`, `BarrierTraits`, `DmaTraits`.)

**Step 2: Read `src/include/kernel_error.h` to know the ErrorCode enum values**

```bash
cat src/include/kernel_error.h
```

Confirm that `kernel::ErrorCode` (or `ErrorCode` in the kernel namespace) has at minimum these values (which device_framework uses):
- `kSuccess` / success equivalent
- `kDeviceError`
- `kInvalidArgument`
- `kDeviceNotSupported`
- `kDevicePermissionDenied`
- `kDeviceBusy`
- `kTransportNotInitialized`
- `kFeatureNegotiationFailed`
- `kQueueNotAvailable`
- `kQueueTooLarge`
- `kNoFreeDescriptors`
- `kNoUsedBuffers`
- `kInvalidDescriptor`

**If any values are missing from kernel ErrorCode:** Add them to `src/include/kernel_error.h`.

**Step 3: Write the platform_config.hpp**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

/// @file platform_config.hpp
/// @brief Concrete implementation of device_framework platform_config.hpp for SimpleKernel.
///
/// This file is placed at src/device/include/device_framework/platform_config.hpp
/// and takes priority over 3rd/device_framework/include/device_framework/platform_config.hpp
/// due to include path ordering in src/device/CMakeLists.txt.

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_

#include "device_framework/traits.hpp"
#include "platform_traits.hpp"
#include "src/include/kernel_error.h"   // adjust path as needed

// PlatformTraits satisfies EnvironmentTraits, BarrierTraits, and DmaTraits.
// See src/device/include/platform_traits.hpp for the implementation.
using PlatformEnvironment = PlatformTraits;
using PlatformBarrier     = PlatformTraits;
using PlatformDma         = PlatformTraits;

// Use the kernel's ErrorCode directly. device_framework's ErrorCode becomes
// an alias for this type, eliminating the ToKernelErrorCode() conversion layer.
using PlatformErrorCode = ::ErrorCode;  // kernel root namespace ErrorCode

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_FRAMEWORK_PLATFORM_CONFIG_HPP_ */
```

**Step 4: Commit**

```bash
git add src/device/include/device_framework/platform_config.hpp
git commit --signoff -m "feat(device): add platform_config.hpp concrete impl for device_framework injection"
```

---

### Task 12: Update `src/device/CMakeLists.txt` — include path ordering

**Files:**
- Modify: `src/device/CMakeLists.txt`

Current content (8 lines):
```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (device INTERFACE)

TARGET_INCLUDE_DIRECTORIES (device INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}
                                             include)

TARGET_SOURCES (device INTERFACE device.cpp)
```

The `include` directory is already listed. However, we need to verify that `src/device/include` comes **before** the device_framework include path in the final compile command. The device_framework is linked as a submodule target; verify its `target_include_directories` is `PUBLIC` or `INTERFACE`, and that our `INTERFACE` include comes first.

**Step 1: Check how device_framework is included**

```bash
cat 3rd/device_framework/CMakeLists.txt
```

**Step 2: Ensure ordering**

If device_framework uses `target_include_directories(device_framework INTERFACE ...)`, our `src/device/include` must be listed before `device_framework` in the link order or via `BEFORE` keyword:

```cmake
TARGET_INCLUDE_DIRECTORIES (device BEFORE INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/include)
```

The `BEFORE` keyword ensures our include directory is prepended to the include list, overriding the placeholder.

**Step 3: Commit**

```bash
git add src/device/CMakeLists.txt
git commit --signoff -m "build(device): use BEFORE keyword in TARGET_INCLUDE_DIRECTORIES for platform_config override"
```

---

### Task 13: Simplify `df_bridge.hpp` — remove DeviceStorage and ToKernelError

**Files:**
- Modify: `src/device/include/df_bridge.hpp`

Current content (218 lines): `DeviceStorage<T>` (placement-new optional), `ToKernelErrorCode()`, `ToKernelError()`, `PrepareMmioProbe()`, `ProbeWithMmio()`.

**Step 1: Read the current `df_bridge.hpp` in full**

```bash
cat src/device/include/df_bridge.hpp
```

**Step 2: Delete `DeviceStorage<T>` class** — replaced by `device_framework::detail::Storage<T>`

Update all callers:
- `src/device/include/driver/virtio_blk_driver.hpp` uses `df_bridge::DeviceStorage<VirtioBlkType>` — update to `device_framework::detail::Storage<VirtioBlkType>` (add `#include "device_framework/detail/storage.hpp"`)
- Check for other callers:
  ```bash
  grep -rn "DeviceStorage" src/
  ```

**Step 3: Delete `ToKernelErrorCode()` and `ToKernelError()`** — error types are now identical

Update all callers:
```bash
grep -rn "ToKernelError\|ToKernelErrorCode" src/
```
- `src/device/device.cpp` — remove the call, use the `Expected` result directly
- Any other callers

**Step 4: Keep `PrepareMmioProbe()` and `ProbeWithMmio()` unchanged**

**Step 5: Write the simplified `df_bridge.hpp`** (~40 lines)

Target structure:
```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */
#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_

#include <cstdint>

#include "device_node.hpp"
#include "src/include/kernel_error.h"
#include "src/include/kernel_types.h"

namespace df_bridge {

struct MmioRegion {
  uintptr_t base;
  size_t size;
};

/**
 * @brief Extract MMIO region from FDT node; validate alignment.
 * @return MmioRegion on success, error on invalid node.
 */
[[nodiscard]] auto PrepareMmioProbe(const DeviceNode& node) -> Expected<MmioRegion>;

/**
 * @brief Call driver Probe with MMIO region, bind result to node.
 * @pre node is not already bound
 */
template <typename Driver>
[[nodiscard]] auto ProbeWithMmio(DeviceNode& node, const MmioRegion& region)
    -> Expected<void>;

}  // namespace df_bridge

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DF_BRIDGE_HPP_ */
```

(Keep the actual implementations of `PrepareMmioProbe` and `ProbeWithMmio` from the original — just preserve those bodies verbatim.)

**Step 6: Commit**

```bash
git add src/device/include/df_bridge.hpp
git commit --signoff -m "refactor(df_bridge): remove DeviceStorage and ToKernelError, error types now unified"
```

---

### Task 14: Simplify `device_node.hpp` — replace `std::atomic<bool>` with `bool`

**Files:**
- Modify: `src/device/include/device_node.hpp`

Current state (168 lines): `std::atomic<bool> bound{false};` forces 28 lines of hand-written copy/move constructors.

**Step 1: Read `device_node.hpp` in full**

Confirm:
- `bound` is used only under `SpinLock lock_` in `TryBind()` (or equivalent) — if so, the atomic is redundant
- The copy/move constructors exist solely because `std::atomic` is non-copyable

**Step 2: Replace `std::atomic<bool> bound{false};` with `bool bound_{false};`**

**Step 3: Remove the hand-written copy constructor, copy assignment, move constructor, move assignment**

Replace with:
```cpp
DeviceNode(const DeviceNode&) = delete;
auto operator=(const DeviceNode&) -> DeviceNode& = delete;
DeviceNode(DeviceNode&&) noexcept = default;
auto operator=(DeviceNode&&) noexcept -> DeviceNode& = default;
```

**Step 4: Update `TryBind()` — replace atomic CAS with SpinLock-guarded bool**

```cpp
// Before (with atomic):
auto TryBind() -> bool {
  bool expected = false;
  return bound.compare_exchange_strong(expected, true);
}

// After (with SpinLock):
auto TryBind() -> bool {
  LockGuard<SpinLock> guard(lock_);
  if (bound_) return false;
  bound_ = true;
  return true;
}
```

**Step 5: Remove `#include <atomic>` if it is no longer used**

```bash
grep -n "atomic" src/device/include/device_node.hpp
```

**Step 6: Commit**

```bash
git add src/device/include/device_node.hpp
git commit --signoff -m "refactor(device_node): replace atomic<bool> with SpinLock-guarded bool, remove hand-rolled copy/move"
```

---

### Task 15: Refactor `driver_registry.hpp` — use `etl::flat_map`

**Files:**
- Modify: `src/device/include/driver_registry.hpp`

Current state (166 lines): `DriverEntry entries_[32]` + `size_t count_` + `SpinLock lock_` + O(N×M) linear scan.

**Step 1: Read the full file**

```bash
cat src/device/include/driver_registry.hpp
```

**Step 2: Understand current `DriverEntry` structure**

Likely:
```cpp
struct DriverEntry {
  const char* compatible;
  auto (*probe)(...) -> Expected<void>;
  auto (*remove)(...) -> void;
};
```

**Step 3: Add FNV-1a hash helper** (constexpr, for use as map key)

```cpp
/// @brief FNV-1a 32-bit hash of a null-terminated string (constexpr)
[[nodiscard]] constexpr auto Fnv1a32(const char* str) -> uint32_t {
  uint32_t hash = 2166136261u;
  for (; *str != '\0'; ++str) {
    hash ^= static_cast<uint8_t>(*str);
    hash *= 16777619u;
  }
  return hash;
}
```

**Step 4: Replace `DriverEntry entries_[32]` + count with `etl::flat_map`**

```cpp
#include "etl/flat_map.h"

static constexpr size_t kMaxDrivers = 32;

// key = FNV-1a hash of compatible string
etl::flat_map<uint32_t, DriverEntry, kMaxDrivers> drivers_;
```

**Step 5: Update `Register<T>()` to use `drivers_.insert({hash, entry})`**

**Step 6: Update `Find(const char* compatible)` to use `drivers_.find(hash)`**

O(log N) binary search instead of O(N×M) linear scan.

**Step 7: Keep the SpinLock for thread safety**

**Step 8: Commit**

```bash
git add src/device/include/driver_registry.hpp
git commit --signoff -m "refactor(driver_registry): replace hand-rolled array with etl::flat_map for O(log N) lookup"
```

---

### Task 16: Update `virtio_blk_driver.hpp` — remove `<Traits>` template parameter

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

Current state (140 lines):
- `template <typename Traits> class VirtioBlkDriver` — template on Traits
- Uses `df_bridge::DeviceStorage<VirtioBlkType>` — replace with `device_framework::detail::Storage<VirtioBlkType>`
- Uses `df_bridge::ToKernelError()` — delete, use result directly
- `VirtioBlkType = device_framework::virtio::blk::VirtioBlkDevice<Traits>` — becomes `device_framework::virtio::blk::VirtioBlkDevice<>`

**Step 1: Read the full file**

```bash
cat src/device/include/driver/virtio_blk_driver.hpp
```

**Step 2: Remove the `template <typename Traits>` wrapper**

The class becomes:
```cpp
class VirtioBlkDriver {
 public:
  using VirtioBlkType = device_framework::virtio::blk::VirtioBlkDevice<>;
  // ...
};
```

**Step 3: Replace `df_bridge::DeviceStorage<VirtioBlkType>` with `device_framework::detail::Storage<VirtioBlkType>`**

Add `#include "device_framework/detail/storage.hpp"`

**Step 4: Delete all `df_bridge::ToKernelError(...)` wrapping calls**

The `Expected<void>` from device_framework and kernel are now the same type — use results directly.

**Step 5: Update `GetDescriptor()` static function**

The returned `DriverDescriptor` should register this driver with its match table. Ensure `Probe` and `Remove` function pointers still have the correct signature.

**Step 6: Commit**

```bash
git add src/device/include/driver/virtio_blk_driver.hpp
git commit --signoff -m "refactor(virtio_blk_driver): remove Traits template param, use unified error and Storage types"
```

---

### Task 17: Update `device.cpp` — remove PlatformTraits usage, move DMA into Probe

**Files:**
- Modify: `src/device/device.cpp`

Current state (144 lines):
- DMA buffer pre-allocated at `DeviceInit()` level before `ProbeAll()`
- `VirtioBlkDriver<PlatformTraits>` instantiation
- `df_bridge::ToKernelError()` calls in `VirtioBlkBlockDevice::ReadSectors/WriteSectors`

**Step 1: Read the full file**

```bash
cat src/device/device.cpp
```

**Step 2: Remove `<PlatformTraits>` from `VirtioBlkDriver<PlatformTraits>`**

It is now `VirtioBlkDriver` (non-template).

**Step 3: Move DMA allocation from `DeviceInit()` into `VirtioBlkDriver::Probe()`**

In `VirtioBlkDriver::Probe()`:
- Calculate DMA size: `auto dma_size = VirtioBlkType::CalcDmaSize(queue_size);`
- Allocate using `PlatformDma::AllocDma(dma_size)` or the kernel allocator (whichever the original code uses — check exact allocator API)
- Pass `dma_buf` to `VirtioBlkDevice::Create(mmio_base, dma_buf)`

**Step 4: Remove `df_bridge::ToKernelError()` calls**

These calls in `ReadSectors`/`WriteSectors` (or equivalent) can be replaced with direct result propagation since error types are unified.

**Step 5: Verify no remaining references to `PlatformTraits` as template argument**

```bash
grep -n "PlatformTraits" src/device/device.cpp
```

**Step 6: Commit**

```bash
git add src/device/device.cpp
git commit --signoff -m "refactor(device): remove PlatformTraits template usage, move DMA alloc into VirtioBlkDriver::Probe"
```

---

### Task 18: Verify `platform_traits.hpp` is still needed or can be removed

**Files:**
- Read: `src/device/include/platform_traits.hpp`

`platform_traits.hpp` defines the `PlatformTraits` struct. After this refactor, it is used **indirectly** via `platform_config.hpp` (which does `using PlatformEnvironment = PlatformTraits`). The file itself stays — it is the concrete type definition. No changes needed.

```bash
grep -rn "platform_traits.hpp" src/
```

Verify the only consumer is `src/device/include/device_framework/platform_config.hpp`. If old consumers remain, update them.

---

## Phase 3 — Verification

### Task 19: Build and verify

**Step 1: Configure build**

```bash
cmake --preset build_riscv64
```

Expected: No errors.

**Step 2: Build kernel**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tee /tmp/build.log
```

Expected: Zero errors, zero new warnings.

**Step 3: Check for remaining Traits template usages**

```bash
grep -rn "template.*VirtioTraits\|<Traits>" \
    3rd/device_framework/include/ \
    src/device/include/ \
    src/device/device.cpp \
  | grep -v "concept VirtioTraits\|VirtioTraits concept\|satisfies VirtioTraits"
```

Expected: Zero results (only the concept definition remains).

**Step 4: Check for remaining ToKernelError usages**

```bash
grep -rn "ToKernelError\|ToKernelErrorCode" src/
```

Expected: Zero results.

**Step 5: Check for remaining DeviceStorage usages**

```bash
grep -rn "DeviceStorage\|df_bridge::Storage" src/
```

Expected: Zero results.

**Step 6: Run unit tests (if host arch matches)**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make unit-test
```

Expected: All tests pass (or pre-existing failures only).

**Step 7: Run pre-commit format check**

```bash
pre-commit run --all-files
```

Expected: All checks pass.

---

### Task 20: Final line count verification

**Step 1: Count net delta**

```bash
# Count lines in changed files before and after
wc -l \
    3rd/device_framework/include/device_framework/expected.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/transport/transport.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/transport/mmio.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/virt_queue/virtqueue_base.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/virt_queue/split.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/device/device_initializer.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk.hpp \
    3rd/device_framework/include/device_framework/detail/virtio/device/virtio_blk_device.hpp \
    src/device/include/df_bridge.hpp \
    src/device/include/driver_registry.hpp \
    src/device/include/device_node.hpp \
    src/device/include/driver/virtio_blk_driver.hpp \
    src/device/device.cpp
```

Expected: Net ~−218 lines vs. pre-refactor state (as documented in design doc).

---

## Summary

| Phase | Tasks | Files |
|-------|-------|-------|
| Phase 1: device_framework | 1–10 | `platform_config.hpp`, `detail/storage.hpp`, `expected.hpp`, `transport.hpp`, `mmio.hpp`, `virtqueue_base.hpp`, `split.hpp`, `device_initializer.hpp`, `virtio_blk.hpp`, `virtio_blk_device.hpp` |
| Phase 2: SimpleKernel | 11–18 | `platform_config.hpp` (impl), `CMakeLists.txt`, `df_bridge.hpp`, `device_node.hpp`, `driver_registry.hpp`, `virtio_blk_driver.hpp`, `device.cpp` |
| Phase 3: Verify | 19–20 | Build + grep verification |

**Net result:** −218 lines, zero template explosion, unified error types, O(log N) driver lookup.
