
/**
 * @file ns16550a.h
 * @brief ns16550a 头文件
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

#include "ns16550a.h"

#include "io.hpp"

Ns16550a::Ns16550a(uint64_t dev_addr) : base_addr_(dev_addr) {
  // disable interrupt
  io::Out<uint8_t>(base_addr_ + kRegIER, 0x00);
  // set baud rate
  io::Out<uint8_t>(base_addr_ + kRegLCR, 0x80);
  io::Out<uint8_t>(base_addr_ + kUartDLL, 0x03);
  io::Out<uint8_t>(base_addr_ + kUartDLM, 0x00);
  // set word length to 8-bits
  io::Out<uint8_t>(base_addr_ + kRegLCR, 0x03);
  // enable FIFOs
  io::Out<uint8_t>(base_addr_ + kRegFCR, 0x07);
  // enable receiver interrupts
  io::Out<uint8_t>(base_addr_ + kRegIER, 0x01);
}

void Ns16550a::PutChar(uint8_t c) const {
  while ((io::In<uint8_t>(base_addr_ + kRegLSR) & (1 << 5)) == 0) {
    ;
  }
  io::Out<uint8_t>(base_addr_ + kRegTHR, c);
}
