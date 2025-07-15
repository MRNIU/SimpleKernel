/**
 * @file interrupt.cpp
 * @brief 中断初始化
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

#include "interrupt.h"

#include "io.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"

std::array<Interrupt::InterruptFunc, Interrupt::kMaxInterrupt>
    Interrupt::interrupt_handlers;

Interrupt::Interrupt() {
  auto [gicd_base_addr, gicr_base_addr] =
      Singleton<KernelFdt>::GetInstance().GetGic();
  gic_ = std::move(Gic(gicd_base_addr, gicr_base_addr));

  // 注册默认中断处理函数
  for (auto &i : interrupt_handlers) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Info("Default Interrupt handler 0x%X, 0x%p\n", cause, context);
      return 0;
    };
  }

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t *context) {
  auto ret = interrupt_handlers[cause](cause, context);

  if (ret) {
    cpu_io::ICC_EOIR1_EL1::Write(cause);
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (func) {
    interrupt_handlers[cause] = func;
  }
}
