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
}  // namespace

void TimerInitSMP() {
  Singleton<Interrupt>::GetInstance().PPI(timer_intid,
                                          cpu_io::GetCurrentCoreId());

  cpu_io::CNTV_CTL_EL0::ENABLE::Clear();
  cpu_io::CNTV_CTL_EL0::IMASK::Set();

  cpu_io::CNTV_TVAL_EL0::Write(interval);

  cpu_io::CNTV_CTL_EL0::ENABLE::Set();
  cpu_io::CNTV_CTL_EL0::IMASK::Clear();
}

void TimerInit() {
  // 计算 interval
  interval = Singleton<BasicInfo>::GetInstance().interval / SIMPLEKERNEL_TICK;

  // 获取定时器中断号
  timer_intid =
      Singleton<KernelFdt>::GetInstance().GetAarch64Intid("arm,armv8-timer") +
      Gic::kPPIBase;
  klog::Info("timer_intid: %d\n", timer_intid);
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      timer_intid, [](uint64_t, uint8_t*) -> uint64_t {
        cpu_io::CNTV_TVAL_EL0::Write(cpu_io::CNTFRQ_EL0::Read() + interval);
        klog::Info("Timer interrupt on core %d\n", cpu_io::GetCurrentCoreId());
        return 0;
      });

  Singleton<Interrupt>::GetInstance().PPI(timer_intid,
                                          cpu_io::GetCurrentCoreId());

  cpu_io::CNTV_CTL_EL0::ENABLE::Clear();
  cpu_io::CNTV_CTL_EL0::IMASK::Set();

  cpu_io::CNTV_TVAL_EL0::Write(interval);

  cpu_io::CNTV_CTL_EL0::ENABLE::Set();
  cpu_io::CNTV_CTL_EL0::IMASK::Clear();
}
