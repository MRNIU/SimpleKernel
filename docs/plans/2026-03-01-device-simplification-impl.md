# Device Module Simplification — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Simplify `src/device/` from ~1149 lines across 13 headers to ~570 lines across 8 headers — removing the `Driver` concept, template registration, variant-typed IDs, DMA leakage into `DeviceNode`, and all placeholder files.

**Architecture:** Replace `template Register<D>()` + `Driver` concept with a plain `DriverEntry` struct (function pointers). Flatten `DeviceNode` to a pure data struct with scalar fields (no variant, no `unique_ptr`, no `SpinLock`). `bound` is protected by `DeviceManager::lock_`, which is held for the entire `ProbeAll()` loop. `Bus` concept is preserved unchanged.

**Tech Stack:** C++23, freestanding (no STL dynamic allocation), Google C++ Style, `Expected<T>` error returns, `etl::singleton<T>`, `SpinLock`/`LockGuard`.

---

## Reference Files (read these before editing)

- Design doc: `docs/plans/2026-03-01-device-simplification-design.md`
- Build command: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`
- Format check: `pre-commit run --all-files`
- Header guard format: `#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_FILENAME_HPP_`
- Copyright line: `/** @copyright Copyright The SimpleKernel Contributors */`

## Task Order (respects header dependencies)

```
device_node.hpp       ← no deps on other device headers
driver_registry.hpp   ← depends on device_node.hpp
mmio_helper.hpp       ← depends on device_node.hpp (new file, replaces df_bridge.hpp)
ns16550a_driver.hpp   ← depends on driver_registry.hpp + mmio_helper.hpp
virtio_blk_driver.hpp ← depends on driver_registry.hpp + mmio_helper.hpp
platform_bus.hpp      ← depends on device_node.hpp
device_manager.hpp    ← depends on driver_registry.hpp + device_node.hpp + bus.hpp
device.cpp            ← depends on all headers
CMakeLists.txt        ← may need df_bridge.hpp → mmio_helper.hpp header rename
```

---

## Task 1: Delete placeholder + redundant files

**Files:**
- Delete: `src/device/include/acpi_bus.hpp`
- Delete: `src/device/include/pci_bus.hpp`
- Delete: `src/device/include/platform_traits.hpp`
- Delete: `src/device/include/virtio_messages.hpp`

**Step 1: Verify these files have no callers**

```bash
grep -r "acpi_bus\|pci_bus\|platform_traits\|virtio_messages" src/ tests/ --include="*.hpp" --include="*.cpp" --include="*.h"
```

Expected output: no matches (these are stubs or unused).

**Step 2: Delete all four files**

```bash
rm src/device/include/acpi_bus.hpp
rm src/device/include/pci_bus.hpp
rm src/device/include/platform_traits.hpp
rm src/device/include/virtio_messages.hpp
```

**Step 3: Verify build still succeeds**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

Expected: no errors related to these files.

**Step 4: Commit**

```bash
git add -A src/device/include/acpi_bus.hpp src/device/include/pci_bus.hpp \
           src/device/include/platform_traits.hpp src/device/include/virtio_messages.hpp
git commit --signoff -m "refactor(device): delete placeholder and redundant header files"
```

---

## Task 2: Rewrite `device_node.hpp`

**Files:**
- Modify: `src/device/include/device_node.hpp`

**Goal:** Replace the current 167-line file (with `variant<BusId>`, `DeviceResource`, `etl::unique_ptr<IoBuffer>`, `SpinLock`, hand-rolled move ctor) with a ~40-line plain data struct.

**Step 1: Write the new `device_node.hpp`**

Replace the entire file with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Device node — per-device hardware description (plain data struct)
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_

#include <cstddef>
#include <cstdint>

/// Bus type discriminator — extension point for future PCI/ACPI buses
enum class BusType : uint8_t { kPlatform, kPci, kAcpi };

/// Device category
enum class DeviceType : uint8_t {
  kChar,      ///< Character device (serial, etc.)
  kBlock,     ///< Block device (disk, etc.)
  kNet,       ///< Network device
  kPlatform,  ///< Platform device (interrupt controller, timer, etc.)
};

/**
 * @brief Hardware resource description for a single device.
 *
 * Plain data struct — no lifecycle management, no DMA buffers,
 * no concurrency primitives. `bound` is protected by
 * DeviceManager::lock_ (held for the entire ProbeAll() loop).
 */
struct DeviceNode {
  /// Human-readable device name (from FDT node name)
  char name[32]{};

  BusType bus_type{BusType::kPlatform};
  DeviceType type{DeviceType::kPlatform};

  /// First MMIO region (extend to array when multi-BAR support is needed)
  uint64_t mmio_base{0};
  size_t mmio_size{0};

  /// First interrupt line (extend when multi-IRQ support is needed)
  uint32_t irq{0};

  /// FDT compatible stringlist ('\0'-separated, e.g. "ns16550a\0ns16550\0")
  char compatible[128]{};
  size_t compatible_len{0};

  /// Global device ID assigned by DeviceManager
  uint32_t dev_id{0};

  /// Set by ProbeAll() under DeviceManager::lock_ — no per-node lock needed.
  bool bound{false};
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_
```

**Step 2: Attempt build (expected to fail — dependent files need updating)**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Note the errors — they tell you which files still reference old fields (`resource`, `dma_buffer`, `TryBind()`, `Release()`, `BusId`, `PlatformId`, etc.). **Do not fix them yet** — Tasks 3–7 handle each file in order.

---

## Task 3: Rewrite `driver_registry.hpp`

**Files:**
- Modify: `src/device/include/driver_registry.hpp`

**Goal:** Replace 198 lines (Driver concept, `template Register<D>()`, `std::variant<...> MatchEntry`, FNV hash index) with ~70 lines (plain `DriverEntry` struct, `Register(const DriverEntry&)`, O(N) scan).

**Step 1: Write the new `driver_registry.hpp`**

Replace the entire file with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Driver registry — function-pointer entries, no Driver concept
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_

#include <cstddef>
#include <cstdint>

#include "device_node.hpp"
#include "expected.hpp"
#include "kstd_cstring"
#include "spinlock.hpp"

/// One entry in a driver's static match table
struct MatchEntry {
  BusType bus_type;
  const char* compatible;  ///< FDT compatible string (Platform)
                           ///< or vendor/HID string (PCI/ACPI — future)
};

/// Immutable driver descriptor — lives in driver's .rodata
struct DriverDescriptor {
  const char* name;
  const MatchEntry* match_table;
  size_t match_count;
};

/**
 * @brief Type-erased driver entry — one per registered driver.
 *
 * @pre  descriptor points to a static DriverDescriptor
 * @pre  match/probe/remove are non-null function pointers
 */
struct DriverEntry {
  const DriverDescriptor* descriptor;
  auto (*match)(DeviceNode&) -> bool;           ///< Hardware detection
  auto (*probe)(DeviceNode&) -> Expected<void>; ///< Driver initialization
  auto (*remove)(DeviceNode&) -> Expected<void>;///< Driver teardown
};

/**
 * @brief Driver registry — flat array of DriverEntry, O(N) lookup.
 *
 * Linear scan over ≤32 drivers is adequate for boot-time use.
 */
class DriverRegistry {
 public:
  /**
   * @brief Register a driver entry.
   *
   * @pre  entry.descriptor != nullptr
   * @return Expected<void> kOutOfMemory if registry is full
   */
  auto Register(const DriverEntry& entry) -> Expected<void>;

  /**
   * @brief Find the first driver whose match_table contains a compatible
   *        string present in node.compatible (linear scan).
   *
   * @return pointer to DriverEntry, or nullptr if no match
   */
  auto FindDriver(const DeviceNode& node) -> const DriverEntry*;

  DriverRegistry() = default;
  ~DriverRegistry() = default;
  DriverRegistry(const DriverRegistry&) = delete;
  DriverRegistry(DriverRegistry&&) = delete;
  auto operator=(const DriverRegistry&) -> DriverRegistry& = delete;
  auto operator=(DriverRegistry&&) -> DriverRegistry& = delete;

 private:
  static constexpr size_t kMaxDrivers = 32;
  DriverEntry drivers_[kMaxDrivers]{};
  size_t count_{0};
  SpinLock lock_{"driver_registry"};
};

// --- Inline implementations (header-only per module convention) ---

inline auto DriverRegistry::Register(const DriverEntry& entry)
    -> Expected<void> {
  LockGuard guard(lock_);
  if (count_ >= kMaxDrivers) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }
  drivers_[count_++] = entry;
  return {};
}

inline auto DriverRegistry::FindDriver(const DeviceNode& node)
    -> const DriverEntry* {
  // Walk the node's compatible stringlist; for each string, scan all drivers.
  const char* p = node.compatible;
  const char* end = node.compatible + node.compatible_len;
  while (p < end) {
    for (size_t i = 0; i < count_; ++i) {
      const auto* desc = drivers_[i].descriptor;
      for (size_t j = 0; j < desc->match_count; ++j) {
        if (desc->match_table[j].bus_type == node.bus_type &&
            kstd::strcmp(desc->match_table[j].compatible, p) == 0) {
          return &drivers_[i];
        }
      }
    }
    p += kstd::strlen(p) + 1;
  }
  return nullptr;
}

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
```

**Step 2: Attempt build (still expected to fail — drivers + device.cpp need updating)**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Errors will be in `ns16550a_driver.hpp`, `virtio_blk_driver.hpp`, `device_manager.hpp`, and `device.cpp`.

---

## Task 4: Create `mmio_helper.hpp` (replace `df_bridge.hpp`)

**Files:**
- Create: `src/device/include/mmio_helper.hpp`
- Keep: `src/device/include/df_bridge.hpp` (deleted in Task 8 after all callers migrated)

**Goal:** Provide the one reusable MMIO operation every MMIO driver needs: extract `mmio_base`/`mmio_size` from a `DeviceNode` and map via `VirtualMemory`. The new API uses the flat `DeviceNode` fields directly.

**Step 1: Create `src/device/include/mmio_helper.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Minimal MMIO probe helper — maps a DeviceNode's MMIO region
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_

#include "device_node.hpp"
#include "expected.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

namespace mmio_helper {

/// MMIO region after mapping
struct ProbeContext {
  uint64_t base;
  size_t size;
};

/**
 * @brief Extract MMIO base/size from node and map region via VirtualMemory.
 *
 * Does NOT set node.bound — caller (driver Probe()) is responsible for
 * setting node.bound = true under DeviceManager::lock_.
 *
 * @param  node         Device node (must have mmio_base != 0)
 * @param  default_size Fallback size when node.mmio_size == 0
 * @return Expected<ProbeContext>; kDeviceNotFound if mmio_base == 0
 */
[[nodiscard]] inline auto Prepare(const DeviceNode& node, size_t default_size)
    -> Expected<ProbeContext> {
  if (node.mmio_base == 0) {
    klog::Err("mmio_helper: no MMIO base for '%s'\n", node.name);
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  size_t size = node.mmio_size > 0 ? node.mmio_size : default_size;
  VirtualMemorySingleton::instance().MapMMIO(node.mmio_base, size);

  return ProbeContext{node.mmio_base, size};
}

}  // namespace mmio_helper

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_MMIO_HELPER_HPP_
```

**Step 2: No build attempt needed yet** — this file is not included by anything until Tasks 5–6 update the drivers.

---

## Task 5: Update `ns16550a_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/ns16550a_driver.hpp`

**Goal:** Replace `GetDescriptor()` + implicit `Driver` concept conformance with `GetEntry()` + `MatchStatic()` + `Instance()`. Switch from `df_bridge.hpp` to `mmio_helper.hpp`. Use flat `DeviceNode` fields.

**Step 1: Write the new `ns16550a_driver.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief NS16550A UART driver
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_

#include <device_framework/ns16550a.hpp>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "mmio_helper.hpp"

class Ns16550aDriver {
 public:
  using Ns16550aType = device_framework::ns16550a::Ns16550a;

  // --- Registration API ---

  /// Return the singleton driver instance.
  static auto Instance() -> Ns16550aDriver& {
    static Ns16550aDriver inst;
    return inst;
  }

  /// Return the DriverEntry for registration.
  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .descriptor = &kDescriptor,
        .match = MatchStatic,
        .probe = [](DeviceNode& n) { return Instance().Probe(n); },
        .remove = [](DeviceNode& n) { return Instance().Remove(n); },
    };
    return entry;
  }

  // --- Driver lifecycle ---

  /**
   * @brief Hardware detection: does the MMIO region respond as NS16550A?
   *
   * For NS16550A this is a compatible-string-only match — no MMIO reads
   * needed (the device has no readable signature register).
   *
   * @return true if node.compatible contains "ns16550a" or "ns16550"
   */
  static auto MatchStatic([[maybe_unused]] DeviceNode& node) -> bool {
    // Compatible match was already done by FindDriver(); return true to confirm.
    return true;
  }

  /**
   * @brief Initialize the NS16550A UART.
   *
   * @pre  node.mmio_base != 0
   * @post uart_ is valid; node.type == DeviceType::kChar
   */
  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = mmio_helper::Prepare(node, 0x100);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    auto result = Ns16550aType::Create(ctx->base);
    if (!result) {
      return std::unexpected(Error(result.error().code));
    }

    uart_ = std::move(*result);
    node.type = DeviceType::kChar;
    klog::Info("Ns16550aDriver: UART at 0x%lX bound\n", node.mmio_base);
    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    return {};
  }

  [[nodiscard]] auto GetDevice() -> Ns16550aType* { return &uart_; }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "ns16550a"},
      {BusType::kPlatform, "ns16550"},
  };
  static const DriverDescriptor kDescriptor;

  Ns16550aType uart_;
};

inline const DriverDescriptor Ns16550aDriver::kDescriptor{
    .name = "ns16550a",
    .match_table = kMatchTable,
    .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_NS16550A_DRIVER_HPP_
```

**Step 2: Attempt incremental build**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Remaining errors should be in `virtio_blk_driver.hpp`, `device_manager.hpp`, `device.cpp`.

---

## Task 6: Update `virtio_blk_driver.hpp`

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

**Goal:** Add `Instance()` + `GetEntry()` + `MatchStatic()`. Move DMA buffer allocation into `Probe()` (it was pre-allocated in `device.cpp`'s DMA loop). Remove `dma_buffer` access on `DeviceNode` (old field is gone). Switch to flat `DeviceNode` fields (`node.mmio_base`, `node.irq`).

**Step 1: Write the new `virtio_blk_driver.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief VirtIO block device driver
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_

#include <etl/io_port.h>

#include <device_framework/detail/storage.hpp>
#include <device_framework/virtio_blk.hpp>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "kstd_memory"
#include "mmio_helper.hpp"

class VirtioBlkDriver {
 public:
  using VirtioBlkType = device_framework::virtio::blk::VirtioBlk<>;

  static constexpr uint32_t kMmioRegionSize = 0x1000;
  static constexpr uint32_t kDefaultQueueCount = 1;
  static constexpr uint32_t kDefaultQueueSize = 128;
  static constexpr size_t kMinDmaBufferSize = 32768;

  // --- Registration API ---

  static auto Instance() -> VirtioBlkDriver& {
    static VirtioBlkDriver inst;
    return inst;
  }

  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .descriptor = &kDescriptor,
        .match = MatchStatic,
        .probe = [](DeviceNode& n) { return Instance().Probe(n); },
        .remove = [](DeviceNode& n) { return Instance().Remove(n); },
    };
    return entry;
  }

  // --- Driver lifecycle ---

  /**
   * @brief Hardware detection: read VirtIO magic + device_id registers.
   *
   * Maps MMIO temporarily to inspect the VirtIO header. Returns true only
   * if the device is a VirtIO block device (magic == 0x74726976, id == 2).
   *
   * @pre  node.mmio_base != 0 (already validated by FindDriver path)
   */
  static auto MatchStatic(DeviceNode& node) -> bool {
    if (node.mmio_base == 0) return false;

    // Map the MMIO region so we can read registers
    auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
    if (!ctx) return false;

    etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(ctx->base)};
    if (magic_reg.read() != device_framework::virtio::kMmioMagicValue) {
      klog::Debug("VirtioBlkDriver: 0x%lX not a VirtIO device\n", ctx->base);
      return false;
    }

    constexpr uint32_t kBlockDeviceId = 2;
    etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<void*>(
        ctx->base +
        device_framework::virtio::MmioTransport::MmioReg::kDeviceId)};
    if (device_id_reg.read() != kBlockDeviceId) {
      klog::Debug("VirtioBlkDriver: 0x%lX is not a block device\n", ctx->base);
      return false;
    }

    return true;
  }

  /**
   * @brief Initialize VirtIO block device.
   *
   * Allocates DMA buffer internally (was previously pre-allocated in
   * device.cpp's DMA loop). Maps MMIO, negotiates features, creates device.
   *
   * @pre  node.mmio_base != 0
   * @post device_ is valid; node.type == DeviceType::kBlock
   */
  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    // Allocate DMA buffer (driver-owned, not stored in DeviceNode)
    dma_buffer_ = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
    if (!dma_buffer_ || !dma_buffer_->IsValid()) {
      klog::Err("VirtioBlkDriver: failed to allocate DMA buffer at 0x%lX\n",
                ctx->base);
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }
    if (dma_buffer_->GetBuffer().size() < kMinDmaBufferSize) {
      klog::Err("VirtioBlkDriver: DMA buffer too small (%zu < %u)\n",
                dma_buffer_->GetBuffer().size(), kMinDmaBufferSize);
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
    }

    uint64_t extra_features =
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSegMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSizeMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kBlkSize) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kFlush) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kGeometry);

    auto result = VirtioBlkType::Create(
        ctx->base, dma_buffer_->GetBuffer().data(), kDefaultQueueCount,
        kDefaultQueueSize, extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", ctx->base);
      return std::unexpected(Error(result.error().code));
    }

    device_.Emplace(std::move(*result));
    node.type = DeviceType::kBlock;
    irq_ = node.irq;

    klog::Info(
        "VirtioBlkDriver: block device at 0x%lX, capacity=%lu sectors, "
        "irq=%u\n",
        ctx->base, device_.Value().GetCapacity(), irq_);

    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    device_.Reset();
    dma_buffer_.reset();
    return {};
  }

  [[nodiscard]] auto GetDevice() -> VirtioBlkType* {
    return device_.HasValue() ? &device_.Value() : nullptr;
  }

  [[nodiscard]] auto GetIrq() const -> uint32_t { return irq_; }

  template <typename CompletionCallback>
  auto HandleInterrupt(CompletionCallback&& on_complete) -> void {
    if (device_.HasValue()) {
      device_.Value().HandleInterrupt(
          static_cast<CompletionCallback&&>(on_complete));
    }
  }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "virtio,mmio"},
  };
  static const DriverDescriptor kDescriptor;

  device_framework::detail::Storage<VirtioBlkType> device_;
  kstd::unique_ptr<IoBuffer> dma_buffer_;
  uint32_t irq_{0};
};

inline const DriverDescriptor VirtioBlkDriver::kDescriptor{
    .name = "virtio-blk",
    .match_table = kMatchTable,
    .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
```

**Step 2: Attempt incremental build**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Remaining errors should be in `platform_bus.hpp`, `device_manager.hpp`, `device.cpp`.

---

## Task 7: Update `platform_bus.hpp`

**Files:**
- Modify: `src/device/include/platform_bus.hpp`

**Goal:** Adapt `Enumerate()` to write the new flat `DeviceNode` fields (`mmio_base`, `mmio_size`, `irq`, `compatible`, `compatible_len`, `bus_type`) instead of the old nested `resource` struct.

**Step 1: Write the new `platform_bus.hpp`**

Replace the entire file with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Platform bus — FDT-driven device discovery
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "expected.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"

/// Platform bus — enumerates devices from the Flattened Device Tree (FDT)
class PlatformBus {
 public:
  explicit PlatformBus(KernelFdt& fdt) : fdt_(fdt) {}

  static auto GetName() -> const char* { return "platform"; }

  /**
   * @brief Enumerate all FDT device nodes with a compatible string.
   *
   * @param  out  Output array of DeviceNode
   * @param  max  Maximum number of nodes to write
   * @return Expected<size_t> number of nodes written
   */
  auto Enumerate(DeviceNode* out, size_t max) -> Expected<size_t> {
    size_t count = 0;

    auto result = fdt_.ForEachNode(
        [&out, &count, max](const char* node_name,
                            const char* compatible_data,
                            size_t compatible_len, uint64_t mmio_base,
                            size_t mmio_size, uint32_t irq) -> bool {
          if (count >= max) return false;
          if (compatible_data == nullptr || compatible_len == 0) return true;

          auto& node = out[count];

          kstd::strncpy(node.name, node_name, sizeof(node.name) - 1);
          node.name[sizeof(node.name) - 1] = '\0';

          node.bus_type = BusType::kPlatform;
          node.type = DeviceType::kPlatform;
          node.mmio_base = mmio_base;
          node.mmio_size = mmio_size;
          node.irq = irq;

          size_t copy_len = compatible_len < sizeof(node.compatible)
                                ? compatible_len
                                : sizeof(node.compatible);
          if (compatible_len > sizeof(node.compatible)) {
            klog::Warn(
                "PlatformBus: compatible truncated %zu→%zu for '%s'\n",
                compatible_len, sizeof(node.compatible), node_name);
          }
          kstd::memcpy(node.compatible, compatible_data, copy_len);
          node.compatible_len = copy_len;

          klog::Debug(
              "PlatformBus: found '%s' compatible='%s' "
              "mmio=0x%lX size=0x%lX irq=%u\n",
              node_name, compatible_data, mmio_base, mmio_size, irq);

          ++count;
          return true;
        });

    if (!result.has_value()) {
      return std::unexpected(result.error());
    }
    return count;
  }

  PlatformBus() = delete;
  ~PlatformBus() = default;
  PlatformBus(const PlatformBus&) = delete;
  PlatformBus(PlatformBus&&) = delete;
  auto operator=(const PlatformBus&) -> PlatformBus& = delete;
  auto operator=(PlatformBus&&) -> PlatformBus& = delete;

 private:
  KernelFdt& fdt_;
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_PLATFORM_BUS_HPP_
```

**Step 2: Attempt incremental build**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Remaining errors should be in `device_manager.hpp` and `device.cpp`.

---

## Task 8: Update `device_manager.hpp`

**Files:**
- Modify: `src/device/include/device_manager.hpp`

**Goal:** Update `ProbeAll()` to:
1. Call `drv->match(node)` for fine-grained hardware detection (new step)
2. Check+set `node.bound` directly (instead of `node.TryBind()`)
3. Use `registry_.FindDriver(node)` with the new `DeviceNode`-taking signature

**Step 1: Write the new `device_manager.hpp`**

Replace the entire file with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Device manager — owns device nodes and driver registry
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_

#include <etl/singleton.h>

#include "bus.hpp"
#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "kstd_cstring"
#include "spinlock.hpp"

/**
 * @brief Device manager — manages all device nodes and the driver registry.
 *
 * Single-threaded boot path: RegisterBus and ProbeAll are called sequentially
 * from DeviceInit(). The SpinLock protects against future concurrent access
 * (e.g. hotplug) but is not strictly needed at boot.
 */
class DeviceManager {
 public:
  /**
   * @brief Register a bus and immediately enumerate its devices.
   *
   * @tparam B  Bus type (must satisfy Bus concept)
   * @return Expected<void> kOutOfMemory if device table is full
   */
  template <Bus B>
  auto RegisterBus(B& bus) -> Expected<void> {
    LockGuard guard(lock_);

    if (device_count_ >= kMaxDevices) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }

    size_t remaining = kMaxDevices - device_count_;
    auto result = bus.Enumerate(devices_ + device_count_, remaining);
    if (!result.has_value()) {
      klog::Err("DeviceManager: bus '%s' enumeration failed: %s\n",
                B::GetName(), result.error().message());
      return std::unexpected(result.error());
    }

    size_t count = result.value();
    for (size_t i = 0; i < count; ++i) {
      devices_[device_count_ + i].dev_id = next_dev_id_++;
    }
    device_count_ += count;

    klog::Info("DeviceManager: bus '%s' enumerated %zu device(s)\n",
               B::GetName(), count);
    return {};
  }

  /**
   * @brief Match registered drivers against all unbound devices and probe.
   *
   * For each device:
   *   1. FindDriver() — compatible string O(N) scan
   *   2. drv->match() — fine-grained hardware detection (may read MMIO)
   *   3. Set node.bound = true under lock_
   *   4. drv->probe() — driver initialization
   *
   * @return Expected<void> (always succeeds; per-device errors are logged)
   */
  auto ProbeAll() -> Expected<void> {
    LockGuard guard(lock_);

    size_t probed = 0;
    for (size_t i = 0; i < device_count_; ++i) {
      auto& node = devices_[i];

      const auto* drv = registry_.FindDriver(node);
      if (drv == nullptr) {
        klog::Debug("DeviceManager: no driver for '%s'\n", node.name);
        continue;
      }

      // Fine-grained hardware detection (may read MMIO registers)
      if (!drv->match(node)) {
        klog::Debug("DeviceManager: driver '%s' rejected '%s'\n",
                    drv->descriptor->name, node.name);
        continue;
      }

      // Check+claim under the DeviceManager lock (no per-node lock needed)
      if (node.bound) continue;
      node.bound = true;

      klog::Info("DeviceManager: probing '%s' with driver '%s'\n",
                 node.name, drv->descriptor->name);

      auto result = drv->probe(node);
      if (!result.has_value()) {
        klog::Err("DeviceManager: probe '%s' failed: %s\n", node.name,
                  result.error().message());
        node.bound = false;  // rollback
        continue;
      }

      ++probed;
      klog::Info("DeviceManager: '%s' bound to '%s'\n", node.name,
                 drv->descriptor->name);
    }

    klog::Info("DeviceManager: probed %zu device(s)\n", probed);
    return {};
  }

  /**
   * @brief Find a device by name.
   *
   * @note Read-only after enumeration; safe without lock once ProbeAll()
   *       has completed.
   */
  auto FindDevice(const char* name) -> Expected<DeviceNode*> {
    for (size_t i = 0; i < device_count_; ++i) {
      if (kstd::strcmp(devices_[i].name, name) == 0) {
        return &devices_[i];
      }
    }
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  /**
   * @brief Enumerate devices by type.
   *
   * @param  type  Device type to filter
   * @param  out   Output pointer array
   * @param  max   Maximum pointers to write
   * @return Number of matching devices written to out
   */
  auto FindDevicesByType(DeviceType type, DeviceNode** out, size_t max)
      -> size_t {
    size_t found = 0;
    for (size_t i = 0; i < device_count_ && found < max; ++i) {
      if (devices_[i].type == type) {
        out[found++] = &devices_[i];
      }
    }
    return found;
  }

  /// Access the driver registry (for driver registration in DeviceInit).
  auto GetRegistry() -> DriverRegistry& { return registry_; }

  DeviceManager() = default;
  ~DeviceManager() = default;
  DeviceManager(const DeviceManager&) = delete;
  DeviceManager(DeviceManager&&) = delete;
  auto operator=(const DeviceManager&) -> DeviceManager& = delete;
  auto operator=(DeviceManager&&) -> DeviceManager& = delete;

 private:
  static constexpr size_t kMaxDevices = 64;
  DeviceNode devices_[kMaxDevices]{};
  size_t device_count_{0};
  uint32_t next_dev_id_{0};
  DriverRegistry registry_;
  SpinLock lock_{"device_manager"};
};

using DeviceManagerSingleton = etl::singleton<DeviceManager>;

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_
```

**Step 2: Attempt incremental build**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Remaining errors should be in `device.cpp` only.

---

## Task 9: Rewrite `device.cpp`

**Files:**
- Modify: `src/device/device.cpp`

**Goal:** Remove the DMA pre-allocation loop (DMA is now in `VirtioBlkDriver::Probe()`). Use the new `Register(entry)` API. Use `VirtioBlkDriver::Instance()` to get the singleton.

**Step 1: Write the new `device.cpp`**

Replace the entire file with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "block_device_provider.hpp"
#include "device_manager.hpp"
#include "driver/ns16550a_driver.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "platform_bus.hpp"

/// Device subsystem initialization — called once from main.cpp boot sequence.
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();

  // Register drivers (one line each — no templates)
  if (auto r = dm.GetRegistry().Register(Ns16550aDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register Ns16550aDriver failed: %s\n",
              r.error().message());
    return;
  }
  if (auto r = dm.GetRegistry().Register(VirtioBlkDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register VirtioBlkDriver failed: %s\n",
              r.error().message());
    return;
  }

  // Enumerate devices via FDT
  PlatformBus platform_bus(KernelFdtSingleton::instance());
  if (auto r = dm.RegisterBus(platform_bus); !r) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              r.error().message());
    return;
  }

  // Match and probe
  if (auto r = dm.ProbeAll(); !r) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n", r.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}

namespace {

/// Adapts VirtioBlk<> to vfs::BlockDevice for the filesystem layer.
class VirtioBlkBlockDevice final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = VirtioBlkDriver::VirtioBlkType;

  explicit VirtioBlkBlockDevice(VirtioBlkType* dev) : dev_(dev) {}

  auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  auto WriteSectors(uint64_t lba, uint32_t count, const void* buf)
      -> Expected<size_t> override {
    const auto* ptr = static_cast<const uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Write(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  [[nodiscard]] auto GetSectorSize() const -> uint32_t override {
    return kSectorSize;
  }

  [[nodiscard]] auto GetSectorCount() const -> uint64_t override {
    return dev_->GetCapacity();
  }

  [[nodiscard]] auto GetName() const -> const char* override {
    return "virtio-blk0";
  }

 private:
  static constexpr uint32_t kSectorSize = 512;
  VirtioBlkType* dev_;
};

}  // namespace

auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice* {
  auto* raw = VirtioBlkDriver::Instance().GetDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: no virtio-blk device probed\n");
    return nullptr;
  }
  static VirtioBlkBlockDevice adapter(raw);
  return &adapter;
}
```

**Step 2: Delete the old `df_bridge.hpp` now that all callers have been migrated**

```bash
rm src/device/include/df_bridge.hpp
```

**Step 3: Build**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1
```

Expected: build succeeds with zero errors.

**Step 4: If the build fails, check for**

- Any file still `#include "df_bridge.hpp"` → update to `mmio_helper.hpp`
- Any file still accessing `node.resource.*` → use `node.mmio_base`, `node.irq`, `node.compatible`
- Any file still calling `node.TryBind()` / `node.Release()` → remove (ProbeAll handles bound)
- Any file still calling `dm.GetRegistry().Register<T>()` → use `Register(T::GetEntry())`
- Any file still calling `DriverRegistry::GetDriverInstance<T>()` → use `T::Instance()`

---

## Task 10: Check CMakeLists.txt for header references

**Files:**
- Read: `src/device/CMakeLists.txt` (if it exists)
- Read: `CMakeLists.txt` in repo root

**Step 1: Check if any CMakeLists references deleted/renamed files**

```bash
grep -r "df_bridge\|acpi_bus\|pci_bus\|platform_traits\|virtio_messages" \
  --include="CMakeLists.txt" .
```

Expected: no matches (these are header-only files, not compiled separately). If there are matches, update the file list.

**Step 2: No changes needed** if no matches found.

---

## Task 11: Full build + format verification

**Step 1: Full clean build**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
```

Expected: exits 0 with `[100%] Built target SimpleKernel`.

**Step 2: Run pre-commit format check**

```bash
pre-commit run --all-files
```

Expected: all checks pass. If clang-format reports diffs, apply them:

```bash
pre-commit run clang-format --all-files
git diff  # inspect changes
git add -A && git commit --signoff -m "style(device): apply clang-format to simplified device module"
```

**Step 3: Run unit tests (if host arch matches)**

```bash
cd build_riscv64 && make unit-test 2>&1 | tail -30
```

Expected: all tests pass (or pre-existing failures with no new failures).

---

## Task 12: Final commit

**Step 1: Review all changed files**

```bash
git diff --stat HEAD
```

Expected changed files:
- `src/device/include/device_node.hpp` (rewritten)
- `src/device/include/driver_registry.hpp` (rewritten)
- `src/device/include/mmio_helper.hpp` (new)
- `src/device/include/platform_bus.hpp` (updated)
- `src/device/include/device_manager.hpp` (updated)
- `src/device/include/driver/ns16550a_driver.hpp` (updated)
- `src/device/include/driver/virtio_blk_driver.hpp` (updated)
- `src/device/device.cpp` (rewritten)
- Deleted: `acpi_bus.hpp`, `pci_bus.hpp`, `platform_traits.hpp`, `virtio_messages.hpp`, `df_bridge.hpp`

**Step 2: Final commit**

```bash
git add -A
git commit --signoff -m "refactor(device): simplify device module — remove Driver concept and variant types

Replace template Register<D>() + Driver concept with plain DriverEntry
function-pointer struct. Flatten DeviceNode to pure data (remove variant
BusId, DeviceResource, DMA buffer, SpinLock). Move DMA allocation into
VirtioBlkDriver::Probe(). Delete placeholder files (acpi_bus, pci_bus,
platform_traits, virtio_messages) and rename df_bridge → mmio_helper.

Net: ~1149 → ~570 lines (~50% reduction), 13 → 8 headers."
```

---

## Success Criteria (verify before marking done)

1. `driver_registry.hpp` contains no C++ concepts or function templates
2. `device_node.hpp` contains no `variant`, `unique_ptr`, `SpinLock`, or `atomic`
3. `DeviceInit()` contains no driver-specific strings or constants (only `GetEntry()` calls)
4. `VirtioBlkDriver::Probe()` contains no hardware register reads — only initialization (`VirtioBlkType::Create`)
5. `acpi_bus.hpp`, `pci_bus.hpp`, `df_bridge.hpp` are deleted
6. Build passes: `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`
7. Pre-commit passes: `pre-commit run --all-files`
