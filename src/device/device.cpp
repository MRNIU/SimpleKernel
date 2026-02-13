/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "device_manager.hpp"
#include "driver/ns16550a_driver.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "platform_bus.hpp"
#include "platform_traits.hpp"
#include "singleton.hpp"

/// 设备初始化入口
auto DeviceInit() -> void {
  auto& dm = Singleton<DeviceManager>::GetInstance();
  auto& fdt = Singleton<KernelFdt>::GetInstance();

  // 注册驱动
  auto reg_uart = dm.GetRegistry().Register<Ns16550aDriver>();
  if (!reg_uart.has_value()) {
    klog::Err("DeviceInit: failed to register Ns16550aDriver: %s\n",
              reg_uart.error().message());
    return;
  }

  auto reg_blk = dm.GetRegistry().Register<VirtioBlkDriver<PlatformTraits>>();
  if (!reg_blk.has_value()) {
    klog::Err("DeviceInit: failed to register VirtioBlkDriver: %s\n",
              reg_blk.error().message());
    return;
  }

  // 注册 Platform 总线并枚举设备
  PlatformBus platform_bus(fdt);
  auto bus_result = dm.RegisterBus(platform_bus);
  if (!bus_result.has_value()) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              bus_result.error().message());
    return;
  }

  // 匹配驱动并 Probe
  auto probe_result = dm.ProbeAll();
  if (!probe_result.has_value()) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n",
              probe_result.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}
