# Device Module Simplification — Full Redesign

**Date**: 2026-03-01
**Status**: Approved
**Scope**: `src/device/` (SimpleKernel side only — `3rd/device_framework` submodule preserved)

---

## Background

Two prior refactors have already been completed:

1. **device-refactor** (`2026-03-01-device-refactor-design.md`, Status: Approved & Implemented)
   — Removed `<Traits>` template explosion from `device_framework`, unified error types via
   `platform_config.hpp`, simplified `df_bridge.hpp`.

2. **device-driver-probe-split** (`2026-03-01-device-driver-probe-split-design.md`, Status: Proposed)
   — Proposed adding `Match()` to the `Driver` concept to separate hardware detection from
   initialization. **This design supersedes that proposal.**

The current code (post-refactor-1, pre-probe-split) still carries significant complexity:

| Pain Point | Location | Root Cause |
|------------|----------|------------|
| `template Register<D>()` + `GetDriverInstance<D>()` | `driver_registry.hpp` | Driver concept forced on registry |
| `std::variant<PlatformCompatible, PciMatchKey, AcpiHid>` MatchEntry | `driver_registry.hpp` | Pre-emptive multi-bus support |
| `std::variant<PlatformId, PciAddress, AcpiId>` BusId in DeviceNode | `device_node.hpp` | Same as above |
| `etl::unique_ptr<IoBuffer> dma_buffer` in DeviceNode | `device_node.hpp` | Driver concern leaking into generic struct |
| `SpinLock` + hand-rolled move in DeviceNode | `device_node.hpp` | Artifact of complex ownership model |
| `acpi_bus.hpp`, `pci_bus.hpp` | `include/` | Placeholder files, zero implementation |

---

## Constraints

- `3rd/device_framework` submodule is **preserved** — `Ns16550a` and `VirtioBlk` hardware
  implementations continue to be used as-is.
- No exceptions, RTTI, `dynamic_cast`, or `typeid`.
- No heap allocation before memory subsystem init.
- C++23, freestanding, Google C++ Style.
- `Bus` concept is **preserved** as an extension point for future PCI/ACPI buses.

---

## Chosen Approach: Function-Pointer Registry + Single Bus Concept

Replace the `Driver` concept and template-based registration with a simple
`DriverEntry` struct (function pointers). Keep the `Bus` concept as the sole C++23
concept in the module. Remove all multi-bus variant types from `DeviceNode` and
`MatchEntry`.

---

## File Structure

### Removed (5 files)

| File | Reason |
|------|--------|
| `include/acpi_bus.hpp` | Placeholder, no implementation |
| `include/pci_bus.hpp` | Placeholder, no implementation |
| `include/platform_traits.hpp` | Content inlined into `device_framework/platform_config.hpp` (already there) |
| `include/virtio_messages.hpp` | Not part of device framework; move to `src/include/` if needed |
| `include/df_bridge.hpp` | Renamed to `mmio_helper.hpp` for clarity |

### Renamed (1 file)

| Old | New |
|-----|-----|
| `include/df_bridge.hpp` | `include/mmio_helper.hpp` |

### Retained + Modified (6 files)

`bus.hpp`, `device_node.hpp`, `driver_registry.hpp`, `platform_bus.hpp`,
`device_manager.hpp`, `driver/ns16550a_driver.hpp`, `driver/virtio_blk_driver.hpp`

### Final structure (8 headers + 1 cpp)

```
src/device/
  device.cpp
  include/
    bus.hpp                        # Bus concept (unchanged)
    device_node.hpp                # Simplified: plain struct, no variant, no DMA
    driver_registry.hpp            # New: function-pointer entries, no Driver concept
    platform_bus.hpp               # FDT enumeration (logic unchanged, adapts new DeviceNode)
    device_manager.hpp             # Simplified: remove DMA pre-allocation loop
    mmio_helper.hpp                # Renamed from df_bridge; stripped to PrepareMmioProbe only
    block_device_provider.hpp      # Unchanged
    device_framework/
      platform_config.hpp          # Unchanged (already in place)
    driver/
      ns16550a_driver.hpp          # Simplified: uses static Instance() + GetEntry()
      virtio_blk_driver.hpp        # Simplified: owns DMA buffer, uses static Instance() + GetEntry()
```

---

## Component Designs

### 1. `device_node.hpp` — Plain hardware description

**Goal**: `DeviceNode` is a plain data struct. No lifecycle management, no DMA,
no concurrency primitives.

```cpp
/// Bus type discriminator — extension point for PCI/ACPI
enum class BusType : uint8_t { kPlatform, kPci, kAcpi };

/// Hardware resources for a single device
struct DeviceNode {
  char      name[32]{};
  BusType   bus_type{BusType::kPlatform};
  DeviceType type{DeviceType::kPlatform};

  /// MMIO region (first region; extend to array when multi-BAR is needed)
  uint64_t  mmio_base{0};
  size_t    mmio_size{0};

  /// Interrupt line (first IRQ; extend when multi-IRQ is needed)
  uint32_t  irq{0};

  /// FDT compatible stringlist ('\0'-separated, e.g. "ns16550a\0ns16550\0")
  char      compatible[128]{};
  size_t    compatible_len{0};

  uint32_t  dev_id{0};

  /// Bound flag — set by ProbeAll() under the DeviceManager lock.
  /// ProbeAll() runs single-threaded at boot; no per-node lock needed.
  bool      bound{false};
};
```

**Removed from current `device_node.hpp`**:

| Removed | Moved to |
|---------|----------|
| `BusId = variant<PlatformId, PciAddress, AcpiId>` | `BusType` enum + `compatible[]` covers FDT; PCI/ACPI address fields belong in the Bus implementation when needed |
| `DeviceResource` nested struct with `MmioRegion[6]` + `irq[4]` | Flattened to single `mmio_base/size` + `irq` |
| `etl::unique_ptr<IoBuffer> dma_buffer` | Moves into `VirtioBlkDriver` private state |
| `SpinLock lock_` | Removed; `bound` is protected by `DeviceManager::lock_` |
| `TryBind()` / `Release()` methods | Removed; `DeviceManager::ProbeAll()` sets `bound` directly under its own lock |
| 28-line hand-rolled move constructor | Deleted; struct becomes trivially movable |

**Line count**: 167 → ~40

---

### 2. `driver_registry.hpp` — Function-pointer registry

**Goal**: Registering a driver = passing one struct. No templates, no concept,
no hash index. Linear scan over ≤32 drivers is O(N), perfectly adequate.

```cpp
/// Match entry — one entry per compatible string a driver handles.
/// bus_type enables future PCI/ACPI extensions without changing this struct.
struct MatchEntry {
  BusType     bus_type;
  const char* compatible;  ///< FDT compatible string (Platform)
                           ///< or vendor/HID string (PCI/ACPI — future)
};

/// Immutable driver descriptor — lives in driver's .rodata
struct DriverDescriptor {
  const char*       name;
  const MatchEntry* match_table;
  size_t            match_count;
};

/// Type-erased driver entry — one per registered driver
struct DriverEntry {
  const DriverDescriptor*              descriptor;
  auto (*match)(DeviceNode&)  -> bool;              ///< Hardware detection (may read MMIO)
  auto (*probe)(DeviceNode&)  -> Expected<void>;    ///< Driver initialization
  auto (*remove)(DeviceNode&) -> Expected<void>;    ///< Driver teardown
};

/// Driver registry — a flat array of DriverEntry
class DriverRegistry {
 public:
  /// Register a driver. No templates, no type erasure ceremony.
  auto Register(const DriverEntry& entry) -> Expected<void>;

  /// Find the first driver whose match_table contains a compatible string
  /// present in node.compatible (linear scan — fine for ≤32 drivers).
  auto FindDriver(const DeviceNode& node) -> const DriverEntry*;

  DriverRegistry() = default;
  ~DriverRegistry() = default;
  DriverRegistry(const DriverRegistry&) = delete;
  auto operator=(const DriverRegistry&) -> DriverRegistry& = delete;

 private:
  static constexpr size_t kMaxDrivers = 32;
  DriverEntry drivers_[kMaxDrivers]{};
  size_t      count_{0};
  SpinLock    lock_{"driver_registry"};
};
```

**What changed vs. current `driver_registry.hpp`**:

| Removed | Replaced with |
|---------|--------------|
| `Driver` concept (5 lines) | Nothing — callers supply `DriverEntry` directly |
| `template Register<D>()` | `Register(const DriverEntry&)` |
| `GetDriverInstance<D>()` static singleton | Static `Instance()` inside each driver class |
| `std::variant<PlatformCompatible, PciMatchKey, AcpiHid>` | `MatchEntry {BusType, const char*}` |
| `etl::flat_map<uint32_t, size_t>` FNV index | Removed (O(N) scan is fine for ≤32 drivers) |
| `PlatformCompatible`, `PciMatchKey`, `AcpiHid` structs | Deleted |
| `Fnv1a32()` helper | Deleted |

**Line count**: 198 → ~70

---

### 3. `mmio_helper.hpp` — Minimal MMIO probe helper

**Goal**: Provide the one reusable operation that every MMIO driver needs: extract
the MMIO base address from a `DeviceNode` and map it into the virtual address space.

```cpp
namespace mmio_helper {

struct ProbeContext {
  uint64_t base;
  size_t   size;
};

/// Extract MMIO base/size from node, map region via VirtualMemory.
/// On failure, does NOT modify node.bound — caller handles rollback.
[[nodiscard]] auto Prepare(const DeviceNode& node, size_t default_size)
    -> Expected<ProbeContext>;

}  // namespace mmio_helper
```

**What changed vs. current `df_bridge.hpp`**:

| Removed | Reason |
|---------|--------|
| `ProbeWithMmio<CreateFn>()` template | Drivers call `Prepare()` + their own create logic inline; the template saved 3 lines but obscured the data flow |
| `MmioProbeContext` (renamed) | Renamed to `ProbeContext` in `mmio_helper` namespace |
| `namespace df_bridge` | Renamed to `mmio_helper` |
| `TryBind()` call inside Prepare | Moved to each driver's `Probe()` for clarity |

**Line count**: 90 → ~30

---

### 4. Driver pattern — `static Instance()` + `GetEntry()`

Every driver follows this pattern. No template registration machinery needed.

```cpp
class Ns16550aDriver {
 public:
  // Single driver instance, driver-managed lifetime
  static auto Instance() -> Ns16550aDriver& {
    static Ns16550aDriver inst;
    return inst;
  }

  // Package everything ProbeAll() needs into one struct
  static auto GetEntry() -> const DriverEntry&;

  // Hardware detection: does this device respond as expected?
  static auto MatchStatic(DeviceNode& node) -> bool;

  // Driver initialization
  auto Probe(DeviceNode& node) -> Expected<void>;
  auto Remove(DeviceNode& node) -> Expected<void>;

  [[nodiscard]] auto GetDevice() -> Ns16550aType*;

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "ns16550a"},
      {BusType::kPlatform, "ns16550"},
  };
  static const DriverDescriptor kDescriptor;

  Ns16550aType uart_;
};

// Implementation of GetEntry():
auto Ns16550aDriver::GetEntry() -> const DriverEntry& {
  static const DriverEntry entry{
      .descriptor = &kDescriptor,
      .match  = MatchStatic,
      .probe  = [](DeviceNode& n) { return Instance().Probe(n); },
      .remove = [](DeviceNode& n) { return Instance().Remove(n); },
  };
  return entry;
}
```

For `VirtioBlkDriver`, `Match()` reads the VirtIO magic + device_id registers
(hardware detection separated from initialization, superseding the probe-split
design proposal). `Probe()` owns DMA buffer allocation internally.

---

### 5. `device_manager.hpp` — ProbeAll() with Match() step

```cpp
auto ProbeAll() -> Expected<void> {
  LockGuard guard(lock_);
  size_t probed = 0;
  for (size_t i = 0; i < device_count_; ++i) {
    auto* drv = registry_.FindDriver(devices_[i]);
    if (drv == nullptr) continue;

    // Fine-grained hardware detection (may read MMIO registers)
    if (!drv->match(devices_[i])) continue;

    // Mark bound under DeviceManager lock — no per-node SpinLock needed
    if (devices_[i].bound) continue;
    devices_[i].bound = true;

    auto result = drv->probe(devices_[i]);
    if (!result.has_value()) {
      devices_[i].bound = false;  // rollback
      klog::Err("DeviceManager: probe '%s' failed: %s\n",
                devices_[i].name, result.error().message());
      continue;
    }
    ++probed;
  }
  klog::Info("DeviceManager: probed %zu device(s)\n", probed);
  return {};
}
```

`FindDevicesByType()` is preserved (still useful for callers like filesystem layer).

---

### 6. `device.cpp` — Clean init sequence (~40 lines)

```cpp
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();

  // Register drivers — one line each, no templates
  if (auto r = dm.GetRegistry().Register(Ns16550aDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register Ns16550aDriver failed\n"); return;
  }
  if (auto r = dm.GetRegistry().Register(VirtioBlkDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register VirtioBlkDriver failed\n"); return;
  }

  // Enumerate devices via FDT
  PlatformBus platform_bus(KernelFdtSingleton::instance());
  if (auto r = dm.RegisterBus(platform_bus); !r) {
    klog::Err("DeviceInit: bus enumeration failed\n"); return;
  }

  // Match and probe
  if (auto r = dm.ProbeAll(); !r) {
    klog::Err("DeviceInit: ProbeAll failed\n"); return;
  }

  klog::Info("DeviceInit: complete\n");
}

// VirtioBlkBlockDevice adapter and GetVirtioBlkBlockDevice() remain in device.cpp
```

The 20-line DMA pre-allocation loop (`for virtio,mmio` devices) is removed.
DMA allocation moves into `VirtioBlkDriver::Probe()`.

---

## Data Flow

```
DeviceInit()
  ├── Registry::Register(Ns16550aDriver::GetEntry())   // store function pointers
  ├── Registry::Register(VirtioBlkDriver::GetEntry())
  └── DeviceManager::RegisterBus(PlatformBus)
        └── PlatformBus::Enumerate(fdt)                // fill DeviceNode[]
              └── DeviceManager::ProbeAll()
                    for each DeviceNode:
                      1. FindDriver(node)    — match compatible string O(N)
                      2. drv->match(node)    — hardware detection (may read MMIO)
                      3. node.bound = true   — claim under DeviceManager::lock_
                      4. drv->probe(node)    — driver initialization
```

---

## File Change Summary

| File | Action | Before | After (est.) |
|------|--------|--------|--------------|
| `include/acpi_bus.hpp` | **Delete** | ~30 | 0 |
| `include/pci_bus.hpp` | **Delete** | ~30 | 0 |
| `include/platform_traits.hpp` | **Delete** (already in platform_config.hpp) | 30 | 0 |
| `include/virtio_messages.hpp` | **Move** to `src/include/` | — | — |
| `include/df_bridge.hpp` | **Rename + simplify** → `mmio_helper.hpp` | 90 | ~30 |
| `include/device_node.hpp` | **Rewrite** | 167 | ~40 |
| `include/driver_registry.hpp` | **Rewrite** | 198 | ~70 |
| `include/device_manager.hpp` | **Modify** (ProbeAll + Match step) | 159 | ~100 |
| `include/platform_bus.hpp` | **Modify** (adapt to new DeviceNode fields) | 99 | ~80 |
| `include/driver/ns16550a_driver.hpp` | **Modify** (GetEntry, Match, Instance) | 60 | ~70 |
| `include/driver/virtio_blk_driver.hpp` | **Modify** (Match, DMA ownership, GetEntry) | 143 | ~120 |
| `device.cpp` | **Simplify** (remove DMA loop) | 143 | ~60 |
| **Net** | | **~1149** | **~570** |

**Net reduction: ~580 lines (~50%)**

---

## Relationship to Previous Designs

| Previous design | Status after this redesign |
|----------------|---------------------------|
| device-refactor (Traits removal) | **Fully preserved** — `platform_config.hpp`, unified errors, `detail/Storage<T>` all remain |
| device-driver-probe-split (Match concept) | **Superseded** — `Match()` is added directly to drivers as a static/instance method and referenced by `DriverEntry.match`; no `Driver` concept needed |

---

## Out of Scope

- `3rd/device_framework` submodule — not modified
- `Bus` concept and `PlatformBus::Enumerate()` logic — preserved, only DeviceNode field names change
- `block_device_provider.hpp` and `GetVirtioBlkBlockDevice()` — preserved
- Architecture-specific code (`src/arch/`) — untouched
- PCI/ACPI bus implementations — future work; `BusType` enum provides the extension point

---

## Success Criteria

1. `driver_registry.hpp` contains no C++ concepts or function templates
2. `device_node.hpp` contains no `variant`, `unique_ptr`, `SpinLock`, or `atomic`
3. `DeviceInit()` contains no driver-specific strings or constants
4. `VirtioBlkDriver::Probe()` contains no hardware register reads (only initialization)
5. `acpi_bus.hpp` and `pci_bus.hpp` are deleted
6. Build passes: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`
7. Pre-commit passes: `pre-commit run --all-files`
