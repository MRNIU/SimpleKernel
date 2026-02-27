# AGENTS.md — src/device/

## OVERVIEW
Header-only device framework using C++23 concepts. FDT-based device enumeration, bus abstraction, driver registry with Probe/Remove lifecycle. Single .cpp file (device.cpp) for init entry point.

## STRUCTURE
```
device.cpp              # DeviceInit() — registers buses, drivers, calls ProbeAll()
include/
  device_manager.hpp    # Singleton<DeviceManager> — owns buses, ProbeAll/RemoveAll
  driver_registry.hpp   # Singleton<DriverRegistry> — Register<T>, driver lookup
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
- Uses `Singleton<T>::GetInstance()` for DeviceManager and DriverRegistry
- `df_bridge.hpp` adapts the 3rd-party `device_framework` submodule to kernel types

## ANTI-PATTERNS
- **DO NOT** create .cpp implementation files for device framework headers — they are intentionally header-only
- **DO NOT** call device APIs before DeviceInit() in boot sequence
- **DO NOT** use raw MMIO — go through DeviceNode resource accessors
