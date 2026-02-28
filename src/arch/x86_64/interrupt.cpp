/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include "interrupt.h"

#include <cpu_io.h>

#include "arch.h"
#include "kernel.h"
#include "kernel_log.hpp"
#include "kstd_cstdio"
#include "kstd_iostream"

namespace {
/**
 * @brief 中断处理函数
 * @tparam no 中断号
 * @param interrupt_context 中断上下文，根据中断不同可能是 InterruptContext 或
 * InterruptContextErrorCode
 */
template <uint8_t no>
__attribute__((target("general-regs-only"))) __attribute__((interrupt)) void
TarpEntry(cpu_io::TrapContext* interrupt_context) {
  InterruptSingleton::instance().Do(no, interrupt_context);
}

};  // namespace

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto& i : interrupt_handlers_) {
    i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
      klog::Info(
          "Default Interrupt handler [%s] 0x%X, 0x%p\n",
          cpu_io::detail::register_info::IdtrInfo::kInterruptNames[cause],
          cause, context);
      DumpStack();
      while (true) {
        ;
      }
    };
  }

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, cpu_io::TrapContext* context) {
  if (cause < cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount) {
    interrupt_handlers_[cause](cause, context);
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (cause < cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount) {
    interrupt_handlers_[cause] = func;
    klog::Debug("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                cpu_io::detail::register_info::IdtrInfo::kInterruptNames[cause],
                cause, func);
  }
}

void Interrupt::SetUpIdtr() { SetUpIdtr<0>(); }

template <uint8_t no>
void Interrupt::SetUpIdtr() {
  if constexpr (no <
                cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount -
                    1) {
    idts_[no] = cpu_io::detail::register_info::IdtrInfo::Idt(
        reinterpret_cast<uint64_t>(TarpEntry<no>), 8, 0x0,
        cpu_io::detail::register_info::IdtrInfo::Idt::Type::k64BitInterruptGate,
        cpu_io::detail::register_info::IdtrInfo::Idt::DPL::kRing0,
        cpu_io::detail::register_info::IdtrInfo::Idt::P::kPresent);
    SetUpIdtr<no + 1>();
  } else {
    // 写入 idtr
    static auto idtr = cpu_io::detail::register_info::IdtrInfo::Idtr{
        .limit =
            sizeof(cpu_io::detail::register_info::IdtrInfo::Idt) *
                cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount -
            1,
        .base = idts_.data(),
    };
    cpu_io::Idtr::Write(idtr);

    // 输出 idtr 信息
    for (size_t i = 0;
         i < (cpu_io::Idtr::Read().limit + 1) /
                 sizeof(cpu_io::detail::register_info::IdtrInfo::Idtr);
         i++) {
      klog::Debug("idtr[%d] 0x%p\n", i, cpu_io::Idtr::Read().base + i);
    }
  }
}

auto Interrupt::SendIpi(uint64_t target_cpu_mask) -> Expected<void> {
  /// @todo
  return std::unexpected(Error(ErrorCode::kIpiSendFailed));
}

auto Interrupt::BroadcastIpi() -> Expected<void> {
  /// @todo
  return std::unexpected(Error(ErrorCode::kIpiSendFailed));
}

auto Interrupt::RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                          uint32_t priority,
                                          InterruptFunc handler)
    -> Expected<void> {
  // 计算 IDT 向量号：kExternalVectorBase + irq
  auto vector = static_cast<uint64_t>(kExternalVectorBase + irq);
  if (vector >= cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount) {
    return std::unexpected(Error(ErrorCode::kApicInvalidIrq));
  }

  // 先注册处理函数
  RegisterInterruptFunc(vector, handler);

  // 再在 IO APIC 上启用 IRQ 重定向到指定核心
  // 注: x86 APIC 优先级由向量号隐含决定，priority 参数不直接使用
  auto result = apic_.SetIrqRedirection(
      static_cast<uint8_t>(irq), static_cast<uint8_t>(vector), cpu_id, false);
  if (!result.has_value()) {
    return std::unexpected(result.error());
  }

  klog::Info("RegisterExternalInterrupt: IRQ %u -> vector 0x%X, cpu %u\n", irq,
             static_cast<uint32_t>(vector), cpu_id);
  return {};
}

void Interrupt::InitApic(size_t cpu_count) { apic_ = Apic(cpu_count); }
