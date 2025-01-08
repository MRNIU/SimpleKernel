
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

Pl011::Pl011(uintptr_t dev_addr) : base_addr_(dev_addr) {
  // disable interrupt
  Write(kRegIER, 0x00);
  // set baud rate
  Write(kRegLCR, 0x80);
  Write(kUartDLL, 0x03);
  Write(kUartDLM, 0x00);
  // set word length to 8-bits
  Write(kRegLCR, 0x03);
  // enable FIFOs
  Write(kRegFCR, 0x07);
  // enable receiver interrupts
  Write(kRegIER, 0x01);
}

void Pl011::PutChar(uint8_t c) {
  while ((Read(kRegLSR) & (1 << 5)) == 0)
    ;
  Write(kRegTHR, c);
}

volatile uint8_t* Pl011::Reg(uint8_t reg) {
  return (volatile uint8_t*)(base_addr_ + reg);
}

uint8_t Pl011::Read(uint8_t reg) { return (*(Reg(reg))); }

void Pl011::Write(uint8_t reg, uint8_t c) { (*Reg(reg)) = c; }
