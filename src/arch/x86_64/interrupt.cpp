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

#include "arch.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"

std::array<Interrupt::InterruptFunc,
           cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount>
    Interrupt::interrupt_handlers;

std::array<cpu_io::detail::register_info::IdtrInfo::Idt,
           cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount>
    Interrupt::idts;

/**
 * @brief 中断处理函数
 * @tparam no 中断号
 * @param interrupt_context 中断上下文，根据中断不同可能是 InterruptContext 或
 * InterruptContextErrorCode
 */
template <uint8_t no>
__attribute__((target("general-regs-only")))
__attribute__((interrupt)) static void
TarpEntry(uint8_t *interrupt_context) {
  Singleton<Interrupt>::GetInstance().Do(no, interrupt_context);
}

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
            sizeof(cpu_io::detail::register_info::IdtrInfo::Idtr) *
                cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount -
            1,
        .base = idts.data(),
    };
    cpu_io::Idtr::Write(idtr);

    // 输出 idtr 信息
    // sk_std::cout << cpu::kAllCr.idtr << sk_std::endl;
    for (size_t i = 0;
         i < (cpu_io::Idtr::Read().limit + 1) /
                 sizeof(cpu_io::detail::register_info::IdtrInfo::Idtr);
         i++) {
      klog::Debug("idtr[%d] 0x%p\n", i, cpu_io::Idtr::Read().base + i);
      // klog::debug << *(cpu::kAllCr.idtr.Read().base + i) << sk_std::endl;
    }
  }
}

Interrupt::Interrupt()
    : pic_(cpu_io::detail::register_info::IdtrInfo::kIrq0,
           cpu_io::detail::register_info::IdtrInfo::kIrq8),
      pit_(200) {
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

  // 初始化 loacl apic

  // 初始化 io apic

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

auto InterruptInit(int, const char **) -> int {
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

  return 0;
}

auto InterruptInitSMP(int, const char **) -> int {
  klog::Info("Hello InterruptInitSMP\n");

  return 0;
}
