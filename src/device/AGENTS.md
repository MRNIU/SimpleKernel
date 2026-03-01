# AGENTS.md — src/device/

## OVERVIEW
Header-only device framework using C++23 concepts and ETL (Embedded Template Library) idioms. FDT-based device enumeration, bus abstraction, driver registry with delegate-based Probe/Remove lifecycle. Two .cpp files: device.cpp (init entry point) and virtio_driver.cpp (VirtIO device-type dispatch).

## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
traits.hpp              # EnvironmentTraits / BarrierTraits / DmaTraits concepts
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
  device/               # Per-device-type headers (virtio_blk, virtio_net, ...)
  transport/            # MMIO and PCI transport layers
  virt_queue/           # Split/packed virtqueue implementations
include/
  device_manager.hpp    # DeviceManagerSingleton — owns buses, ProbeAll/RemoveAll
  driver_registry.hpp   # DriverRegistry — DriverEntry lookup via etl::flat_map
  platform_bus.hpp      # PlatformBus — FDT-driven device enumeration
  bus.hpp               # Bus ABC — Enumerate/Probe/Remove interface
  device_node.hpp       # DeviceNode — per-device state (compatible, regs, irqs)
  acpi_bus.hpp          # ACPI bus (x86_64 path, placeholder)
  pci_bus.hpp           # PCI bus (placeholder)
  platform_traits.hpp   # Compile-time platform feature detection
  block_device_provider.hpp  # Block device abstraction for filesystem layer
  mmio_helper.hpp       # Safe MMIO region setup helpers
```

## WHERE TO LOOK
- **Adding a driver** → Copy `ns16550a/ns16550a_driver.hpp` pattern: `Probe()`, `Remove()`, `GetEntry()`, `kMatchTable[]`
- **Device init flow** → `device.cpp`: register buses → register drivers → `ProbeAll()`
- **FDT enumeration** → `platform_bus.hpp`: walks device tree, matches `compatible` strings
- **Block devices** → `block_device_provider.hpp` + `virtio_driver.hpp`
- **Adding a new VirtIO device** → Add `virtio/device/virtio_xxx.hpp`, add a `case DeviceId::kXxx` to `virtio/virtio_driver.cpp`, no changes to DeviceInit needed.

## CONVENTIONS
- **Mostly header-only** — `device.cpp` and `virtio_driver.cpp` are the only .cpp files.
  Do NOT create additional .cpp files without strong justification.
- **Compile-time isolation** → `device` is STATIC and PRIVATE-links all `xxx_driver` INTERFACE targets. External code can only access hardware through `DeviceManager`. Arch code that needs direct driver access must explicitly `TARGET_LINK_LIBRARIES(... xxx_driver)`.
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
