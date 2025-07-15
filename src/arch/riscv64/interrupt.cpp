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

#include <cpu_io.h>

#include "basic_info.hpp"
#include "sk_cstdio"
#include "sk_iostream"

alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kInterruptMaxCount> Interrupt::interrupt_handlers_;
/// 异常处理函数数组
alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kExceptionMaxCount> Interrupt::exception_handlers_;

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto &i : interrupt_handlers_) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Info("Default Interrupt handler [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                     [cause],
                 cause, context);
      return 0;
    };
  }
  // 注册默认异常处理函数
  for (auto &i : exception_handlers_) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Err("Default Exception handler [%s] 0x%X, 0x%p\n",
                cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                    [cause],
                cause, context);
      while (1)
        ;
      return 0;
    };
  }
  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t *context) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    // 中断
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kInterruptMaxCount) {
      interrupt_handlers_[exception_code](exception_code, context);
    }
  } else {
    // 异常
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kExceptionMaxCount) {
      exception_handlers_[exception_code](exception_code, context);
    }
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kInterruptMaxCount) {
      interrupt_handlers_[exception_code] = func;
      klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                     [exception_code],
                 cause, func);
    }
  } else {
    if (exception_code <
        cpu_io::detail::register_info::csr::ScauseInfo::kExceptionMaxCount) {
      exception_handlers_[exception_code] = func;
      klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                     [exception_code],
                 cause, func);
    }
  }
}
