
/**
 * @file interrupt.h
 * @brief 中断处理
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

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_

#include <cpu_io.h>

#include <array>
#include <cstdint>

#include "interrupt_base.h"
#include "singleton.hpp"
#include "sk_stdio.h"

class Interrupt final : public InterruptBase {
 public:
  cpu_io::Pic pic_;
  cpu_io::Pit pit_;

  Interrupt();

  /// @name 构造/析构函数
  /// @{
  Interrupt(const Interrupt &) = delete;
  Interrupt(Interrupt &&) = delete;
  auto operator=(const Interrupt &) -> Interrupt & = delete;
  auto operator=(Interrupt &&) -> Interrupt & = delete;
  ~Interrupt() override = default;
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

 private:
  /// 中断处理函数数组
  static std::array<InterruptFunc,
                    cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount>
      interrupt_handlers;

  static std::array<cpu_io::detail::register_info::IdtrInfo::Idt,
                    cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount>
      idts;

  /**
   * @brief 初始化 idtr
   */
  template <uint8_t no = 0>
  void SetUpIdtr();
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_ */
