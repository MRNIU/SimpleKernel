/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Local APIC 驱动实现
 */

#include <cpuid.h>

#include "apic.h"
#include "io.hpp"
#include "kernel_log.hpp"

bool LocalApic::Init() {
  klog::Info("Initializing Local APIC\n");

  // 检查 APIC 是否全局启用
  if (!cpu_io::msr::apic::IsGloballyEnabled()) {
    klog::Info("Enabling APIC globally\n");
    cpu_io::msr::apic::EnableGlobally();
  }

  // 首先尝试启用 x2APIC 模式
  if (EnableX2Apic()) {
    is_x2apic_mode_ = true;
    klog::Info("Using x2APIC mode\n");
  } else {
    // 如果 x2APIC 失败，尝试启用传统 xAPIC 模式
    klog::Info("x2APIC not available, trying xAPIC mode\n");
    if (!EnableXApic()) {
      klog::Err("Failed to enable APIC in any mode\n");
      return false;
    }
    is_x2apic_mode_ = false;
    klog::Info("Using xAPIC mode\n");
  }

  // 启用 Local APIC(通过设置 SIVR)
  uint32_t sivr;
  if (is_x2apic_mode_) {
    sivr = cpu_io::msr::apic::ReadSivr();
  } else {
    // xAPIC 模式下通过内存映射读取
    sivr =
        *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicSivrOffset);
  }

  // 设置 APIC Software Enable 位
  sivr |= kApicSoftwareEnableBit;
  // 设置虚假中断向量为 0xFF
  sivr |= kSpuriousVector;

  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteSivr(sivr);
  } else {
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicSivrOffset) =
        sivr;
  }

  // 清除任务优先级
  SetTaskPriority(0);

  // 禁用所有 LVT 条目(设置 mask 位)
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteLvtTimer(kLvtMaskBit);
    cpu_io::msr::apic::WriteLvtLint0(kLvtMaskBit);
    cpu_io::msr::apic::WriteLvtLint1(kLvtMaskBit);
    cpu_io::msr::apic::WriteLvtError(kLvtMaskBit);
  } else {
    // xAPIC 模式下通过内存映射写入
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtTimerOffset) =
        kLvtMaskBit;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtLint0Offset) =
        kLvtMaskBit;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtLint1Offset) =
        kLvtMaskBit;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtErrorOffset) =
        kLvtMaskBit;
  }

  klog::Info("Local APIC initialized successfully in %s mode\n",
             is_x2apic_mode_ ? "x2APIC" : "xAPIC");
  klog::Info("APIC ID: 0x%x\n", GetApicId());
  return true;
}

uint32_t LocalApic::GetApicId() const {
  if (is_x2apic_mode_) {
    return cpu_io::msr::apic::ReadId();
  } else {
    // xAPIC 模式下，APIC ID 在偏移 0x20 处，高 8 位包含 ID
    uint32_t id_reg =
        *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicIdOffset);
    return (id_reg >> kApicIdShift) & kApicIdMask;
  }
}

uint32_t LocalApic::GetApicVersion() const {
  if (is_x2apic_mode_) {
    return cpu_io::msr::apic::ReadVersion();
  } else {
    // xAPIC 模式下，版本寄存器在偏移 0x30 处
    return *reinterpret_cast<volatile uint32_t *>(apic_base_ +
                                                  kXApicVersionOffset);
  }
}

void LocalApic::SendEoi() {
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteEoi(0);
  } else {
    // xAPIC 模式下，写入 EOI 寄存器(偏移 0xB0)
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicEoiOffset) = 0;
  }
}

void LocalApic::SendIpi(uint32_t destination_apic_id, uint8_t vector) {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint64_t icr = static_cast<uint64_t>(vector);
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    // 等待发送完成
    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      // 等待发送状态清除
    }
  } else {
    // xAPIC 模式：使用内存映射接口
    // ICR 分为两个 32 位寄存器：ICR_LOW (0x300) 和 ICR_HIGH (0x310)

    // 设置目标 APIC ID(ICR_HIGH 的位 24-31)
    uint32_t icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicIcrHighOffset) =
        icr_high;

    // 设置向量和传递模式(ICR_LOW)
    uint32_t icr_low = static_cast<uint32_t>(vector);
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicIcrLowOffset) =
        icr_low;

    // 等待发送完成(检查 ICR_LOW 的 Delivery Status 位)
    while ((*reinterpret_cast<volatile uint32_t *>(apic_base_ +
                                                   kXApicIcrLowOffset) &
            kIcrDeliveryStatusBit) != 0) {
      // 等待发送状态清除
    }
  }

  klog::Info("IPI sent to APIC ID 0x%x, vector 0x%x\n", destination_apic_id,
             vector);
}

void LocalApic::BroadcastIpi(uint8_t vector) {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint64_t icr = static_cast<uint64_t>(vector);
    icr |= 0x80000;  // 目标简写：除自己外的所有 CPU

    cpu_io::msr::apic::WriteIcr(icr);

    // 等待发送完成
    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      // 等待发送状态清除
    }
  } else {
    // xAPIC 模式：使用内存映射接口
    // 广播到除自己外的所有 CPU(目标简写模式)

    // ICR_HIGH 设为 0(不使用具体目标 ID)
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicIcrHighOffset) =
        0;

    // ICR_LOW：向量 + 目标简写模式(位 18-19 = 11)
    uint32_t icr_low = static_cast<uint32_t>(vector) | kIcrBroadcastMode;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicIcrLowOffset) =
        icr_low;

    // 等待发送完成
    while ((*reinterpret_cast<volatile uint32_t *>(apic_base_ +
                                                   kXApicIcrLowOffset) &
            kIcrDeliveryStatusBit) != 0) {
      // 等待发送状态清除
    }
  }

  klog::Info("Broadcast IPI sent with vector 0x%x\n", vector);
}

void LocalApic::SetTaskPriority(uint8_t priority) {
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteTpr(static_cast<uint32_t>(priority));
  } else {
    // xAPIC 模式下，TPR 寄存器在偏移 0x80 处
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicTprOffset) =
        static_cast<uint32_t>(priority);
  }
}

uint8_t LocalApic::GetTaskPriority() const {
  if (is_x2apic_mode_) {
    return static_cast<uint8_t>(cpu_io::msr::apic::ReadTpr() & kApicIdMask);
  } else {
    // xAPIC 模式下，TPR 寄存器在偏移 0x80 处
    uint32_t tpr =
        *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicTprOffset);
    return static_cast<uint8_t>(tpr & kApicIdMask);
  }
}

void LocalApic::EnableTimer(uint32_t initial_count, uint32_t divide_value,
                            uint8_t vector, bool periodic) {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    cpu_io::msr::apic::WriteTimerDivide(divide_value);

    uint32_t lvt_timer = static_cast<uint32_t>(vector);
    if (periodic) {
      lvt_timer |= kLvtPeriodicMode;  // 设置周期模式(位 17)
    }

    cpu_io::msr::apic::WriteLvtTimer(lvt_timer);
    cpu_io::msr::apic::WriteTimerInitCount(initial_count);
  } else {
    // xAPIC 模式：使用内存映射接口
    // 设置分频器(偏移 0x3E0)
    *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicTimerDivideOffset) = divide_value;

    // 设置 LVT 定时器寄存器(偏移 0x320)
    uint32_t lvt_timer = static_cast<uint32_t>(vector);
    if (periodic) {
      lvt_timer |= kLvtPeriodicMode;  // 设置周期模式(位 17)
    }

    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtTimerOffset) =
        lvt_timer;

    // 设置初始计数值(偏移 0x380)
    *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicTimerInitCountOffset) = initial_count;
  }

  klog::Info("APIC Timer enabled: vector=0x%x, initial_count=%u, periodic=%s\n",
             vector, initial_count, periodic ? "true" : "false");
}

void LocalApic::DisableTimer() {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint32_t lvt_timer = cpu_io::msr::apic::ReadLvtTimer();
    lvt_timer |= kLvtMaskBit;  // 设置 mask 位(位 16)
    cpu_io::msr::apic::WriteLvtTimer(lvt_timer);
    cpu_io::msr::apic::WriteTimerInitCount(0);
  } else {
    // xAPIC 模式：使用内存映射接口
    uint32_t lvt_timer = *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicLvtTimerOffset);
    lvt_timer |= kLvtMaskBit;  // 设置 mask 位(位 16)
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtTimerOffset) =
        lvt_timer;
    *reinterpret_cast<volatile uint32_t *>(apic_base_ +
                                           kXApicTimerInitCountOffset) = 0;
  }

  klog::Info("APIC Timer disabled\n");
}

uint32_t LocalApic::GetTimerCurrentCount() const {
  if (is_x2apic_mode_) {
    return cpu_io::msr::apic::ReadTimerCurrCount();
  } else {
    // xAPIC 模式下，当前计数寄存器在偏移 0x390 处
    return *reinterpret_cast<volatile uint32_t *>(apic_base_ +
                                                  kXApicTimerCurrCountOffset);
  }
}

void LocalApic::SetupPeriodicTimer(uint32_t frequency_hz, uint8_t vector) {
  // 使用 APIC 定时器的典型配置
  // 假设 APIC 时钟频率为 100MHz(实际应从 CPU 频率计算)

  // 计算初始计数值
  uint32_t initial_count = kDefaultApicClockHz / frequency_hz;

  // 选择合适的分频值以获得更好的精度
  uint32_t divide_value = kTimerDivideBy1;  // 分频 1
  if (initial_count > 0xFFFFFFFF) {
    // 如果计数值太大，使用分频
    divide_value = kTimerDivideBy16;  // 分频 16
    initial_count = (kDefaultApicClockHz / 16) / frequency_hz;
  }

  EnableTimer(initial_count, divide_value, vector, true);

  klog::Info("Periodic timer setup: frequency=%u Hz, vector=0x%x\n",
             frequency_hz, vector);
}

void LocalApic::SetupOneShotTimer(uint32_t microseconds, uint8_t vector) {
  // 假设 APIC 时钟频率为 100MHz

  // 计算初始计数值(微秒转换为时钟周期)
  uint32_t initial_count =
      (kDefaultApicClockHz / kMicrosecondsPerSecond) * microseconds;

  // 选择合适的分频值
  uint32_t divide_value = kTimerDivideBy1;  // 分频 1
  if (initial_count > 0xFFFFFFFF) {
    divide_value = kTimerDivideBy16;  // 分频 16
    initial_count =
        ((kDefaultApicClockHz / 16) / kMicrosecondsPerSecond) * microseconds;
  }

  EnableTimer(initial_count, divide_value, vector, false);

  klog::Info("One-shot timer setup: delay=%u μs, vector=0x%x\n", microseconds,
             vector);
}

uint32_t LocalApic::CalibrateTimer() {
  // 校准 APIC 定时器频率
  // 这是一个简化的实现，实际使用中应该使用更精确的方法

  klog::Info("Calibrating APIC timer...\n");

  // 设置分频器为 1
  cpu_io::msr::apic::WriteTimerDivide(kTimerDivideBy1);

  // 设置一个大的初始值
  cpu_io::msr::apic::WriteTimerInitCount(kCalibrationCount);

  // 这里应该等待一个已知的时间间隔(例如使用 PIT 或 HPET)
  // 为了简化，我们假设等待了 10ms
  // 在实际实现中，应该使用精确的时间源

  // 模拟等待 10ms
  volatile uint32_t delay = kCalibrationDelayLoop;  // 简单的延时循环
  while (delay--) {
    __asm__ volatile("nop");
  }

  // 读取当前计数值
  uint32_t current_count = GetTimerCurrentCount();
  uint32_t elapsed_ticks = kCalibrationCount - current_count;

  // 假设等待了 10ms，计算 APIC 时钟频率
  uint32_t apic_frequency =
      elapsed_ticks * kCalibrationMultiplier;  // 10ms -> 1s

  klog::Info("APIC timer frequency: ~%u Hz\n", apic_frequency);

  // 停止定时器
  cpu_io::msr::apic::WriteTimerInitCount(0);

  return apic_frequency;
}

void LocalApic::SendInitIpi(uint32_t destination_apic_id) {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint64_t icr = kInitIpiMode;
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    // 等待发送完成
    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // xAPIC 模式：使用内存映射接口
    // 设置目标 APIC ID(ICR_HIGH)
    uint32_t icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    io::Out<uint32_t>(apic_base_ + kXApicIcrHighOffset, icr_high);

    // 发送 INIT IPI(ICR_LOW)
    uint32_t icr_low = kInitIpiMode;
    io::Out<uint32_t>(apic_base_ + kXApicIcrLowOffset, icr_low);

    // 等待发送完成
    while ((io::In<uint32_t>(apic_base_ + kXApicIcrLowOffset) &
            kIcrDeliveryStatusBit) != 0) {
      ;
    }
  }

  klog::Info("INIT IPI sent to APIC ID 0x%x\n", destination_apic_id);
}

void LocalApic::SendStartupIpi(uint32_t destination_apic_id,
                               uint8_t start_page) {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint64_t icr =
        kSipiMode | start_page;  // SIPI with start page (delivery mode = 110b)
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    // 等待发送完成
    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // xAPIC 模式：使用内存映射接口
    // 设置目标 APIC ID(ICR_HIGH)
    uint32_t icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    io::Out<uint32_t>(apic_base_ + kXApicIcrHighOffset, icr_high);

    // 发送 SIPI(ICR_LOW)
    uint32_t icr_low = kSipiMode | start_page;
    io::Out<uint32_t>(apic_base_ + kXApicIcrLowOffset, icr_low);

    // 等待发送完成
    while ((io::In<uint32_t>(apic_base_ + kXApicIcrLowOffset) &
            kIcrDeliveryStatusBit) != 0) {
      ;
    }
  }

  klog::Info("SIPI sent to APIC ID 0x%x, start page 0x%x\n",
             destination_apic_id, start_page);
}

void LocalApic::ConfigureLvtEntries() {
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    // 配置 LINT0 (通常连接到 8259 PIC 的 INTR)
    uint32_t lint0 = kExtIntMode;  // ExtINT delivery mode (111b)
    cpu_io::msr::apic::WriteLvtLint0(lint0);

    // 配置 LINT1 (通常连接到 NMI)
    uint32_t lint1 = kNmiMode;  // NMI delivery mode (100b)
    cpu_io::msr::apic::WriteLvtLint1(lint1);

    // 配置错误中断
    uint32_t error = kErrorVector;  // 使用向量 0xEF
    cpu_io::msr::apic::WriteLvtError(error);
  } else {
    // xAPIC 模式：使用内存映射接口
    // 配置 LINT0 (偏移 0x350)
    uint32_t lint0 = kExtIntMode;  // ExtINT delivery mode (111b)
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtLint0Offset) =
        lint0;

    // 配置 LINT1 (偏移 0x360)
    uint32_t lint1 = kNmiMode;  // NMI delivery mode (100b)
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtLint1Offset) =
        lint1;

    // 配置错误中断 (偏移 0x370)
    uint32_t error = kErrorVector;  // 使用向量 0xEF
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicLvtErrorOffset) =
        error;
  }

  klog::Info("LVT entries configured\n");
}

uint32_t LocalApic::ReadErrorStatus() {
  if (is_x2apic_mode_) {
    // x2APIC 模式下没有直接的 ESR 访问方式
    // 这里返回 0 表示没有错误
    return 0;
  } else {
    // xAPIC 模式：读取 ESR (Error Status Register, 偏移 0x280)
    // 读取 ESR 之前需要先写入 0
    *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicEsrOffset) = 0;
    return *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicEsrOffset);
  }
}

void LocalApic::PrintInfo() const {
  klog::Info("=== Local APIC Information ===\n");
  klog::Info("APIC ID: 0x%x\n", GetApicId());
  klog::Info("APIC Version: 0x%x\n", GetApicVersion());
  klog::Info("Mode: %s\n", is_x2apic_mode_ ? "x2APIC" : "xAPIC");
  klog::Info("x2APIC Enabled: %s\n", IsX2ApicEnabled() ? "Yes" : "No");
  klog::Info("Task Priority: 0x%x\n", GetTaskPriority());
  klog::Info("Timer Current Count: %u\n", GetTimerCurrentCount());

  // 读取各种寄存器状态
  if (is_x2apic_mode_) {
    // x2APIC 模式：使用 MSR 接口
    uint32_t sivr = cpu_io::msr::apic::ReadSivr();
    klog::Info("SIVR: 0x%x (APIC %s)\n", sivr,
               (sivr & kApicSoftwareEnableBit) ? "Enabled" : "Disabled");

    uint32_t lvt_timer = cpu_io::msr::apic::ReadLvtTimer();
    klog::Info("LVT Timer: 0x%x\n", lvt_timer);

    uint32_t lvt_lint0 = cpu_io::msr::apic::ReadLvtLint0();
    klog::Info("LVT LINT0: 0x%x\n", lvt_lint0);

    uint32_t lvt_lint1 = cpu_io::msr::apic::ReadLvtLint1();
    klog::Info("LVT LINT1: 0x%x\n", lvt_lint1);

    uint32_t lvt_error = cpu_io::msr::apic::ReadLvtError();
    klog::Info("LVT Error: 0x%x\n", lvt_error);
  } else {
    // xAPIC 模式：使用内存映射接口
    uint32_t sivr =
        *reinterpret_cast<volatile uint32_t *>(apic_base_ + kXApicSivrOffset);
    klog::Info("SIVR: 0x%x (APIC %s)\n", sivr,
               (sivr & kApicSoftwareEnableBit) ? "Enabled" : "Disabled");

    uint32_t lvt_timer = *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicLvtTimerOffset);
    klog::Info("LVT Timer: 0x%x\n", lvt_timer);

    uint32_t lvt_lint0 = *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicLvtLint0Offset);
    klog::Info("LVT LINT0: 0x%x\n", lvt_lint0);

    uint32_t lvt_lint1 = *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicLvtLint1Offset);
    klog::Info("LVT LINT1: 0x%x\n", lvt_lint1);

    uint32_t lvt_error = *reinterpret_cast<volatile uint32_t *>(
        apic_base_ + kXApicLvtErrorOffset);
    klog::Info("LVT Error: 0x%x\n", lvt_error);

    klog::Info("APIC Base Address: 0x%lx\n", apic_base_);
  }

  klog::Info("==============================\n");
}

bool LocalApic::CheckX2ApicSupport() const {
  // 使用 CPUID 检查 x2APIC 支持
  uint32_t eax;
  uint32_t ebx;
  uint32_t ecx;
  uint32_t edx;
  __get_cpuid(1, &eax, &ebx, &ecx, &edx);

  // ECX 位 21 表示 x2APIC 支持
  return (ecx & (1 << 21)) != 0;
}

bool LocalApic::EnableXApic() const {
  cpu_io::msr::apic::EnableGlobally();
  cpu_io::msr::apic::DisableX2Apic();
  return IsXApicEnabled();
}

bool LocalApic::DisableXApic() const {
  cpu_io::msr::apic::DisableGlobally();
  return !IsXApicEnabled();
}

bool LocalApic::IsXApicEnabled() const {
  return cpu_io::msr::apic::IsGloballyEnabled() &&
         !cpu_io::msr::apic::IsX2ApicEnabled();
}

bool LocalApic::EnableX2Apic() const {
  // 检查 CPU 是否支持 x2APIC
  if (!CheckX2ApicSupport()) {
    return false;
  }

  // 启用 x2APIC 模式
  cpu_io::msr::apic::EnableX2Apic();

  // 验证 x2APIC 是否成功启用
  return IsX2ApicEnabled();
}

bool LocalApic::DisableX2Apic() const {
  cpu_io::msr::apic::DisableX2Apic();
  return !IsX2ApicEnabled();
}

bool LocalApic::IsX2ApicEnabled() const {
  return cpu_io::msr::apic::IsX2ApicEnabled();
}

void LocalApic::SetApicBase(uint64_t base_address) {
  uint64_t apic_base_msr = cpu_io::msr::apic::ReadBase();

  // 保留控制位，更新基地址
  apic_base_msr =
      (apic_base_msr & kApicBaseControlMask) | (base_address & kApicBaseMask);

  cpu_io::msr::apic::WriteBase(apic_base_msr);
  apic_base_ = base_address;
}

bool LocalApic::WakeupAp(uint32_t destination_apic_id, uint8_t start_vector) {
  klog::Info("Waking up AP with APIC ID 0x%x, start vector 0x%x\n",
             destination_apic_id, start_vector);
  // 标准的 INIT-SIPI-SIPI 序列用于唤醒 Application Processor (AP)

  // 步骤 1: 发送 INIT IPI
  klog::Info("Step 1: Sending INIT IPI to APIC ID 0x%x\n", destination_apic_id);
  SendInitIpi(destination_apic_id);

  // 等待 10ms (INIT IPI 后的标准等待时间)
  // 这里使用简单的延时循环，实际使用中应该使用精确的定时器
  volatile uint32_t delay = 10 * kCalibrationDelayLoop;  // 约 10ms
  while (delay--) {
    __asm__ volatile("nop");
  }

  // 步骤 2: 发送第一个 SIPI
  klog::Info("Step 2: Sending first SIPI to APIC ID 0x%x\n",
             destination_apic_id);
  SendStartupIpi(destination_apic_id, start_vector);

  // 等待 200μs (SIPI 后的标准等待时间)
  delay = 200 * (kCalibrationDelayLoop / 1000);  // 约 200μs
  while (delay--) {
    __asm__ volatile("nop");
  }

  // // 步骤 3: 发送第二个 SIPI (为了可靠性)
  // klog::Info("Step 3: Sending second SIPI to APIC ID 0x%x\n",
  // destination_apic_id); SendStartupIpi(destination_apic_id, start_vector);

  // 等待 200μs
  delay = 200 * (kCalibrationDelayLoop / 1000);  // 约 200μs
  while (delay--) {
    __asm__ volatile("nop");
  }

  // 注意：这里无法直接检测 AP 是否成功启动，因为需要 AP 自己报告状态
  // 在实际系统中，AP 启动后应该通过某种机制(如共享内存变量)报告其状态

  klog::Info("INIT-SIPI-SIPI sequence completed for APIC ID 0x%x\n",
             destination_apic_id);
  klog::Info("AP should start execution at physical address 0x%x\n",
             static_cast<uint32_t>(start_vector) * 4096);

  return true;  // 序列发送成功，但不能保证 AP 实际启动成功
}
