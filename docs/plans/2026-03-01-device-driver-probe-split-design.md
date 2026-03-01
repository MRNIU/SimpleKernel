# Device Framework: Separate Driver Logic from Hardware Detection

**Date**: 2026-03-01
**Status**: Proposed
**Scope**: `src/device/` — driver concept, device manager, drivers, device.cpp

---

## Problem Statement

The current device subsystem mixes **hardware detection** and **driver initialization** in three places:

### 1. `VirtioBlkDriver::Probe()` does two unrelated things

```cpp
auto Probe(DeviceNode& node) -> Expected<void> {
  // Phase 1: hardware detection — should this be "my" device?
  etl::io_port_ro<uint32_t> magic_reg{...};
  if (magic_reg.read() != kMmioMagicValue) return error;  // read virtio magic
  if (device_id_reg.read() != kBlockDeviceId) return error;  // read device sub-type

  // Phase 2: driver initialization — actually set up the device
  auto result = VirtioBlkType::Create(base, dma_buf, kDefaultQueueCount, ...);
  device_.Emplace(std::move(*result));
}
```

Hardware detection (reading registers to confirm device identity) and driver initialization
(allocating virtqueues, setting feature bits) are completely different concerns with different
failure modes and different semantics, yet they live in the same function.

### 2. `DeviceInit()` contains driver-specific DMA logic

```cpp
// device.cpp — generic init code, but hardcodes virtio internals
for (size_t i = 0; i < n; ++i) {
  const auto& plat = std::get<PlatformId>(devs[i]->resource.id);
  if (strcmp(plat.compatible, "virtio,mmio") == 0) {           // driver-specific string
    devs[i]->dma_buffer = kstd::make_unique<IoBuffer>(
        VirtioBlkDriver::kMinDmaBufferSize);                    // driver-specific constant
  }
}
// DeviceInit() must run this BEFORE ProbeAll() — temporal coupling
dm.ProbeAll();
```

This loop is VirtioBlkDriver's internal resource requirement leaking into the generic
initialization flow. Any new driver requiring pre-Probe resources would need another hardcoded
block here.

### 3. `DeviceNode::dma_buffer` is a driver-specific concern

```cpp
struct DeviceNode {
  char name[32]{};
  DeviceType type{};
  DeviceResource resource{};
  uint32_t dev_id{0};
  etl::unique_ptr<IoBuffer> dma_buffer{};  // only VirtioBlkDriver uses this
};
```

All device nodes carry a DMA buffer field, even though only VirtIO devices need one. This
is a leaky abstraction — the generic node knows about driver internals.

---

## Constraints

- No exceptions, RTTI, `dynamic_cast`, or `typeid`
- No heap allocation before memory subsystem init (DMA allocation happens during ProbeAll,
  which is called after MemoryInit)
- C++23, freestanding, Google C++ Style
- `device_framework` submodule is NOT modified in this refactor (only `src/device/`)
- Existing `Bus` concept and `PlatformBus::Enumerate()` are NOT modified
- Existing `DriverDescriptor` / `kMatchTable[]` coarse-matching mechanism is PRESERVED

---

## Chosen Approach: `Driver::Match()` for Fine-Grained Hardware Detection

Add a `Match(DeviceNode&) -> bool` method to the `Driver` concept. The framework calls
`Match()` BEFORE `Probe()`. Drivers that need register reads to confirm device identity
(e.g., VirtIO MMIO — one compatible string covers multiple device sub-types) do so in
`Match()`. `Probe()` becomes pure initialization.

This mirrors the Linux driver model where `id_table`/`compatible[]` is the coarse match
and `probe()` is initialization.

---

## Architecture

### Phase Flow (After)

```
DeviceInit()
  ├── Register<Ns16550aDriver>()
  ├── Register<VirtioBlkDriver>()
  └── RegisterBus(PlatformBus)
        └── ProbeAll()
              for each DeviceNode:
                1. FindDriver(resource)  — coarse compat string match (O(log N))
                2. driver.Match(node)    — fine hardware detection (NEW)
                   ↳ for VirtIO: read magic + device_id registers
                   ↳ for NS16550A: return true (compat string is already unique)
                3. TryBind(node)         — atomic claim
                4. driver.Probe(node)    — pure driver initialization
```

### Conceptual Separation

| Concern | Location | Question answered |
|---------|----------|-------------------|
| Coarse device discovery | `Bus::Enumerate()` | What addresses/IRQs exist? |
| Coarse driver matching | `DriverRegistry::FindDriver()` | Which driver *might* handle this? |
| **Fine hardware detection** | **`Driver::Match()`** (NEW) | Is this *specifically* my device? |
| Driver initialization | `Driver::Probe()` | Set up driver state and hardware |

---

## Component Designs

### 1. `driver_registry.hpp` — extend Driver concept + DriverEntry

**Add `Match()` to Driver concept:**

```cpp
template <typename D>
concept Driver = requires(D d, DeviceNode& node) {
  { D::GetDescriptor() } -> std::same_as<const DriverDescriptor&>;
  { d.Match(node) }  -> std::same_as<bool>;           // NEW: hardware detection
  { d.Probe(node) }  -> std::same_as<Expected<void>>;
  { d.Remove(node) } -> std::same_as<Expected<void>>;
};
```

**Add `match` function pointer to DriverEntry:**

```cpp
struct DriverEntry {
  const DriverDescriptor* descriptor;
  auto (*match)(DeviceNode&) -> bool;              // NEW
  auto (*probe)(DeviceNode&) -> Expected<void>;
  auto (*remove)(DeviceNode&) -> Expected<void>;
};
```

**Update `Register<D>()`** to bind `Match()`:

```cpp
drivers_[idx] = DriverEntry{
    .descriptor = &D::GetDescriptor(),
    .match  = [](DeviceNode& n) -> bool         { return GetDriverInstance<D>().Match(n); },
    .probe  = [](DeviceNode& n) -> Expected<void>{ return GetDriverInstance<D>().Probe(n); },
    .remove = [](DeviceNode& n) -> Expected<void>{ return GetDriverInstance<D>().Remove(n); },
};
```

### 2. `device_manager.hpp` — ProbeAll() calls Match() before Probe()

```cpp
auto ProbeAll() -> Expected<void> {
  LockGuard guard(lock_);
  size_t probed = 0;
  for (size_t i = 0; i < device_count_; ++i) {
    auto* drv = registry_.FindDriver(devices_[i].resource);  // coarse match
    if (drv == nullptr) continue;

    // NEW: fine hardware detection — driver reads registers to confirm identity
    if (!drv->match(devices_[i])) {
      klog::Debug("DeviceManager: '%s' hardware detection rejected by '%s'\n",
                  devices_[i].name, drv->descriptor->name);
      continue;
    }

    if (!devices_[i].TryBind()) continue;  // atomic claim

    auto result = drv->probe(devices_[i]);
    if (!result.has_value()) {
      klog::Err("DeviceManager: probe '%s' failed: %s\n",
                devices_[i].name, result.error().message());
      devices_[i].Release();
      continue;
    }
    ++probed;
  }
  klog::Info("DeviceManager: probed %zu device(s)\n", probed);
  return {};
}
```

### 3. `driver/virtio_blk_driver.hpp` — split Probe() into Match() + Probe()

**Before (current):**
```
Probe() = hardware detection + driver init + DMA allocation check
DeviceInit() = DMA allocation (pre-Probe, hardcoded "virtio,mmio")
```

**After:**
```
Match() = hardware detection (magic + device_id register reads)
Probe() = DMA allocation (self-managed) + driver init (virtqueue setup)
```

```cpp
// Hardware detection — only reads registers, does NOT modify driver state
auto Match(DeviceNode& node) -> bool {
  uint64_t base = node.resource.mmio[0].base;
  if (base == 0) return false;
  etl::io_port_ro<uint32_t> magic{reinterpret_cast<void*>(base)};
  if (magic.read() != device_framework::virtio::kMmioMagicValue) return false;
  etl::io_port_ro<uint32_t> dev_id{reinterpret_cast<void*>(
      base + device_framework::virtio::MmioTransport::MmioReg::kDeviceId)};
  return dev_id.read() == kBlockDeviceId;
}

// Driver initialization — Match() already confirmed device identity
auto Probe(DeviceNode& node) -> Expected<void> {
  auto ctx = df_bridge::PrepareMmioProbe(node, kMmioRegionSize);
  if (!ctx) return std::unexpected(ctx.error());

  // DMA allocation is now driver-owned (moved from device.cpp)
  auto dma_buf = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
  if (!dma_buf || !dma_buf->IsValid()) {
    node.Release();
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  auto result = VirtioBlkType::Create(
      ctx->base, dma_buf->GetBuffer().data(),
      kDefaultQueueCount, kDefaultQueueSize, extra_features_);
  if (!result.has_value()) { node.Release(); return ...; }

  device_.Emplace(std::move(*result));
  dma_buf_ = std::move(dma_buf);  // driver owns the DMA buffer lifetime
  // ...
}

private:
  device_framework::detail::Storage<VirtioBlkType> device_;
  kstd::unique_ptr<IoBuffer> dma_buf_;   // NEW: driver-owned DMA buffer
  uint32_t irq_{0};
```

### 4. `driver/ns16550a_driver.hpp` — trivial Match()

```cpp
// Compatible string "ns16550a"/"ns16550" is already unique — no register reads needed
auto Match([[maybe_unused]] DeviceNode& node) -> bool { return true; }
```

### 5. `device_node.hpp` — remove `dma_buffer`

```cpp
struct DeviceNode {
  char name[32]{};
  DeviceType type{};
  DeviceResource resource{};
  uint32_t dev_id{0};
  // dma_buffer REMOVED — DMA is now driver-internal state
 private:
  bool bound_{false};
  SpinLock lock_{"device_node"};
};
```

Remove includes: `etl/memory.h`, `io_buffer.hpp`, `kstd_memory`.

### 6. `device.cpp` — delete driver-specific DMA loop

**Before:**
```cpp
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  // ... register drivers, register bus ...

  // DRIVER-SPECIFIC: hardcoded virtio DMA pre-allocation
  DeviceNode* devs[64]; size_t n = dm.FindDevicesByType(...);
  for (size_t i = 0; i < n; ++i) {
    if (strcmp(plat.compatible, "virtio,mmio") == 0) {
      devs[i]->dma_buffer = kstd::make_unique<IoBuffer>(...);
    }
  }

  dm.ProbeAll();
}
```

**After:**
```cpp
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();
  auto& fdt = KernelFdtSingleton::instance();

  dm.GetRegistry().Register<Ns16550aDriver>();
  dm.GetRegistry().Register<VirtioBlkDriver>();

  PlatformBus platform_bus(fdt);
  dm.RegisterBus(platform_bus);

  dm.ProbeAll();  // Match() + Probe() chained by framework

  klog::Info("DeviceInit: complete\n");
}
```

Remove includes: `kstd_memory`, `block_device_provider.hpp` (if unused by init).
`FindDevicesByType()` no longer called at init time.

---

## Design Decision: Where Does MMIO Mapping Happen?

`df_bridge::PrepareMmioProbe()` currently calls `VirtualMemorySingleton::instance().MapMMIO()`.
This is called from `Probe()`, AFTER `Match()` confirms the device. This ordering is correct:

```
Match() — reads MMIO registers (physical address, no virtual mapping needed yet)
Probe() → PrepareMmioProbe() → MapMMIO() — establishes virtual mapping for driver use
```

This means `Match()` MUST work with the physical MMIO address directly (before MapMMIO).
For `VirtioBlkDriver::Match()`, the magic register read uses the physical address — this is
valid because the kernel's early MMIO setup maps identity or the FDT addresses directly in
the physical window. This matches the existing `Probe()` behavior before the refactor.

---

## File Change Summary

| File | Action | Delta (est.) |
|------|--------|--------------|
| `src/device/include/driver_registry.hpp` | Modify: add `match` to Driver concept + DriverEntry | +12 |
| `src/device/include/device_manager.hpp` | Modify: ProbeAll() calls `drv->match()` | +8 |
| `src/device/include/device_node.hpp` | Modify: remove `dma_buffer`, remove 3 includes | −10 |
| `src/device/include/driver/virtio_blk_driver.hpp` | Modify: add `Match()`, Probe() takes DMA ownership | +10 (net after DMA moves in) |
| `src/device/include/driver/ns16550a_driver.hpp` | Modify: add `Match() { return true; }` | +4 |
| `src/device/device.cpp` | Modify: remove DMA loop, remove FindDevicesByType call | −25 |
| **Net** | | **~−11 lines, structural clarity gain** |

---

## Out of Scope

- `device_framework` submodule — not modified
- `Bus` concept and `PlatformBus` — not modified
- `df_bridge.hpp` structure — `PrepareMmioProbe` / `ProbeWithMmio` retained as-is
- `DriverDescriptor::kMatchTable[]` coarse matching — retained as-is
- PCI bus, ACPI bus — placeholder files not modified
- Block device provider (`GetVirtioBlkBlockDevice`) — preserved in `device.cpp`

---

## Success Criteria

1. `DeviceInit()` contains no driver-specific strings or constants
2. `VirtioBlkDriver::Probe()` contains no hardware register reads (only initialization)
3. `DeviceNode` has no `dma_buffer` field
4. Build passes: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`
5. `grep -rn "virtio,mmio" src/device/device.cpp` → zero results
