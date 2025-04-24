
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

#include "pl011.h"

#include "io.hpp"

Pl011::Pl011(uint64_t dev_addr, uint64_t clock, uint64_t baud_rate)
    : base_addr_(dev_addr), base_clock_(clock), baud_rate_(baud_rate) {
  // Clear all errors
  io::Out<uint32_t>(base_addr_ + kRegRSRECR, 0);
  // Disable everything
  io::Out<uint32_t>(base_addr_ + kRegCR, 0);

  if (baud_rate_ != 0) {
    uint32_t divisor = (base_clock_ * 4) / baud_rate_;

    io::Out<uint32_t>(base_addr_ + kRegIBRD, divisor >> 6);
    io::Out<uint32_t>(base_addr_ + kRegFBRD, divisor & 0x3f);
  }

  // Configure TX to 8 bits, 1 stop bit, no parity, fifo disabled.
  io::Out<uint32_t>(base_addr_ + kRegLCRH, kLCRHWlen8);

  // Enable receive interrupt
  io::Out<uint32_t>(base_addr_ + kRegIMSC, kIMSCRxim);

  // Enable UART and RX/TX
  io::Out<uint32_t>(base_addr_ + kRegCR, kCREnable | kCRTxEnable | kCRRxEnable);
}

void Pl011::PutChar(uint8_t c) {
  // Wait until there is space in the FIFO or device is disabled
  while (io::In<uint32_t>(base_addr_ + kRegFR) & kFRTxFIFO) {
    ;
  }

  io::Out<uint32_t>(base_addr_ + kRegDR, c);
}
