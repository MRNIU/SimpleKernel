/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "driver.h"

#include <cstdint>

#include "device_manager.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"

auto DeviceInit(int, const char**) -> void {
  Singleton<DeviceManager<EnvironmentTraits>>::GetInstance() =
      DeviceManager<EnvironmentTraits>{};
  auto& device_manager =
      Singleton<DeviceManager<EnvironmentTraits>>::GetInstance();

  device_manager.AddDevice<MmioTransport>("virtio,mmio",
                                          MmioTransport{0x10007000});
}
