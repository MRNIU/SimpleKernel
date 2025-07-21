/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include <cpu_io.h>

#include "arch.h"
#include "interrupt.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"

void InterruptInit(int, const char **) {
  // 初始化中断
  Singleton<Interrupt>::GetInstance();

  // 注册时钟中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::IdtrInfo::kIrq0,
      [](uint64_t exception_code, uint8_t *) -> uint64_t {
        Singleton<Interrupt>::GetInstance().pit_.Ticks();
        if (Singleton<Interrupt>::GetInstance().pit_.GetTicks() % 100 == 0) {
          klog::Info("Handle %d %s\n", exception_code,
                     cpu_io::detail::register_info::IdtrInfo::kInterruptNames
                         [exception_code]);
        }
        Singleton<Interrupt>::GetInstance().pic_.Clear(exception_code);
        return 0;
      });

  // 允许时钟中断
  Singleton<Interrupt>::GetInstance().pic_.Enable(
      cpu_io::detail::register_info::IdtrInfo::kIrq0);
  // 开启中断
  cpu_io::Rflags::If::Set();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char **) {}
