/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <device_framework/pl011.hpp>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_cstdio"

namespace {
/**
 * @brief 通用异常处理辅助函数
 * @param exception_msg 异常类型描述
 * @param context 中断上下文
 * @param print_regs 要打印的寄存器数量 (0, 4, 或 8)
 */
static void HandleException(const char* exception_msg,
                            cpu_io::TrapContext* context, int print_regs = 0) {
  klog::Err("%s\n", exception_msg);
  klog::Err(
      "  ESR_EL1: 0x%016lX, ELR_EL1: 0x%016lX, SP_EL0: 0x%016lX, SP_EL1: "
      "0x%016lX, SPSR_EL1: 0x%016lX\n",
      context->esr_el1, context->elr_el1, context->sp_el0, context->sp_el1,
      context->spsr_el1);

  if (print_regs == 4) {
    klog::Err("  x0-x3: 0x%016lX 0x%016lX 0x%016lX 0x%016lX\n", context->x0,
              context->x1, context->x2, context->x3);
  } else if (print_regs == 8) {
    klog::Err(
        "  x0-x7: 0x%016lX 0x%016lX 0x%016lX 0x%016lX 0x%016lX 0x%016lX "
        "0x%016lX 0x%016lX\n",
        context->x0, context->x1, context->x2, context->x3, context->x4,
        context->x5, context->x6, context->x7);
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
  klog::Err("IRQ Exception at Current EL with SP0\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_current_el_sp0_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::Err("FIQ Exception at Current EL with SP0\n");
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
  klog::Err("FIQ Exception at Current EL with SPx\n");
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
  klog::Err("IRQ Exception at Lower EL using AArch64\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch64_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::Err("FIQ Exception at Lower EL using AArch64\n");
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
  klog::Err("IRQ Exception at Lower EL using AArch32\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch32_handler(
    [[maybe_unused]] cpu_io::TrapContext* context) {
  klog::Err("FIQ Exception at Lower EL using AArch32\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch32_handler(cpu_io::TrapContext* context) {
  HandleException("Error Exception at Lower EL using AArch32", context);
}

auto uart_handler(uint64_t cause, cpu_io::TrapContext*) -> uint64_t {
  Pl011Singleton::instance().HandleInterrupt(
      [](uint8_t ch) { sk_putchar(ch, nullptr); });
  return cause;
}

void InterruptInit(int, const char**) {
  InterruptSingleton::create();

  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  auto uart_intid =
      KernelFdtSingleton::instance().GetAarch64Intid("arm,pl011").value() +
      Gic::kSPIBase;

  klog::Info("uart_intid: %d\n", uart_intid);

  // 通过统一接口注册 UART 外部中断（先注册 handler，再启用 GIC SPI）
  InterruptSingleton::instance()
      .RegisterExternalInterrupt(uart_intid, cpu_io::GetCurrentCoreId(), 0,
                                 uart_handler)
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to register UART IRQ: %s\n", err.message());
        return std::unexpected(err);
      });

  cpu_io::EnableInterrupt();

  // 初始化定时器
  TimerInit();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char**) {
  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  InterruptSingleton::instance().SetUP();

  cpu_io::EnableInterrupt();

  TimerInitSMP();

  klog::Info("Hello InterruptInitSMP\n");
}
