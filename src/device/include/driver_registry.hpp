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
  auto (*match)(DeviceNode&) -> bool;             ///< Hardware detection
  auto (*probe)(DeviceNode&) -> Expected<void>;   ///< Driver initialization
  auto (*remove)(DeviceNode&) -> Expected<void>;  ///< Driver teardown
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
