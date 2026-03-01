/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Driver registry — ETL-based entries with delegate callbacks;
 *        mmio_helper — safe MMIO region setup helpers
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_

#include <etl/delegate.h>
#include <etl/flat_map.h>
#include <etl/span.h>
#include <etl/vector.h>

#include <cstddef>
#include <cstdint>

#include "device_node.hpp"
#include "expected.hpp"
#include "kernel.h"
#include "kernel_log.hpp"
#include "kstd_cstring"
#include "spinlock.hpp"
#include "virtual_memory.hpp"

/// One entry in a driver's static match table
struct MatchEntry {
  BusType bus_type;
  const char* compatible;  ///< FDT compatible string (Platform)
                           ///< or vendor/HID string (PCI/ACPI — future)
};

/**
 * @brief Type-erased driver entry — one per registered driver.
 *
 * @pre  match/probe/remove delegates are bound before registration
 */
struct DriverEntry {
  const char* name;
  etl::span<const MatchEntry> match_table;
  etl::delegate<bool(DeviceNode&)> match;             ///< Hardware detection
  etl::delegate<Expected<void>(DeviceNode&)> probe;   ///< Driver initialization
  etl::delegate<Expected<void>(DeviceNode&)> remove;  ///< Driver teardown
};

/// Comparator for const char* keys in flat_map (uses kstd::strcmp).
struct CStrLess {
  auto operator()(const char* a, const char* b) const -> bool {
    return kstd::strcmp(a, b) < 0;
  }
};

/**
 * @brief Driver registry — ETL vector of DriverEntry with flat_map compat
 * index.
 *
 * Registration builds an etl::flat_map from compatible string → driver index,
 * reducing FindDriver from O(N·M·K) to O(Cn · log T).
 */
class DriverRegistry {
 public:
  /**
   * @brief Register a driver entry.
   *
   * @pre  entry.match/probe/remove delegates are bound
   * @return Expected<void> kOutOfMemory if registry is full
   */
  auto Register(const DriverEntry& entry) -> Expected<void>;

  /**
   * @brief Find the first driver whose match_table has a compatible string
   *        present in node.compatible (flat_map lookup, O(Cn · log T)).
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
  /// Max total MatchEntry rows across all drivers (32 drivers × ~3 compat
  /// strings)
  static constexpr size_t kMaxCompatEntries = 96;

  etl::vector<DriverEntry, kMaxDrivers> drivers_;
  etl::flat_map<const char*, size_t, kMaxCompatEntries, CStrLess> compat_map_;
  SpinLock lock_{"driver_registry"};
};

// --- Inline implementations (header-only per module convention) ---

inline auto DriverRegistry::Register(const DriverEntry& entry)
    -> Expected<void> {
  LockGuard guard(lock_);
  if (drivers_.full()) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }
  const size_t idx = drivers_.size();
  for (const auto& me : entry.match_table) {
    if (compat_map_.full()) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }
    // insert() is a no-op on duplicate key — first-registered driver wins.
    compat_map_.insert({me.compatible, idx});
  }
  drivers_.push_back(entry);
  return {};
}

inline auto DriverRegistry::FindDriver(const DeviceNode& node)
    -> const DriverEntry* {
  // Walk the node's compatible stringlist; for each string, do a flat_map
  // lookup.
  const char* p = node.compatible;
  const char* end = node.compatible + node.compatible_len;
  while (p < end) {
    const auto it = compat_map_.find(p);
    if (it != compat_map_.end()) {
      auto& entry = drivers_[it->second];
      if (entry.match(const_cast<DeviceNode&>(node))) {
        return &entry;
      }
    }
    p += kstd::strlen(p) + 1;
  }
  return nullptr;
}

// ---------------------------------------------------------------------------
// mmio_helper — safe MMIO region setup (formerly mmio_helper.hpp)
// ---------------------------------------------------------------------------

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

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_REGISTRY_HPP_
