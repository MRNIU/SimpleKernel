/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "ns16550a.h"

#include <cpu_io.h>

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
    cpu_io::Pause();
  }
  io::Out<uint8_t>(base_addr_ + kRegTHR, c);
}

auto Ns16550a::GetChar() const -> uint8_t {
  // 等待直到接收缓冲区有数据 (LSR bit 0 = 1)
  while ((io::In<uint8_t>(base_addr_ + kRegLSR) & (1 << 0)) == 0) {
    cpu_io::Pause();
  }
  return io::In<uint8_t>(base_addr_ + kRegRHR);
}

auto Ns16550a::TryGetChar() const -> std::optional<uint8_t> {
  // 检查接收缓冲区是否有数据 (LSR bit 0 = 1)
  if ((io::In<uint8_t>(base_addr_ + kRegLSR) & (1 << 0)) != 0) {
    return io::In<uint8_t>(base_addr_ + kRegRHR);
  }
  return std::nullopt;
}

auto Ns16550a::HasData() const -> bool {
  return (io::In<uint8_t>(base_addr_ + kRegLSR) & (1 << 0)) != 0;
}
