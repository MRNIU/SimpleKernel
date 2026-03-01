/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Device manager — owns device nodes and driver registry
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_

#include <etl/singleton.h>

#include <cassert>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "spinlock.hpp"

/**
 * @brief Device manager — manages all device nodes and drivers.
 *
 * @pre  Memory subsystem must be initialised before calling any method
 * @post After ProbeAll(), bound devices are ready for use
 */
class DeviceManager {
 public:
  /**
   * @brief Register a bus and immediately enumerate its devices.
   *
   * @tparam B              Bus type (must satisfy Bus concept)
   * @param  bus            Bus instance
   * @return Expected<void> void on success, error on failure
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
   * @brief Match registered drivers and probe all unbound devices.
   *
   * @return Expected<void> void on success, error on failure
   */
  auto ProbeAll() -> Expected<void>;

  /**
   * @brief Find a device by name.
   *
   * @note  Read-only path — safe for concurrent callers once device_count_
   *        and devices_ are stable (post-enumeration).
   * @param  name           Device name
   * @return Expected<DeviceNode*> device pointer, or kDeviceNotFound
   */
  auto FindDevice(const char* name) -> Expected<DeviceNode*>;

  /**
   * @brief Enumerate devices by type.
   *
   * @param  type           Device type
   * @param  out            Output array of device node pointers
   * @param  max            Maximum results
   * @return size_t         Number of matching devices found
   */
  auto FindDevicesByType(DeviceType type, DeviceNode** out, size_t max)
      -> size_t;

  /**
   * @brief Access the driver registry.
   *
   * @return DriverRegistry& driver registry instance
   */
  auto GetRegistry() -> DriverRegistry& { return registry_; }

  /// @name Construction / destruction
  /// @{
  DeviceManager() = default;
  ~DeviceManager() = default;
  DeviceManager(const DeviceManager&) = delete;
  DeviceManager(DeviceManager&&) = delete;
  auto operator=(const DeviceManager&) -> DeviceManager& = delete;
  auto operator=(DeviceManager&&) -> DeviceManager& = delete;
  /// @}

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
