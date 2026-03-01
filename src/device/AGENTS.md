# AGENTS.md — src/device/

## OVERVIEW
Header-only device framework using C++23 concepts and ETL (Embedded Template Library) idioms. FDT-based device enumeration, bus abstraction, driver registry with delegate-based Probe/Remove lifecycle. Two .cpp files: device.cpp (init entry point) and virtio_driver.cpp (VirtIO device-type dispatch).

## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
virtio_driver.cpp       # VirtIO device-id dispatch (the only other .cpp besides device.cpp)
include/
  device_manager.hpp    # DeviceManagerSingleton (etl::singleton<DeviceManager>) — owns buses, ProbeAll/RemoveAll
  driver_registry.hpp   # DriverRegistry — DriverEntry lookup via etl::flat_map
  platform_bus.hpp      # PlatformBus — FDT-driven device enumeration
  bus.hpp               # Bus ABC — Enumerate/Probe/Remove interface
  device_node.hpp       # DeviceNode — per-device state (compatible, regs, irqs)
  acpi_bus.hpp          # ACPI bus (x86_64 path, placeholder)
  pci_bus.hpp           # PCI bus (placeholder)
  platform_traits.hpp   # Compile-time platform feature detection
  block_device_provider.hpp  # Block device abstraction for filesystem layer
  driver/
    ns16550a_driver.hpp      # NS16550A UART — reference driver implementation
    virtio_driver.hpp        # Unified VirtIO driver — auto-detects device type at runtime
```

## WHERE TO LOOK
- **Adding a driver** → Copy `ns16550a_driver.hpp` pattern: `Probe()`, `Remove()`, `GetEntry()`, `kMatchTable[]`
- **Device init flow** → `device.cpp`: register buses → register drivers → `ProbeAll()`
- **FDT enumeration** → `platform_bus.hpp`: walks device tree, matches `compatible` strings
- **Block devices** → `block_device_provider.hpp` + `virtio_driver.hpp`
- **Adding a new VirtIO device** → Add `detail/virtio/device/virtio_xxx.hpp`, add a `case DeviceId::kXxx` to `virtio_driver.cpp`, no changes to DeviceInit needed.

## CONVENTIONS
- **Mostly header-only** — `device.cpp` and `virtio_driver.cpp` are the only .cpp files.
  Do NOT create additional .cpp files without strong justification.
- Drivers define static `kMatchTable[]` using `MatchEntry` struct with FDT `compatible` strings
- `GetEntry()` returns `DriverEntry` containing:
  - `name`: `const char*` driver name
  - `match_table`: `etl::span<const MatchEntry>`
  - `match`: `etl::delegate<bool(DeviceNode&)>`
  - `probe/remove`: `etl::delegate<Expected<void>(DeviceNode&)>`
- Uses named `etl::singleton<T>` aliases (e.g. `DeviceManagerSingleton::instance()`) defined in `kernel.h`
- `etl::string_view` is **FREESTANDING-UNSAFE** (transitively pulls in `<string>`) — use `const char*` with `CStrLess` comparator for strings in headers.

## DISCOVERIES / KEY PATTERNS
- **ETL Delegates**: Built using `etl::delegate<Signature>::create<&Class::Method>(instance)` or `::create<&Class::StaticMethod>()`.
- **Match Priority**: `DriverRegistry` uses `etl::flat_map` for O(log N) lookup; first registered driver for a `compatible` string wins.
- **MMIO Access**: Always use `DeviceNode` resource accessors or `mmio_helper::Prepare` for safe MMIO region setup.

## ANTI-PATTERNS
- **DO NOT** create .cpp implementation files for device framework headers — only `device.cpp` and `virtio_driver.cpp` are allowed .cpp files
- **DO NOT** call device APIs before DeviceInit() in boot sequence
- **DO NOT** use raw MMIO — go through DeviceNode resource accessors
- **DO NOT** include `<string>` or use `std::string`/`etl::string_view` in freestanding code
