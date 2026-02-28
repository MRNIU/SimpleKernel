/**
 * @copyright Copyright The SimpleKernel Contributors
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
 * @brief  设备管理器 — 管理所有设备节点和驱动
 */
class DeviceManager {
 public:
  /**
   * @brief  注册一条总线并立即枚举其上的设备
   * @tparam B              总线类型
   * @param  bus            总线实例
   * @return Expected<void> 成功时返回 void，失败时返回错误
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
    // 分配全局设备编号
    for (size_t i = 0; i < count; ++i) {
      devices_[device_count_ + i].dev_id = next_dev_id_++;
    }
    device_count_ += count;

    klog::Info("DeviceManager: bus '%s' enumerated %zu device(s)\n",
               B::GetName(), count);
    return {};
  }

  /**
   * @brief  匹配已注册驱动并 Probe 所有未绑定设备
   * @return Expected<void> 成功时返回 void，失败时返回错误
   */
  auto ProbeAll() -> Expected<void> {
    LockGuard guard(lock_);

    size_t probed = 0;
    for (size_t i = 0; i < device_count_; ++i) {
      auto* drv = registry_.FindDriver(devices_[i].resource);
      if (drv == nullptr) {
        klog::Debug("DeviceManager: no driver for '%s'\n", devices_[i].name);
        continue;
      }

      // TryBind() 原子检查并绑定，已绑定则跳过
      if (!devices_[i].TryBind()) {
        continue;
      }

      klog::Info("DeviceManager: probing '%s' with driver '%s'\n",
                 devices_[i].name, drv->descriptor->name);

      auto result = drv->probe(devices_[i]);
      if (!result.has_value()) {
        klog::Err("DeviceManager: probe '%s' failed: %s\n", devices_[i].name,
                  result.error().message());
        devices_[i].Release();  // 探测失败，回滚绑定
        continue;
      }

      ++probed;
      klog::Info("DeviceManager: '%s' bound to '%s'\n", devices_[i].name,
                 drv->descriptor->name);
    }

    klog::Info("DeviceManager: probed %zu device(s)\n", probed);
    return {};
  }

  /**
   * @brief  通过名称查找设备
   * @note   并发安全：由于是只读路径，在设备枚举完成且设备表（device_count_ 和
   *         devices_）稳定后，并发调用是安全的，无需加锁。
   * @param  name           设备名称
   * @return Expected<DeviceNode*> 成功时返回设备节点指针；失败时返回
   * kDeviceNotFound 错误
   */
  auto FindDevice(const char* name) -> Expected<DeviceNode*> {
    for (size_t i = 0; i < device_count_; ++i) {
      if (strcmp(devices_[i].name, name) == 0) {
        return &devices_[i];
      }
    }
    return std::unexpected(Error(ErrorCode::kDeviceNotFound));
  }

  /**
   * @brief  通过类型枚举设备
   * @param  type           设备类型
   * @param  out            输出设备节点指针数组
   * @param  max            最大枚举数量
   * @return size_t         实际找到的设备数量
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

  /**
   * @brief  获取驱动注册表
   * @return DriverRegistry& 驱动注册表实例
   */
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

using DeviceManagerSingleton = etl::singleton<DeviceManager>;

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DEVICE_MANAGER_HPP_
