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

namespace {
auto DefaultInterruptHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::info << "Default Interrupt handler ["
             << cpu_io::ScauseInfo::kInterruptNames[cause] << "] " << klog::HEX
             << cause << ", " << klog::hex
             << reinterpret_cast<uintptr_t>(context);
  return 0;
}

auto DefaultExceptionHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::err << "Default Exception handler ["
            << cpu_io::ScauseInfo::kExceptionNames[cause] << "] " << klog::HEX
            << cause << ", " << klog::hex
            << reinterpret_cast<uintptr_t>(context);
  while (true) {
    cpu_io::Pause();
  }
}
}  // namespace

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto& i : interrupt_handlers_) {
    i = InterruptDelegate::create<DefaultInterruptHandler>();
  }
  // 注册默认异常处理函数
  for (auto& i : exception_handlers_) {
    i = InterruptDelegate::create<DefaultExceptionHandler>();
  }
  klog::info << "Interrupt init.";
}

void Interrupt::Do(uint64_t cause, cpu_io::TrapContext* context) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    // 中断
    if (exception_code < cpu_io::ScauseInfo::kInterruptMaxCount) {
      interrupt_handlers_[exception_code](exception_code, context);
    }
  } else {
    // 异常
    if (exception_code < cpu_io::ScauseInfo::kExceptionMaxCount) {
      exception_handlers_[exception_code](exception_code, context);
    }
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptDelegate func) {
  auto interrupt = cpu_io::Scause::Interrupt::Get(cause);
  auto exception_code = cpu_io::Scause::ExceptionCode::Get(cause);

  if (interrupt) {
    assert(exception_code < cpu_io::ScauseInfo::kInterruptMaxCount &&
           "Interrupt code out of range");

    interrupt_handlers_[exception_code] = func;
    klog::info << "RegisterInterruptFunc ["
               << cpu_io::ScauseInfo::kInterruptNames[exception_code] << "] "
               << klog::HEX << cause;
  } else {
    assert(exception_code < cpu_io::ScauseInfo::kExceptionMaxCount &&
           "Exception code out of range");

    exception_handlers_[exception_code] = func;
    klog::info << "RegisterInterruptFunc ["
               << cpu_io::ScauseInfo::kExceptionNames[exception_code] << "] "
               << klog::HEX << cause;
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
                                          InterruptDelegate handler)
    -> Expected<void> {
  if (irq >= Plic::kInterruptMaxCount) {
    return std::unexpected(Error(ErrorCode::kIrqChipInvalidIrq));
  }

  // 先注册处理函数
  plic_.RegisterInterruptFunc(static_cast<uint8_t>(irq), handler);

  // 再在 PLIC 上为指定核心启用该中断
  plic_.Set(cpu_id, irq, priority, true);

  klog::info << "RegisterExternalInterrupt: IRQ " << irq << ", cpu " << cpu_id
             << ", priority " << priority;
  return {};
}

void Interrupt::InitPlic(uint64_t dev_addr, size_t ndev, size_t context_count) {
  plic_ = Plic(dev_addr, ndev, context_count);
}
