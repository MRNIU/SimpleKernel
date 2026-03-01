# AGENTS.md — src/device/

## OVERVIEW
Header-only device framework using C++23 concepts and ETL (Embedded Template Library) idioms. FDT-based device enumeration, bus abstraction, driver registry with delegate-based Probe/Remove lifecycle. Single .cpp file (device.cpp) for init entry point.

## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
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
    virtio_blk_driver.hpp    # VirtIO block device driver
```

## WHERE TO LOOK
- **Adding a driver** → Copy `ns16550a_driver.hpp` pattern: `Probe()`, `Remove()`, `GetEntry()`, `kMatchTable[]`
- **Device init flow** → `device.cpp`: register buses → register drivers → `ProbeAll()`
- **FDT enumeration** → `platform_bus.hpp`: walks device tree, matches `compatible` strings
- **Block devices** → `block_device_provider.hpp` + `virtio_blk_driver.hpp`

## CONVENTIONS
- **Entirely header-only** except `device.cpp` — no separate .cpp for framework classes
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
- **DO NOT** create .cpp implementation files for device framework headers — they are intentionally header-only
- **DO NOT** call device APIs before DeviceInit() in boot sequence
- **DO NOT** use raw MMIO — go through DeviceNode resource accessors
- **DO NOT** include `<string>` or use `std::string`/`etl::string_view` in freestanding code


## OVERVIEW
Header-only device framework using C++23 concepts. FDT-based device enumeration, bus abstraction, driver registry with Probe/Remove lifecycle. Single .cpp file (device.cpp) for init entry point.

## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
include/
  device_manager.hpp    # DeviceManagerSingleton (etl::singleton<DeviceManager>) — owns buses, ProbeAll/RemoveAll
  driver_registry.hpp   # DriverRegistry — Register<T>, driver lookup
  platform_bus.hpp      # PlatformBus — FDT-driven device enumeration
  bus.hpp               # Bus ABC — Enumerate/Probe/Remove interface
  device_node.hpp       # DeviceNode — per-device state (compatible, regs, irqs)
  df_bridge.hpp         # Bridge between device_framework 3rd-party lib and kernel
  acpi_bus.hpp          # ACPI bus (x86_64 path, placeholder)
  pci_bus.hpp           # PCI bus (placeholder)
  platform_traits.hpp   # Compile-time platform feature detection
  block_device_provider.hpp  # Block device abstraction for filesystem layer
  driver/
    ns16550a_driver.hpp      # NS16550A UART — reference driver implementation
    virtio_blk_driver.hpp    # VirtIO block device driver
```

## WHERE TO LOOK
- **Adding a driver** → Copy `ns16550a_driver.hpp` pattern: `Probe()`, `Remove()`, `GetDescriptor()`, `kMatchTable[]`
- **Device init flow** → `device.cpp`: register buses → register drivers → `ProbeAll()`
- **FDT enumeration** → `platform_bus.hpp`: walks device tree, matches `compatible` strings
- **Block devices** → `block_device_provider.hpp` + `virtio_blk_driver.hpp`

## CONVENTIONS
- **Entirely header-only** except `device.cpp` — no separate .cpp for framework classes
- Drivers define static `kMatchTable[]` with FDT `compatible` strings for matching
- `GetDescriptor()` returns `DriverDescriptor` with name, match table, Probe/Remove function pointers
- Uses named `etl::singleton<T>` aliases (e.g. `DeviceManagerSingleton::instance()`) defined in `kernel.h`
- `df_bridge.hpp` adapts the 3rd-party `device_framework` submodule to kernel types

## ANTI-PATTERNS
- **DO NOT** create .cpp implementation files for device framework headers — they are intentionally header-only
- **DO NOT** call device APIs before DeviceInit() in boot sequence
- **DO NOT** use raw MMIO — go through DeviceNode resource accessors
