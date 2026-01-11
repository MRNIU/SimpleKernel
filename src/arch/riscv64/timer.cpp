/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "opensbi_interface.h"
#include "singleton.hpp"
#include "task_manager.hpp"

namespace {
uint64_t interval = 0;
}

// 系统 tick 频率
static constexpr uint64_t kTickPerSec = 100;

void TimerInitSMP() {
  // 开启时钟中断
  cpu_io::Sie::Stie::Set();

  // 设置初次时钟中断时间
  sbi_set_timer(interval);
}

void TimerInit() {
  // 设置 tick 频率
  Singleton<TaskManager>::GetInstance().SetTickFrequency(kTickPerSec);

  // 计算 interval
  interval = Singleton<BasicInfo>::GetInstance().interval / kTickPerSec;

  // 注册时钟中断
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(
      cpu_io::detail::register_info::csr::ScauseInfo::kSupervisorTimerInterrupt,
      [](uint64_t, uint8_t*) -> uint64_t {
        sbi_set_timer(cpu_io::Time::Read() + interval);
        Singleton<TaskManager>::GetInstance().UpdateTick();
        return 0;
      });

  // 开启时钟中断
  cpu_io::Sie::Stie::Set();

  // 设置初次时钟中断时间
  sbi_set_timer(interval);
}
