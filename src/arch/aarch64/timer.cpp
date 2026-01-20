/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel_fdt.hpp"
#include "singleton.hpp"

namespace {
uint64_t interval = 0;
uint64_t timer_intid = 0;

auto timer_handler(uint64_t cause, uint8_t*) -> uint64_t {
  // 2s
  uint64_t interval_clk = 2 * cpu_io::CNTFRQ_EL0::Read();
  cpu_io::CNTV_TVAL_EL0::Write(interval_clk);
  klog::Info("Timer interrupt on core %d\n", cpu_io::GetCurrentCoreId());
  return cause;
}

}  // namespace

void TimerInitSMP() {
  Singleton<Interrupt>::GetInstance().PPI(timer_intid,
                                          cpu_io::GetCurrentCoreId());

  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(timer_intid,
                                                            timer_handler);

  cpu_io::CNTV_CTL_EL0::ENABLE::Clear();
  cpu_io::CNTV_CTL_EL0::IMASK::Set();

  uint64_t interval_clk = 2 * cpu_io::CNTFRQ_EL0::Read();
  cpu_io::CNTV_TVAL_EL0::Write(interval_clk);

  cpu_io::CNTV_CTL_EL0::ENABLE::Set();
  cpu_io::CNTV_CTL_EL0::IMASK::Clear();
}

void TimerInit() {
  timer_intid =
      Singleton<KernelFdt>::GetInstance().GetAarch64Intid("arm,armv8-timer") +
      Gic::kPPIBase;

  klog::Info("timer_intid: %d\n", timer_intid);

  Singleton<Interrupt>::GetInstance().PPI(timer_intid,
                                          cpu_io::GetCurrentCoreId());

  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(timer_intid,
                                                            timer_handler);

  cpu_io::CNTV_CTL_EL0::ENABLE::Clear();
  cpu_io::CNTV_CTL_EL0::IMASK::Set();

  uint64_t interval_clk = 2 * cpu_io::CNTFRQ_EL0::Read();
  cpu_io::CNTV_TVAL_EL0::Write(interval_clk);

  cpu_io::CNTV_CTL_EL0::ENABLE::Set();
  cpu_io::CNTV_CTL_EL0::IMASK::Clear();
}
