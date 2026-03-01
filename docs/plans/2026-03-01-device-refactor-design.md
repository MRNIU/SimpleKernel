# Device Framework Refactor Design

**Date**: 2026-03-01
**Status**: Approved
**Scope**: `3rd/device_framework/` + `src/device/`

## Problem Statement

The current device subsystem has four interrelated complexity sources:

1. **Traits template explosion** — `PlatformTraits` propagates as a template parameter through every layer: `VirtioBlkDriver<Traits>` → `VirtioBlk<Traits>` → `MmioTransport<Traits>` → `SplitVirtqueue<Traits>`. Every intermediate class becomes a template, generating redundant instantiations and obscuring the actual logic.

2. **Dual error systems** — `device_framework` defines its own `ErrorCode` enum; SimpleKernel has a separate `kernel::ErrorCode`. `df_bridge::ToKernelErrorCode()` bridges them — pure noise with no semantic value.

3. **`df_bridge.hpp` glue layer** — `DeviceStorage<T>` is a hand-rolled `optional<T>` (placement new + aligned storage). Combined with `ToKernelErrorCode()` and Traits forwarding, `df_bridge.hpp` exists solely to paper over the impedance between two independently-typed systems.

4. **`DriverRegistry` hand-rolled type erasure** — Custom `DriverEntry` + function pointer arrays + linear scan, reimplementing what `etl::flat_map` provides.

## Constraints

- `device_framework` remains an independent git submodule (reusable across projects)
- `device_framework` stays self-contained (no ETL dependency)
- Ops interface (`Open`/`Release`/`Read`/`Write`/`Mmap`/`Ioctl`) is preserved for future VFS integration
- No exceptions, RTTI, `dynamic_cast`, or `typeid`
- No heap allocation before memory subsystem init
- C++23, freestanding, Google C++ Style

## Chosen Approach: Global Traits Injection via `platform_config.hpp`

`device_framework` defines a **placeholder header** that every internal layer `#include`s to obtain platform-specific types. The using project provides the concrete implementation by placing its version earlier on the include path. All `<Traits>` template parameters are eliminated from internal layers.

## Architecture

### Layer Diagram (After)

```
SimpleKernel
  src/device/include/device_framework/platform_config.hpp  ← concrete impl
  src/device/include/df_bridge.hpp                         ← thin (40 lines)
  src/device/include/driver_registry.hpp                   ← etl::flat_map
  src/device/include/device_node.hpp                       ← no hand-rolled copy/move
  src/device/device.cpp                                    ← DMA in Probe callbacks

device_framework (submodule)
  include/device_framework/platform_config.hpp             ← placeholder (not impl)
  include/device_framework/ops/                            ← preserved (future VFS)
  detail/ns16550a/                                         ← no <Traits> params
  detail/virtio/                                           ← no <Traits> params
  detail/pl011/                                            ← no <Traits> params
  detail/storage.hpp                                       ← internal Storage<T>
```

### CMake Include Priority

```cmake
# src/device/CMakeLists.txt
target_include_directories(SimpleKernel PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include   # platform_config.hpp impl — searched first
)
target_link_libraries(SimpleKernel PRIVATE device_framework)
# device_framework's include dir comes via transitive dependency,
# but src/device/include takes precedence, overriding the placeholder.
```

## Component Designs

### 1. `device_framework/platform_config.hpp` (placeholder)

```cpp
// include/device_framework/platform_config.hpp
// DO NOT implement here. Provided by the using project at build time.
//
// The using project must supply:
//   using PlatformEnvironment = <type satisfying EnvironmentTraits concept>;
//   using PlatformBarrier     = <type satisfying BarrierTraits concept>;
//   using PlatformDma         = <type satisfying DmaTraits concept>;
//   using PlatformErrorCode   = <integer-compatible enum or int>;
```

`device_framework/CMakeLists.txt` documents this requirement but does not provide a default — a missing `platform_config.hpp` is a compile-time error that forces the using project to provide one.

### 2. Internal layers — Traits removed

**Before:**
```cpp
template <typename Traits>
class MmioTransport {
    using Env = typename Traits::EnvironmentType;
    using Dma = typename Traits::DmaType;
};
```

**After:**
```cpp
#include <device_framework/platform_config.hpp>

class MmioTransport {
    using Env = PlatformEnvironment;
    using Dma = PlatformDma;
};
```

Same change applied to `SplitVirtqueue`, `VirtioBlk`, `Ns16550a`, `Pl011` and all `detail/` classes.

### 3. `device_framework/detail/storage.hpp` (moved internal)

`DeviceStorage<T>` migrates from `df_bridge.hpp` into `device_framework/detail/storage.hpp` as a private implementation detail. It is not part of the public API. Named `Storage<T>` internally.

```cpp
// detail/storage.hpp — internal only, not installed as public header
template <typename T>
class Storage {
    alignas(T) uint8_t buf_[sizeof(T)];
    bool initialized_ = false;
public:
    template <typename... Args>
    auto emplace(Args&&... args) -> T&;
    auto reset() -> void;
    auto has_value() const -> bool;
    auto value() -> T&;
};
```

### 4. Error system unification

`platform_config.hpp` (SimpleKernel's version) defines:

```cpp
using PlatformErrorCode = kernel::ErrorCode;
```

`device_framework` replaces its internal `enum class ErrorCode` with `using ErrorCode = PlatformErrorCode`. The `Expected<T>` alias in `device_framework` becomes compatible with SimpleKernel's `Expected<T>` — no conversion needed at the boundary. `ToKernelErrorCode()` is deleted.

### 5. `df_bridge.hpp` (simplified)

Deleted:
- `DeviceStorage<T>` — moved to `device_framework/detail/storage.hpp`
- `ToKernelErrorCode()` — error types are now identical
- Traits-related `using` declarations

Retained:
```cpp
// Extract MMIO base/size from FDT node, validate alignment
auto PrepareMmioProbe(const FdtNode& node) -> Expected<MmioRegion>;

// Call driver Probe, manage device lifecycle binding
template <typename Driver>
auto ProbeWithMmio(DeviceNode& node, const MmioRegion& region) -> Expected<void>;
```

Result: ~120 lines → ~40 lines.

### 6. `DriverRegistry` (etl::flat_map)

```cpp
struct DriverEntry {
    const char*  compatible;
    auto (*probe)(DeviceNode&, uintptr_t base, size_t size) -> Expected<void>;
    auto (*remove)(DeviceNode&) -> void;
};

// Internal storage
etl::flat_map<uint32_t, DriverEntry, kMaxDrivers> drivers_;
// key = FNV-1a hash of compatible string (constexpr, computed at registration)
```

Lookup is O(log N) vs. current O(N×M). `REGISTER_DRIVER(...)` macro interface unchanged.

### 7. `DeviceNode` (simplified)

```cpp
// Before: std::atomic<bool> bound_  — forced 28 lines of hand-written copy/move
// After:  bool bound_               — protected by existing SpinLock lock_

DeviceNode(const DeviceNode&) = delete;
auto operator=(const DeviceNode&) -> DeviceNode& = delete;
DeviceNode(DeviceNode&&) = default;
auto operator=(DeviceNode&&) -> DeviceNode& = default;
```

### 8. DMA allocation (moved into Probe)

`device.cpp` currently pre-allocates DMA buffers before calling `ProbeAll`. Each driver's `Probe` callback is responsible for its own DMA allocation, using `PlatformDma` (available via `platform_config.hpp`). `device.cpp` no longer contains driver-specific DMA setup.

## Data Flow (After)

```
DeviceInit()
  ├── DriverRegistry::Register(ns16550a_driver)    // etl::flat_map insert
  ├── DriverRegistry::Register(virtio_blk_driver)
  └── DeviceManager::ProbeAll(platform_bus)
        └── PlatformBus::Enumerate(fdt)
              └── for each FDT node:
                    DriverRegistry::Find(compatible)       // O(log N)
                    df_bridge::PrepareMmioProbe(node)      // extract base/size
                    df_bridge::ProbeWithMmio(node, region)
                          └── Ns16550a::Probe(base, size)  // concrete class, not template
                                └── PlatformEnvironment / PlatformDma
                                    (from platform_config.hpp, resolved at compile time)
```

## File Change Summary

| File | Action | Delta |
|------|--------|-------|
| `device_framework/include/device_framework/platform_config.hpp` | Add (placeholder) | +20 |
| `device_framework/detail/storage.hpp` | Add (internal) | +50 |
| `device_framework/detail/ns16550a/` | Modify (remove `<Traits>`) | −30 |
| `device_framework/detail/virtio/` | Modify (remove `<Traits>`) | −60 |
| `device_framework/detail/pl011/` | Modify (remove `<Traits>`) | −20 |
| `device_framework/include/device_framework/traits.hpp` | Modify (concepts only, no impls) | −10 |
| `src/device/include/device_framework/platform_config.hpp` | Add (SimpleKernel impl) | +20 |
| `src/device/include/df_bridge.hpp` | Modify | −80 (120→40) |
| `src/device/include/driver_registry.hpp` | Modify | −60 |
| `src/device/include/device_node.hpp` | Modify | −28 |
| `src/device/device.cpp` | Modify (DMA into drivers) | −20 |
| **Net** | | **−218 lines** |

## Out of Scope

- Ops interface (`Open`/`Release`/`Read`/`Write`/`Mmap`/`Ioctl`) — preserved as-is, integrated with VFS separately
- New device drivers — no new drivers added in this refactor
- Scheduler / task subsystem — untouched
- Architecture-specific code (`src/arch/`) — untouched
