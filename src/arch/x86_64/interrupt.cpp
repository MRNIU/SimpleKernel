/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include "interrupt.h"

#include <cpu_io.h>

#include "arch.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"

namespace {
/**
 * @brief 中断处理函数
 * @tparam no 中断号
 * @param interrupt_context 中断上下文，根据中断不同可能是 InterruptContext 或
 * InterruptContextErrorCode
 */
template <uint8_t no>
__attribute__((target("general-regs-only"))) __attribute__((interrupt)) void
TarpEntry(uint8_t *interrupt_context) {
  Singleton<Interrupt>::GetInstance().Do(no, interrupt_context);
}

/**
 * @brief APIC 时钟中断处理函数
 * @param cause 中断原因
 * @param context 中断上下文
 * @return uint64_t 返回值
 */
uint64_t ApicTimerHandler(uint64_t cause, uint8_t *context) {
  // APIC 时钟中断处理
  static uint64_t tick_count = 0;
  tick_count++;

  // 每100次中断打印一次信息（减少日志输出）
  if (tick_count % 100 == 0) {
    klog::Info("APIC Timer interrupt %lu, vector 0x%X\n", tick_count,
               static_cast<uint32_t>(cause));
  }

  // 发送 EOI 信号给 Local APIC
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().SendEoi();
  return 0;
}
};  // namespace

alignas(4096) std::array<Interrupt::InterruptFunc,
                         cpu_io::detail::register_info::IdtrInfo::
                             kInterruptMaxCount> Interrupt::interrupt_handlers;

alignas(4096) std::array<cpu_io::detail::register_info::IdtrInfo::Idt,
                         cpu_io::detail::register_info::IdtrInfo::
                             kInterruptMaxCount> Interrupt::idts;

template <uint8_t no>
void Interrupt::SetUpIdtr() {
  if constexpr (no <
                cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount -
                    1) {
    idts[no] = cpu_io::detail::register_info::IdtrInfo::Idt(
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
        .base = idts.data(),
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

Interrupt::Interrupt() {
  // 注册默认中断处理函数
  for (auto &i : interrupt_handlers) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
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

  // 初始化 idtr
  SetUpIdtr();

  // 初始化当前 CPU 的 Local APIC
  if (Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic()) {
    klog::Info("Local APIC initialized successfully\n");
  } else {
    klog::Err("Failed to initialize Local APIC\n");
  }

  // 初始化 IO APIC (暂时保留注释)
  // TODO: 实现 IO APIC 支持

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t *context) {
  if (cause < cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount) {
    interrupt_handlers[cause](cause, context);
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (cause < cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount) {
    interrupt_handlers[cause] = func;
    klog::Debug("RegisterInterruptFunc [%s] 0x%X, 0x%p\n",
                cpu_io::detail::register_info::IdtrInfo::kInterruptNames[cause],
                cause, func);
  }
}

void Interrupt::EnableApicTimer(uint32_t frequency_hz, uint8_t vector) {
  // 注册中断处理函数
  RegisterInterruptFunc(vector, ApicTimerHandler);

  // 启用 Local APIC 定时器
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().SetupPeriodicTimer(
      frequency_hz, vector);
  klog::Info("APIC Timer enabled: %u Hz, vector 0x%X\n", frequency_hz, vector);
}

void Interrupt::DisableApicTimer() {
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().DisableTimer();
  klog::Info("APIC Timer disabled\n");
}
