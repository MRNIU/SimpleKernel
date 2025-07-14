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
extern "C" void sync_current_el_sp0_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Sync Exception at Current EL with SP0\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_current_el_sp0_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("IRQ Exception at Current EL with SP0\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_current_el_sp0_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("FIQ Exception at Current EL with SP0\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_sp0_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Error Exception at Current EL with SP0\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_current_el_spx_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Sync Exception at Current EL with SPx\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_current_el_spx_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("IRQ Exception at Current EL with SPx\n");
  auto cause = cpu_io::ICC_IAR1_EL1::INTID::Get();
  Singleton<Interrupt>::GetInstance().Do(cause, nullptr);
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_current_el_spx_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("FIQ Exception at Current EL with SPx\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_current_el_spx_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Error Exception at Current EL with SPx\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_lower_el_aarch64_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Sync Exception at Lower EL using AArch64\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_lower_el_aarch64_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("IRQ Exception at Lower EL using AArch64\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch64_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("FIQ Exception at Lower EL using AArch64\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch64_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Error Exception at Lower EL using AArch64\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void sync_lower_el_aarch32_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Sync Exception at Lower EL using AArch32\n");
  while (true) {
    asm volatile("wfi");
  }
}

extern "C" void irq_lower_el_aarch32_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("IRQ Exception at Lower EL using AArch32\n");
  // 处理 IRQ 中断
  // ...
}

extern "C" void fiq_lower_el_aarch32_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("FIQ Exception at Lower EL using AArch32\n");
  // 处理 FIQ 中断
  // ...
}

extern "C" void error_lower_el_aarch32_handler(
    uint64_t sp __attribute_maybe_unused__) {
  klog::Err("Error Exception at Lower EL using AArch32\n");
  while (true) {
    asm volatile("wfi");
  }
}

auto timer_handler(uint64_t cause, uint8_t *) -> uint64_t {
  klog::Info("Timer interrupt\n");
  // 2s
  uint64_t interval_clk = 2 * cpu_io::CNTFRQ_EL0::Read();
  cpu_io::CNTV_TVAL_EL0::Write(interval_clk);
  return cause;
}

auto uart_handler(uint64_t cause, uint8_t *) -> uint64_t {
  klog::Info("%c", Singleton<Pl011>::GetInstance().GetChar());
  return cause;
}

std::array<Interrupt::InterruptFunc, Interrupt::kMaxInterrupt>
    Interrupt::interrupt_handlers;

Interrupt::Interrupt() {
  auto [gicd_base_addr, gicr_base_addr] =
      Singleton<KernelFdt>::GetInstance().GetGic();
  gic_ = std::move(Gic(gicd_base_addr, gicr_base_addr));

  // 注册默认中断处理函数
  for (auto &i : interrupt_handlers) {
    i = [](uint64_t cause, uint8_t *context) -> uint64_t {
      klog::Info("Default Interrupt handler 0x%X, 0x%p\n", cause, context);
      return 0;
    };
  }

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t *context) {
  auto ret = interrupt_handlers[cause](cause, context);

  if (ret) {
    cpu_io::ICC_EOIR1_EL1::Write(cause);
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (func) {
    interrupt_handlers[cause] = func;
  }
}

auto InterruptInit(int, const char **) -> int {
  klog::Info("Hello InterruptInit\n");

  cpu_io::VBAR_EL1::Write(reinterpret_cast<uint64_t>(vector_table));

  auto timer_intid =
      Singleton<KernelFdt>::GetInstance().GetAarch64Intid("arm,armv8-timer") +
      Gic::kPPIBase;

  auto uart_intid =
      Singleton<KernelFdt>::GetInstance().GetAarch64Intid("arm,pl011") +
      Gic::kSPIBase;

  klog::Info("timer_intid: %d, uart_intid: %d\n", timer_intid, uart_intid);

  Singleton<Interrupt>::GetInstance().SPI(uart_intid);
  Singleton<Interrupt>::GetInstance().PPI(timer_intid, 0);

  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(timer_intid,
                                                            timer_handler);

  // 注册 uart 中断处理函数
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(uart_intid,
                                                            uart_handler);

  cpu_io::EnableInterrupt();

  cpu_io::CNTV_CTL_EL0::ENABLE::Clear();
  cpu_io::CNTV_CTL_EL0::IMASK::Set();

  uint64_t interval_clk = 2 * cpu_io::CNTFRQ_EL0::Read();
  cpu_io::CNTV_TVAL_EL0::Write(interval_clk);

  cpu_io::CNTV_CTL_EL0::ENABLE::Set();
  cpu_io::CNTV_CTL_EL0::IMASK::Clear();

  return 0;
}

auto InterruptInitSMP(int, const char **) -> int {}
