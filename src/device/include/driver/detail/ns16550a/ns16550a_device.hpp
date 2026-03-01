/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_NS16550A_NS16550A_DEVICE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_NS16550A_NS16550A_DEVICE_HPP_

#include <cstdint>

#include "driver/detail/ns16550a/ns16550a.hpp"
#include "driver/detail/uart_device.hpp"

namespace detail::ns16550a {

/**
 * @brief NS16550A 字符设备
 *
 * 将底层 Ns16550a 驱动适配到统一的 CharDevice 接口。
 * 支持 Open/Release/Read/Write/Poll 操作。
 */
class Ns16550aDevice : public UartDevice<Ns16550aDevice, Ns16550a> {
 public:
  Ns16550aDevice() = default;

  /**
   * @brief 工厂方法：创建已初始化的 NS16550A 字符设备
   * @param base_addr 设备 MMIO 基地址
   * @return 成功返回已初始化的 Ns16550aDevice 实例，失败返回错误
   */
  [[nodiscard]] static auto Create(uint64_t base_addr)
      -> Expected<Ns16550aDevice> {
    auto driver = Ns16550a::Create(base_addr);
    if (!driver) {
      return std::unexpected(driver.error());
    }
    Ns16550aDevice dev;
    dev.driver_ = std::move(*driver);
    return dev;
  }
};

}  // namespace detail::ns16550a

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_NS16550A_NS16550A_DEVICE_HPP_
