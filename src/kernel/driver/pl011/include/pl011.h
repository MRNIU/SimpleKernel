
/**
 * @file pl011.h
 * @brief pl011 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-05-24
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-05-24<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_DRIVER_PL011_INCLUDE_PL011_H_
#define SIMPLEKERNEL_SRC_KERNEL_DRIVER_PL011_INCLUDE_PL011_H_

#include <cstdint>

/**
 * @brief PL011 串口驱动
 * @see https://developer.arm.com/documentation/ddi0183/g/
 */
class Pl011 {
 public:
  /**
   * 构造函数
   * @param dev_addr 设备地址
   */
  explicit Pl011(uint64_t dev_addr);

  /// @name 默认构造/析构函数
  /// @{
  Pl011() = delete;
  Pl011(const Pl011& na16550a) = delete;
  Pl011(Pl011&& na16550a) = delete;
  auto operator=(const Pl011& na16550a) -> Pl011& = delete;
  auto operator=(Pl011&& na16550a) -> Pl011& = delete;
  ~Pl011() = default;
  /// @}

  void PutChar(uint8_t c);

 private:
  static constexpr const uint32_t DR_OFFSET = 0x000;
  static constexpr const uint32_t FR_OFFSET = 0x018;
  static constexpr const uint32_t IBRD_OFFSET = 0x024;
  static constexpr const uint32_t FBRD_OFFSET = 0x028;
  static constexpr const uint32_t LCR_OFFSET = 0x02c;
  static constexpr const uint32_t CR_OFFSET = 0x030;
  static constexpr const uint32_t IMSC_OFFSET = 0x038;
  static constexpr const uint32_t DMACR_OFFSET = 0x048;

  uint64_t base_addr_;
  uint64_t base_clock;
  uint32_t baudrate;

  void calculate_divisiors(uint32_t* integer, uint32_t* fractional) {
    // 64 * F_UARTCLK / (16 * B) = 4 * F_UARTCLK / B
    const uint32_t div = 4 * base_clock / baudrate;

    *fractional = div & 0x3f;
    *integer = (div >> 6) & 0xffff;
  }

  static const uint32_t FR_BUSY = (1 << 3);

  void wait_tx_complete(const struct pl011* dev) {
    while ((*Read(FR_OFFSET) * FR_BUSY) != 0) {
    }
  }

  inline volatile uint8_t* Reg(uint8_t reg);

  inline uint8_t Read(uint8_t reg);

  inline void Write(uint8_t reg, uint8_t c);
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_DRIVER_PL011_INCLUDE_PL011_H_ */
