/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_PL011_PL011_DEVICE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_PL011_PL011_DEVICE_HPP_

#include <cstdint>

#include "driver/detail/pl011/pl011.hpp"
#include "driver/detail/uart_device.hpp"

namespace detail::pl011 {

/**
 * @brief PL011 字符设备
 *
 * 将底层 Pl011 驱动适配到统一的 CharDevice 接口。
 * 支持 Open/Release/Read/Write/Poll 操作。
 */
class Pl011Device : public UartDevice<Pl011Device, Pl011> {
 public:
  Pl011Device() = default;
  explicit Pl011Device(uint64_t base_addr) { driver_ = Pl011(base_addr); }
  Pl011Device(uint64_t base_addr, uint64_t clock, uint64_t baud_rate) {
    driver_ = Pl011(base_addr, clock, baud_rate);
  }
};

}  // namespace detail::pl011

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_PL011_PL011_DEVICE_HPP_
