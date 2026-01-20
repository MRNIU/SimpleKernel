/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "io.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "pl011.h"
#include "sk_cstdio"

/**
 * @brief 中断处理函数
 * 由于 aarch64-linux-gnu-g++ 13.2.0 不支持 aarch64 的
 * __attribute__((interrupt("IRQ"))) 写法，需要手动处理中断函数
 */
extern "C" void vector_table();

// 同步异常处理程序
extern "C" void sync_current_el_sp0_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Sync Exception at Current EL with SP0\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_current_el_sp0_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("IRQ Exception at Current EL with SP0\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_current_el_sp0_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("FIQ Exception at Current EL with SP0\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_sp0_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Error Exception at Current EL with SP0\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_current_el_spx_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Sync Exception at Current EL with SPx\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_current_el_spx_handler(uint64_t sp [[maybe_unused]]) {
  auto cause = cpu_io::ICC_IAR1_EL1::INTID::Get();
  Singleton<Interrupt>::GetInstance().Do(cause, nullptr);
}

extern "C" void fiq_current_el_spx_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("FIQ Exception at Current EL with SPx\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_spx_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Error Exception at Current EL with SPx\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_lower_el_aarch64_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Sync Exception at Lower EL using AArch64\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_lower_el_aarch64_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("IRQ Exception at Lower EL using AArch64\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch64_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("FIQ Exception at Lower EL using AArch64\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch64_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Error Exception at Lower EL using AArch64\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_lower_el_aarch32_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Sync Exception at Lower EL using AArch32\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_lower_el_aarch32_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("IRQ Exception at Lower EL using AArch32\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch32_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("FIQ Exception at Lower EL using AArch32\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch32_handler(uint64_t sp [[maybe_unused]]) {
  klog::Err("Error Exception at Lower EL using AArch32\n");
  while (true) {
    asm volatile("wfi");
  }
}

auto uart_handler(uint64_t cause, uint8_t*) -> uint64_t {
  sk_putchar(Singleton<Pl011>::GetInstance().TryGetChar(), nullptr);
  return cause;
}

void InterruptInit(int, const char**) {
  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  auto uart_intid =
      Singleton<KernelFdt>::GetInstance().GetAarch64Intid("arm,pl011") +
      Gic::kSPIBase;

  klog::Info("uart_intid: %d\n", uart_intid);

  Singleton<Interrupt>::GetInstance().SPI(uart_intid,
                                          cpu_io::GetCurrentCoreId());

  // 注册 uart 中断处理函数
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(uart_intid,
                                                            uart_handler);

  cpu_io::EnableInterrupt();

  // 初始化定时器
  TimerInit();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char**) {
  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  Singleton<Interrupt>::GetInstance().SetUP();

  cpu_io::EnableInterrupt();

  TimerInitSMP();

  klog::Info("Hello InterruptInitSMP\n");
}
