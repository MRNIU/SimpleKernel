
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

#ifndef SIMPLEKERNEL_SRC_DRIVER_PL011_INCLUDE_PL011_H_
#define SIMPLEKERNEL_SRC_DRIVER_PL011_INCLUDE_PL011_H_

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
   * @param uart_clk 串口时钟
   * @param baud_rate 波特率
   */
  explicit Pl011(uint64_t dev_addr, uint64_t clock = 0, uint64_t baud_rate = 0);

  /// @name 默认构造/析构函数
  /// @{
  Pl011() = default;
  Pl011(const Pl011&) = delete;
  Pl011(Pl011&&) = default;
  auto operator=(const Pl011&) -> Pl011& = delete;
  auto operator=(Pl011&&) -> Pl011& = default;
  ~Pl011() = default;
  /// @}

  void PutChar(uint8_t c) const;

  /**
   * 阻塞式读取一个字符
   * @return 读取到的字符
   */
  [[nodiscard]] auto GetChar() const -> uint8_t;

  /**
   * 非阻塞式尝试读取一个字符
   * @return 读取到的字符，如果没有数据则返回 -1
   */
  [[nodiscard]] auto TryGetChar() const -> uint8_t;

 private:
  /// data register
  static constexpr const uint32_t kRegDR = 0x00;
  /// receive status or error clear
  static constexpr const uint32_t kRegRSRECR = 0x04;
  /// DMA watermark configure
  static constexpr const uint32_t kRegDmaWm = 0x08;
  /// Timeout period
  static constexpr const uint32_t kRegTimeOut = 0x0C;
  /// flag register
  static constexpr const uint32_t kRegFR = 0x18;
  /// IrDA low-poer
  static constexpr const uint32_t kRegILPR = 0x20;
  /// integer baud register
  static constexpr const uint32_t kRegIBRD = 0x24;
  /// fractional baud register
  static constexpr const uint32_t kRegFBRD = 0x28;
  /// line control register
  static constexpr const uint32_t kRegLCRH = 0x2C;
  /// control register
  static constexpr const uint32_t kRegCR = 0x30;
  /// interrupt FIFO level select
  static constexpr const uint32_t kRegIFLS = 0x34;
  /// interrupt mask set/clear
  static constexpr const uint32_t kRegIMSC = 0x38;
  /// raw interrupt register
  static constexpr const uint32_t kRegRIS = 0x3C;
  /// masked interrupt register
  static constexpr const uint32_t kRegMIS = 0x40;
  /// interrupt clear register
  static constexpr const uint32_t kRegICR = 0x44;
  /// DMA control register
  static constexpr const uint32_t kRegDmaCR = 0x48;

  /// flag register bits
  static constexpr const uint32_t kFRRTXDIS = (1 << 13);
  static constexpr const uint32_t kFRTERI = (1 << 12);
  static constexpr const uint32_t kFRDDCD = (1 << 11);
  static constexpr const uint32_t kFRDDSR = (1 << 10);
  static constexpr const uint32_t kFRDCTS = (1 << 9);
  static constexpr const uint32_t kFRRI = (1 << 8);
  static constexpr const uint32_t kFRTXFE = (1 << 7);
  static constexpr const uint32_t kFRRXFF = (1 << 6);
  static constexpr const uint32_t kFRTxFIFO = (1 << 5);
  static constexpr const uint32_t kFRRXFE = (1 << 4);
  static constexpr const uint32_t kFRBUSY = (1 << 3);
  static constexpr const uint32_t kFRDCD = (1 << 2);
  static constexpr const uint32_t kFRDSR = (1 << 1);
  static constexpr const uint32_t kFRCTS = (1 << 0);

  /// transmit/receive line register bits
  static constexpr const uint32_t kLCRHSPS = (1 << 7);
  static constexpr const uint32_t kLCRHWlen8 = (3 << 5);
  static constexpr const uint32_t kLCRHWLEN_7 = (2 << 5);
  static constexpr const uint32_t kLCRHWLEN_6 = (1 << 5);
  static constexpr const uint32_t kLCRHWLEN_5 = (0 << 5);
  static constexpr const uint32_t kLCRHFEN = (1 << 4);
  static constexpr const uint32_t kLCRHSTP2 = (1 << 3);
  static constexpr const uint32_t kLCRHEPS = (1 << 2);
  static constexpr const uint32_t kLCRHPEN = (1 << 1);
  static constexpr const uint32_t kLCRHBRK = (1 << 0);

  /// control register bits
  static constexpr const uint32_t kCRCTSEN = (1 << 15);
  static constexpr const uint32_t kCRRTSEN = (1 << 14);
  static constexpr const uint32_t kCROUT2 = (1 << 13);
  static constexpr const uint32_t kCROUT1 = (1 << 12);
  static constexpr const uint32_t kCRRTS = (1 << 11);
  static constexpr const uint32_t kCRDTR = (1 << 10);
  static constexpr const uint32_t kCRRxEnable = (1 << 9);
  static constexpr const uint32_t kCRTxEnable = (1 << 8);
  static constexpr const uint32_t kCRLPE = (1 << 7);
  static constexpr const uint32_t kCROVSFACT = (1 << 3);
  static constexpr const uint32_t kCREnable = (1 << 0);

  static constexpr const uint32_t kIMSCRTIM = (1 << 6);
  static constexpr const uint32_t kIMSCRxim = (1 << 4);

  uint64_t base_addr_ = 0;
  uint64_t base_clock_ = 0;
  uint64_t baud_rate_ = 0;
};

#endif /* SIMPLEKERNEL_SRC_DRIVER_PL011_INCLUDE_PL011_H_ */
