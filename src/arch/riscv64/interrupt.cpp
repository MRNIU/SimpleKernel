/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include "interrupt.h"

#include <cpu_io.h>
#include <opensbi_interface.h>

#include <cassert>

#include "basic_info.hpp"
#include "kernel.h"
#include "kstd_cstdio"
#include "kstd_iostream"

alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kInterruptMaxCount> Interrupt::interrupt_handlers_;
/// 异常处理函数数组
alignas(4) std::array<Interrupt::InterruptFunc,
                      cpu_io::detail::register_info::csr::ScauseInfo::
                          kExceptionMaxCount> Interrupt::exception_handlers_;

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto& i : interrupt_handlers_) {
    i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
      klog::Info("Default Interrupt handler [%s] 0x%X, 0x%p\n",
                 cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                     [cause],
                 cause, context);
      return 0;
    };
  }
  // 注册默认异常处理函数
  for (auto& i : exception_handlers_) {
    i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
      klog::Err("Default Exception handler [%s] 0x%X, 0x%p\n",
                cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                    [cause],
                cause, context);
      while (true) {
        cpu_io::Pause();
      }
    };
  }
  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, cpu_io::TrapContext* context) {
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
    assert(exception_code < cpu_io::detail::register_info::csr::ScauseInfo::
                                kInterruptMaxCount &&
           "Interrupt code out of range");

    interrupt_handlers_[exception_code] = func;
    klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
               cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                   [exception_code],
               cause, func);
  } else {
    assert(exception_code < cpu_io::detail::register_info::csr::ScauseInfo::
                                kExceptionMaxCount &&
           "Exception code out of range");

    exception_handlers_[exception_code] = func;
    klog::Info("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
               cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                   [exception_code],
               cause, func);
  }
}

auto Interrupt::SendIpi(uint64_t target_cpu_mask) -> Expected<void> {
  if (target_cpu_mask > (1UL << SIMPLEKERNEL_MAX_CORE_COUNT) - 1) {
    return std::unexpected(Error(ErrorCode::kIpiTargetOutOfRange));
  }

  auto ret = sbi_send_ipi(target_cpu_mask, 0);
  if (ret.error != SBI_SUCCESS) {
    return std::unexpected(Error(ErrorCode::kIpiSendFailed));
  }
  return {};
}

auto Interrupt::BroadcastIpi() -> Expected<void> {
  // 如果没有其他核心，直接返回成功
  if (BasicInfoSingleton::instance().core_count == 1) {
    return {};
  }

  uint64_t mask = 0;
  auto current = cpu_io::GetCurrentCoreId();
  for (size_t i = 0; i < BasicInfoSingleton::instance().core_count; ++i) {
    if (i != current) {
      mask |= (1UL << i);
    }
  }

  return SendIpi(mask);
}

auto Interrupt::RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                          uint32_t priority,
                                          InterruptFunc handler)
    -> Expected<void> {
  if (irq >= Plic::kInterruptMaxCount) {
    return std::unexpected(Error(ErrorCode::kIrqChipInvalidIrq));
  }

  // 先注册处理函数
  PlicSingleton::instance().RegisterInterruptFunc(static_cast<uint8_t>(irq),
                                                  handler);

  // 再在 PLIC 上为指定核心启用该中断
  PlicSingleton::instance().Set(cpu_id, irq, priority, true);

  klog::Info("RegisterExternalInterrupt: IRQ %u, cpu %u, priority %u\n", irq,
             cpu_id, priority);
  return {};
}
