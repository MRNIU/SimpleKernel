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

namespace {
// 定义 APIC 时钟中断向量号（使用高优先级向量）
constexpr uint8_t kApicTimerVector = 0xF0;
constexpr uint32_t kApicTimerFrequencyHz = 100;

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

void InterruptInit(int, const char **) {
  Singleton<Interrupt>::GetInstance().SetUpIdtr();

  // 注册中断处理函数
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(kApicTimerVector,
                                                            ApicTimerHandler);

  // 启用 Local APIC 定时器
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().SetupPeriodicTimer(
      kApicTimerFrequencyHz, kApicTimerVector);
  // 开启中断
  cpu_io::Rflags::If::Set();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char **) {
  Singleton<Interrupt>::GetInstance().SetUpIdtr();
  // 启用 Local APIC 定时器
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().SetupPeriodicTimer(
      kApicTimerFrequencyHz, kApicTimerVector);
  cpu_io::Rflags::If::Set();
  klog::Info("Hello InterruptInit SMP\n");
}
