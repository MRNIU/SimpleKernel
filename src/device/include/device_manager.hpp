/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 设备管理器
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_

#include "bus.hpp"
#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "spinlock.hpp"

/// 设备管理器 — 管理所有设备节点和驱动
class DeviceManager {
 public:
  /// 注册一条总线并立即枚举其上的设备
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
    // 分配全局设备编号
    for (size_t i = 0; i < count; ++i) {
      devices_[device_count_ + i].dev_id = next_dev_id_++;
    }
    device_count_ += count;

    klog::Info("DeviceManager: bus '%s' enumerated %zu device(s)\n",
               B::GetName(), count);
    return {};
  }

  /// 匹配已注册驱动并 Probe 所有未绑定设备
  auto ProbeAll() -> Expected<void> {
    LockGuard guard(lock_);

    size_t probed = 0;
    for (size_t i = 0; i < device_count_; ++i) {
      if (devices_[i].bound.load(std::memory_order_acquire)) {
        continue;
      }

      auto* drv = registry_.FindDriver(devices_[i].resource);
      if (drv == nullptr) {
        klog::Debug("DeviceManager: no driver for '%s'\n", devices_[i].name);
        continue;
      }

      klog::Info("DeviceManager: probing '%s' with driver '%s'\n",
                 devices_[i].name, drv->descriptor->name);

      auto result = drv->probe(devices_[i]);
      if (!result.has_value()) {
        klog::Err("DeviceManager: probe '%s' failed: %s\n", devices_[i].name,
                  result.error().message());
        continue;
      }

      devices_[i].bound.store(true, std::memory_order_release);
      ++probed;
      klog::Info("DeviceManager: '%s' bound to '%s'\n", devices_[i].name,
                 drv->descriptor->name);
    }

    klog::Info("DeviceManager: probed %zu device(s)\n", probed);
    return {};
  }

  /// 通过名称查找设备（只读路径，设备表稳定后无需加锁）
  auto FindDevice(const char* name) -> Expected<DeviceNode*> {
    for (size_t i = 0; i < device_count_; ++i) {
      if (strcmp(devices_[i].name, name) == 0) {
        return &devices_[i];
      }
    }
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  /// 通过类型枚举设备
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

  /// 获取驱动注册表
  auto GetRegistry() -> DriverRegistry& { return registry_; }

  /// @name 构造/析构函数
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

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_ */
