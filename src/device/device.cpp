/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "device_manager.hpp"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "ns16550a/ns16550a_driver.hpp"
#include "platform_bus.hpp"
#include "virtio/virtio_driver.hpp"

/// 设备子系统初始化入口
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();

  if (auto r = dm.GetRegistry().Register(Ns16550aDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register Ns16550aDriver failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.GetRegistry().Register(VirtioDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register VirtioDriver failed: %s\n",
              r.error().message());
    return;
  }

  PlatformBus platform_bus(KernelFdtSingleton::instance());
  if (auto r = dm.RegisterBus(platform_bus); !r) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.ProbeAll(); !r) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n", r.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}
