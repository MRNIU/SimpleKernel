
/**
 * @file interrupt.h
 * @brief interrupt 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_

#include <cpu_io.h>

#include <cstdint>

#include "gic.h"
#include "interrupt_base.h"
#include "singleton.hpp"
#include "sk_stdio.h"

class Interrupt final : public InterruptBase {
 public:
  static constexpr const size_t kMaxInterrupt = 128;

  Interrupt();

  /// @name 构造/析构函数
  /// @{
  Interrupt(const Interrupt &) = delete;
  Interrupt(Interrupt &&) = delete;
  auto operator=(const Interrupt &) -> Interrupt & = delete;
  auto operator=(Interrupt &&) -> Interrupt & = delete;
  ~Interrupt() = default;
  /// @}

  /**
   * @brief 执行中断处理
   * @param  cause 中断或异常号
   * @param  context 中断上下文
   */
  void Do(uint64_t cause, uint8_t *context) override;

  /**
   * @brief 注册中断处理函数
   * @param cause 中断原因
   * @param func 处理函数
   */
  void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) override;

  __always_inline void SPI(uint32_t intid) const { gic_.SPI(intid); }
  __always_inline void PPI(uint32_t intid, uint32_t cpuid) const {
    gic_.PPI(intid, cpuid);
  }

 private:
  static std::array<InterruptFunc, kMaxInterrupt> interrupt_handlers;

  Gic gic_;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_ */
