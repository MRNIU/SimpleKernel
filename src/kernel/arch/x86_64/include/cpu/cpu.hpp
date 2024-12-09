
/**
 * @file cpu.hpp
 * @brief x86_64 cpu 相关定义
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2024-03-05
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2024-03-05<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_CPU_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_CPU_HPP_

#include <cstdint>
#include <cstdlib>
#include <type_traits>
#include <typeinfo>

#include "kernel_log.hpp"
#include "regs.hpp"
#include "sk_cstdio"
#include "sk_iostream"

/**
 * x86_64 cpu 相关定义
 * @note 寄存器读写设计见 arch/README.md
 */
namespace cpu {
/**
 * @brief  从端口读数据
 * @tparam T              要读的数据类型
 * @param  port           要读的端口
 * @return uint8_t        读取到的数据
 */
template <class T>
static __always_inline auto In(const uint32_t port) -> T {
  T data;
  if constexpr (std::is_same<T, uint8_t>::value) {
    __asm__ volatile("inb %1, %0" : "=a"(data) : "dN"(port));
  } else if constexpr (std::is_same<T, uint16_t>::value) {
    __asm__ volatile("inw %1, %0" : "=a"(data) : "dN"(port));
  } else if constexpr (std::is_same<T, uint32_t>::value) {
    __asm__ volatile("inl %1, %0" : "=a"(data) : "dN"(port));
  } else {
    klog::Err("No Type\n");
    throw;
  }
  return data;
}

/**
 * @brief  向端口写数据
 * @tparam T              要写的数据类型
 * @param  port           要写的端口
 * @param  data           要写的数据
 */
template <class T>
static __always_inline void Out(const uint32_t port, const T data) {
  if constexpr (std::is_same<T, uint8_t>::value) {
    __asm__ volatile("outb %1, %0" : : "dN"(port), "a"(data));
  } else if constexpr (std::is_same<T, uint16_t>::value) {
    __asm__ volatile("outw %1, %0" : : "dN"(port), "a"(data));
  } else if constexpr (std::is_same<T, uint32_t>::value) {
    __asm__ volatile("outl %1, %0" : : "dN"(port), "a"(data));
  } else {
    klog::Err("No Type\n");
    throw;
  }
}

/// @name 端口
static constexpr const uint32_t kCom1 = 0x3F8;
/**
 * 串口定义
 */
class Serial {
 public:
  explicit Serial(uint32_t port) : port_(port) {
    // Disable all interrupts
    Out<uint8_t>(port_ + 1, 0x00);
    // Enable DLAB (set baud rate divisor)
    Out<uint8_t>(port_ + 3, 0x80);
    // Set divisor to 3 (lo byte) 38400 baud
    Out<uint8_t>(port_ + 0, 0x03);
    // (hi byte)
    Out<uint8_t>(port_ + 1, 0x00);
    // 8 bits, no parity, one stop bit
    Out<uint8_t>(port_ + 3, 0x03);
    // Enable FIFO, clear them, with 14-byte threshold
    Out<uint8_t>(port_ + 2, 0xC7);
    // IRQs enabled, RTS/DSR set
    Out<uint8_t>(port_ + 4, 0x0B);
    // Set in loopback mode, test the serial chip
    Out<uint8_t>(port_ + 4, 0x1E);
    // Test serial chip (send byte 0xAE and check if serial returns same byte)
    Out<uint8_t>(port_ + 0, 0xAE);
    // Check if serial is faulty (i.e: not same byte as sent)
    if (In<uint8_t>(port_ + 0) != 0xAE) {
      asm("hlt");
    }

    // If serial is not faulty set it in normal operation mode (not-loopback
    // with IRQs enabled and OUT#1 and OUT#2 bits enabled)
    Out<uint8_t>(port_ + 4, 0x0F);
  }

  ~Serial() = default;

  /// @name 不使用的构造函数
  /// @{
  Serial() = delete;
  Serial(const Serial &) = delete;
  Serial(Serial &&) = delete;
  auto operator=(const Serial &) -> Serial & = delete;
  auto operator=(Serial &&) -> Serial & = delete;
  /// @}

  /**
   * @brief  读一个字节
   * @return uint8_t         读取到的数据
   */
  [[nodiscard]] auto Read() const -> uint8_t {
    while (!SerialReceived()) {
      ;
    }
    return In<uint8_t>(port_);
  }

  /**
   * @brief  写一个字节
   * @param  c              要写的数据
   */
  void Write(uint8_t c) const {
    while (!IsTransmitEmpty()) {
      ;
    }
    Out<uint8_t>(port_, c);
  }

 private:
  uint32_t port_;

  /**
   * @brief 串口是否接收到数据
   * @return true
   * @return false
   */
  [[nodiscard]] auto SerialReceived() const -> bool {
    return bool(In<uint8_t>(port_ + 5) & 1);
  }

  /**
   * @brief 串口是否可以发送数据
   * @return true
   * @return false
   */
  [[nodiscard]] auto IsTransmitEmpty() const -> bool {
    return bool((In<uint8_t>(port_ + 5) & 0x20) != 0);
  }
};
};  // namespace cpu

#endif  // SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INCLUDE_CPU_CPU_HPP_
