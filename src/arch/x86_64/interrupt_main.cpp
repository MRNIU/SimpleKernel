/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
 */

#include <cpu_io.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_log.hpp"
#include "kstd_cstdio"
#include "kstd_iostream"

namespace {
using InterruptDelegate = InterruptBase::InterruptDelegate;

// 定义 APIC 时钟中断向量号（使用高优先级向量）
constexpr uint8_t kApicTimerVector = 0xF0;
constexpr uint32_t kApicTimerFrequencyHz = 100;

/**
 * @brief APIC 时钟中断处理函数
 * @param cause 中断原因
 * @param context 中断上下文
 * @return uint64_t 返回值
 */
uint64_t ApicTimerHandler(uint64_t cause, cpu_io::TrapContext* context) {
  // APIC 时钟中断处理
  static uint64_t tick_count = 0;
  tick_count++;

  // 每100次中断打印一次信息（减少日志输出）
  if (tick_count % 100 == 0) {
    klog::Info("APIC Timer interrupt {}, vector {:#x}\n", tick_count,
               static_cast<uint32_t>(cause));
  }

  // 发送 EOI 信号给 Local APIC
  InterruptSingleton::instance().apic().SendEoi();
  return 0;
}

/**
 * @brief 键盘中断处理函数
 * @param cause 中断原因
 * @param context 中断上下文
 * @return uint64_t 返回值
 */
uint64_t KeyboardHandler(uint64_t cause, cpu_io::TrapContext* context) {
  klog::Info("Keyboard interrupt received, vector {:#x}\n",
             static_cast<uint32_t>(cause));

  // 读取键盘扫描码
  // 8042 键盘控制器的数据端口是 0x60
  uint8_t scancode = cpu_io::In<uint8_t>(0x60);

  // 简单的扫描码处理 - 仅显示按下的键（忽略释放事件）
  if (!(scancode & 0x80)) {  // 最高位为0表示按下
    klog::Info("Key pressed: scancode {:#04x}\n", scancode);

    // 简单的扫描码到ASCII的映射（仅作为示例）
    static const char scancode_to_ascii[] = {
        0,   27,  '1',  '2',  '3',  '4', '5', '6',  '7', '8', '9', '0',
        '-', '=', '\b', '\t', 'q',  'w', 'e', 'r',  't', 'y', 'u', 'i',
        'o', 'p', '[',  ']',  '\n', 0,   'a', 's',  'd', 'f', 'g', 'h',
        'j', 'k', 'l',  ';',  '\'', '`', 0,   '\\', 'z', 'x', 'c', 'v',
        'b', 'n', 'm',  ',',  '.',  '/', 0,   '*',  0,   ' '};

    if (scancode < sizeof(scancode_to_ascii) && scancode_to_ascii[scancode]) {
      char ascii_char = scancode_to_ascii[scancode];
      klog::Info("Key: '{}'\n", ascii_char);
    }
  }

  // 发送 EOI 信号给 Local APIC
  InterruptSingleton::instance().apic().SendEoi();
  return 0;
}

};  // namespace

void InterruptInit(int, const char**) {
  InterruptSingleton::create();

  // 初始化 APIC（从 ArchInit 移至此处）
  InterruptSingleton::instance().InitApic(
      BasicInfoSingleton::instance().core_count);
  InterruptSingleton::instance().apic().InitCurrentCpuLocalApic().or_else(
      [](Error err) -> Expected<void> {
        klog::Err("Failed to initialize APIC: {}\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return std::unexpected(err);
      });

  InterruptSingleton::instance().SetUpIdtr();

  // 注册 APIC Timer 中断处理函数（Local APIC 内部中断，不走 IO APIC）
  InterruptSingleton::instance().RegisterInterruptFunc(
      kApicTimerVector, InterruptDelegate::create<ApicTimerHandler>());

  // 通过统一接口注册键盘外部中断（IRQ 1 = PS/2 键盘，先注册 handler 再启用 IO
  // APIC）
  constexpr uint8_t kKeyboardIrq = 1;
  InterruptSingleton::instance()
      .RegisterExternalInterrupt(kKeyboardIrq, cpu_io::GetCurrentCoreId(), 0,
                                 InterruptDelegate::create<KeyboardHandler>())
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to register keyboard IRQ: {}\n", err.message());
        return std::unexpected(err);
      });

  // 启用 Local APIC 定时器
  InterruptSingleton::instance().apic().SetupPeriodicTimer(
      kApicTimerFrequencyHz, kApicTimerVector);
  // 开启中断
  cpu_io::Rflags::If::Set();

  klog::Info("Hello InterruptInit\n");
}

void InterruptInitSMP(int, const char**) {
  InterruptSingleton::instance().SetUpIdtr();

  // 初始化当前 AP 核的 Local APIC
  InterruptSingleton::instance().apic().InitCurrentCpuLocalApic().or_else(
      [](Error err) -> Expected<void> {
        klog::Err("Failed to initialize APIC for AP: {}\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return std::unexpected(err);
      });

  // 启用 Local APIC 定时器
  InterruptSingleton::instance().apic().SetupPeriodicTimer(
      kApicTimerFrequencyHz, kApicTimerVector);
  cpu_io::Rflags::If::Set();
  klog::Info("Hello InterruptInit SMP\n");
}
