/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstdio"
#include "pl011/pl011_driver.hpp"
#include "pl011_singleton.h"

using InterruptDelegate = InterruptBase::InterruptDelegate;
namespace {
/**
 * @brief 通用异常处理辅助函数
 * @param exception_msg 异常类型描述
 * @param context 中断上下文
 * @param print_regs 要打印的寄存器数量 (0, 4, 或 8)
 */
static void HandleException(const char* exception_msg,
                            cpu_io::TrapContext* context, int print_regs = 0) {
  klog::err << exception_msg;
  klog::err << "  ESR_EL1: " << klog::HEX << context->esr_el1
            << ", ELR_EL1: " << klog::HEX << context->elr_el1
            << ", SP_EL0: " << klog::HEX << context->sp_el0
            << ", SP_EL1: " << klog::HEX << context->sp_el1
            << ", SPSR_EL1: " << klog::HEX << context->spsr_el1;

  if (print_regs == 4) {
    klog::err << "  x0-x3: " << klog::HEX << context->x0 << " " << klog::HEX
              << context->x1 << " " << klog::HEX << context->x2 << " "
              << klog::HEX << context->x3;
  } else if (print_regs == 8) {
    klog::err << "  x0-x7: " << klog::HEX << context->x0 << " " << klog::HEX
              << context->x1 << " " << klog::HEX << context->x2 << " "
              << klog::HEX << context->x3 << " " << klog::HEX << context->x4
              << " " << klog::HEX << context->x5 << " " << klog::HEX
              << context->x6 << " " << klog::HEX << context->x7;
  }

  while (true) {
    cpu_io::Pause();
  }
}
}  // namespace

/**
 * @brief 中断处理函数
 */
extern "C" void vector_table();

// 同步异常处理程序
extern "C" void sync_current_el_sp0_handler(cpu_io::TrapContext* context) {
  HandleException("Sync Exception at Current EL with SP0", context, 4);
}

extern "C" void irq_current_el_sp0_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "IRQ Exception at Current EL with SP0";
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_current_el_sp0_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "FIQ Exception at Current EL with SP0";
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_sp0_handler(cpu_io::TrapContext* context) {
  HandleException("Error Exception at Current EL with SP0", context);
}

extern "C" void sync_current_el_spx_handler(cpu_io::TrapContext* context) {
  HandleException("Sync Exception at Current EL with SPx", context, 4);
}

extern "C" void irq_current_el_spx_handler(cpu_io::TrapContext* context) {
  auto cause = cpu_io::ICC_IAR1_EL1::INTID::Get();
  InterruptSingleton::instance().Do(cause, context);
}

extern "C" void fiq_current_el_spx_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "FIQ Exception at Current EL with SPx";
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_spx_handler(cpu_io::TrapContext* context) {
  HandleException("Error Exception at Current EL with SPx", context);
}

extern "C" void sync_lower_el_aarch64_handler(cpu_io::TrapContext* context) {
  HandleException("Sync Exception at Lower EL using AArch64", context, 8);
}

extern "C" void irq_lower_el_aarch64_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "IRQ Exception at Lower EL using AArch64";
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch64_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "FIQ Exception at Lower EL using AArch64";
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch64_handler(cpu_io::TrapContext* context) {
  HandleException("Error Exception at Lower EL using AArch64", context);
}

extern "C" void sync_lower_el_aarch32_handler(cpu_io::TrapContext* context) {
  HandleException("Sync Exception at Lower EL using AArch32", context);
}

extern "C" void irq_lower_el_aarch32_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "IRQ Exception at Lower EL using AArch32";
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch32_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::err << "FIQ Exception at Lower EL using AArch32";
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch32_handler(cpu_io::TrapContext* context) {
  HandleException("Error Exception at Lower EL using AArch32", context);
}

auto uart_handler(uint64_t cause, cpu_io::TrapContext*) -> uint64_t {
  Pl011Singleton::instance().HandleInterrupt(
      [](uint8_t ch) { etl_putchar(ch); });
  return cause;
}

void InterruptInit(int, const char**) {
  InterruptSingleton::create();

  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  auto uart_intid =
      KernelFdtSingleton::instance().GetAarch64Intid("arm,pl011").value() +
      Gic::kSPIBase;

  klog::info << "uart_intid: " << uart_intid;

  // 通过统一接口注册 UART 外部中断（先注册 handler，再启用 GIC SPI）
  InterruptSingleton::instance()
      .RegisterExternalInterrupt(uart_intid, cpu_io::GetCurrentCoreId(), 0,
                                 InterruptDelegate::create<uart_handler>())
      .or_else([](Error err) -> Expected<void> {
        klog::err << "Failed to register UART IRQ: " << err.message();
        return std::unexpected(err);
      });

  cpu_io::EnableInterrupt();

  // 初始化定时器
  TimerInit();

  klog::info << "Hello InterruptInit";
}

void InterruptInitSMP(int, const char**) {
  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  InterruptSingleton::instance().SetUP();

  cpu_io::EnableInterrupt();

  TimerInitSMP();

  klog::info << "Hello InterruptInitSMP";
}
