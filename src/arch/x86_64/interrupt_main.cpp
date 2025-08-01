/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include <cpu_io.h>

#include "arch.h"
#include "interrupt.h"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_iostream"

void InterruptInit(int, const char **) {
  // 定义 APIC 时钟中断向量号（使用高优先级向量）
  constexpr uint8_t kApicTimerVector = 0xF0;

  // 启用基于 Local APIC 的时钟中断（100 Hz）
  Singleton<Interrupt>::GetInstance().EnableApicTimer(100, kApicTimerVector);

  // 开启中断
  cpu_io::Rflags::If::Set();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char **) {}
