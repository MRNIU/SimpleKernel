/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "device_manager.hpp"

#include <cassert>

#include "kernel_log.hpp"
#include "kstd_cstring"

auto DeviceManager::ProbeAll() -> Expected<void> {
  LockGuard guard(lock_);

  size_t probed = 0;
  size_t no_driver_count = 0;
  for (size_t i = 0; i < device_count_; ++i) {
    auto& node = devices_[i];

    const auto* drv = registry_.FindDriver(node);
    if (drv == nullptr) {
      ++no_driver_count;
      continue;
    }

    if (!drv->match(node)) {
      klog::Debug("DeviceManager: driver '%s' rejected '%s'\n", drv->name,
                  node.name);
      continue;
    }

    if (node.bound) {
      continue;
    }
    node.bound = true;

    klog::Info("DeviceManager: probing '%s' with driver '%s'\n", node.name,
               drv->name);

    auto result = drv->probe(node);
    if (!result.has_value()) {
      klog::Err("DeviceManager: probe '%s' failed: %s\n", node.name,
                result.error().message());
      node.bound = false;
      continue;
    }

    ++probed;
    klog::Info("DeviceManager: '%s' bound to '%s'\n", node.name, drv->name);
  }

  klog::Info("DeviceManager: probed %zu device(s), %zu skipped (no driver)\n",
             probed, no_driver_count);
  return {};
}

auto DeviceManager::FindDevice(const char* name) -> Expected<DeviceNode*> {
  assert(name != nullptr && "FindDevice: name must not be null");
  for (size_t i = 0; i < device_count_; ++i) {
    if (kstd::strcmp(devices_[i].name, name) == 0) {
      return &devices_[i];
    }
  }
  return std::unexpected(Error(ErrorCode::kDeviceNotFound));
}

auto DeviceManager::FindDevicesByType(DeviceType type, DeviceNode** out,
                                      size_t max) -> size_t {
  assert((out != nullptr || max == 0) &&
         "FindDevicesByType: out must not be null when max > 0");
  size_t found = 0;
  for (size_t i = 0; i < device_count_ && found < max; ++i) {
    if (devices_[i].type == type) {
      out[found++] = &devices_[i];
    }
  }
  return found;
}
