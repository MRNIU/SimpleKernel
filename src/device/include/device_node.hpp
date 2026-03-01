/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Device node — per-device hardware description (plain data struct)
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_

#include <concepts>
#include <cstddef>
#include <cstdint>

#include "expected.hpp"

/// Bus type discriminator — extension point for future PCI/ACPI buses
enum class BusType : uint8_t { kPlatform, kPci, kAcpi };

/// Device category
enum class DeviceType : uint8_t {
  kChar,      ///< Character device (serial, etc.)
  kBlock,     ///< Block device (disk, etc.)
  kNet,       ///< Network device
  kPlatform,  ///< Platform device (interrupt controller, timer, etc.)
};

namespace vfs {
class BlockDevice;
}  // namespace vfs

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

  /// Set by the driver's Probe() — points to a kernel-lifetime adapter.
  /// nullptr if not a block device or not yet probed.
  vfs::BlockDevice* block_device{nullptr};
};

/// 总线 concept — 每种总线负责枚举自己管辖范围内的设备
template <typename B>
concept Bus = requires(B b, DeviceNode* out, size_t max) {
  /// 枚举该总线上所有设备，填充到 out[]，返回发现的设备数
  { b.Enumerate(out, max) } -> std::same_as<Expected<size_t>>;
  /// 返回总线名称（用于日志）
  { B::GetName() } -> std::same_as<const char*>;
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_NODE_HPP_
