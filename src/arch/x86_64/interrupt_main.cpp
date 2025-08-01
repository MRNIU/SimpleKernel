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

// 定义键盘中断向量号
constexpr uint8_t kKeyboardVector = 0xF1;

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

/**
 * @brief 键盘中断处理函数
 * @param cause 中断原因
 * @param context 中断上下文
 * @return uint64_t 返回值
 */
uint64_t KeyboardHandler(uint64_t cause, uint8_t *context) {
  klog::Info("Keyboard interrupt received, vector 0x%X\n",
             static_cast<uint32_t>(cause));

  // 读取键盘扫描码
  // 8042 键盘控制器的数据端口是 0x60
  uint8_t scancode = cpu_io::In<uint8_t>(0x60);

  // 简单的扫描码处理 - 仅显示按下的键（忽略释放事件）
  if (!(scancode & 0x80)) {  // 最高位为0表示按下
    klog::Info("Key pressed: scancode 0x%02X\n", scancode);

    // 简单的扫描码到ASCII的映射（仅作为示例）
    static const char scancode_to_ascii[] = {
        0,   27,  '1',  '2',  '3',  '4', '5', '6',  '7', '8', '9', '0',
        '-', '=', '\b', '\t', 'q',  'w', 'e', 'r',  't', 'y', 'u', 'i',
        'o', 'p', '[',  ']',  '\n', 0,   'a', 's',  'd', 'f', 'g', 'h',
        'j', 'k', 'l',  ';',  '\'', '`', 0,   '\\', 'z', 'x', 'c', 'v',
        'b', 'n', 'm',  ',',  '.',  '/', 0,   '*',  0,   ' '};

    if (scancode < sizeof(scancode_to_ascii) && scancode_to_ascii[scancode]) {
      char ascii_char = scancode_to_ascii[scancode];
      klog::Info("Key: '%c'\n", ascii_char);
    }
  }

  // 发送 EOI 信号给 Local APIC
  Singleton<Apic>::GetInstance().GetCurrentLocalApic().SendEoi();
  return 0;
}

bool EnableKeyboardInterrupt(uint8_t vector) {
  klog::Info("Enabling keyboard interrupt with vector 0x%x\n", vector);

  // 键盘使用 IRQ 1 (传统 PS/2 键盘)
  constexpr uint8_t kKeyboardIrq = 1;

  // 获取当前 CPU 的 APIC ID 作为目标
  uint32_t destination_apic_id = cpu_io::GetApicInfo().apic_id;

  // 通过 APIC 设置 IRQ 重定向到指定向量
  bool success = Singleton<Apic>::GetInstance().SetIrqRedirection(
      kKeyboardIrq, vector, destination_apic_id, false);

  if (success) {
    klog::Info("Keyboard interrupt enabled successfully\n");
  } else {
    klog::Err("Failed to enable keyboard interrupt\n");
  }

  return success;
}

bool DisableKeyboardInterrupt() {
  klog::Info("Disabling keyboard interrupt\n");

  // 键盘使用 IRQ 1
  constexpr uint8_t kKeyboardIrq = 1;

  // 屏蔽键盘中断
  bool success = Singleton<Apic>::GetInstance().MaskIrq(kKeyboardIrq);

  if (success) {
    klog::Info("Keyboard interrupt disabled successfully\n");
  } else {
    klog::Err("Failed to disable keyboard interrupt\n");
  }

  return success;
}

};  // namespace

void InterruptInit(int, const char **) {
  Singleton<Interrupt>::GetInstance().SetUpIdtr();

  // 注册中断处理函数
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(kApicTimerVector,
                                                            ApicTimerHandler);

  // 注册键盘中断处理函数
  Singleton<Interrupt>::GetInstance().RegisterInterruptFunc(kKeyboardVector,
                                                            KeyboardHandler);

  // 启用键盘中断
  EnableKeyboardInterrupt(kKeyboardVector);

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
