/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <etl/io_port.h>

#include "apic.h"
#include "kernel_log.hpp"

auto LocalApic::Init() -> Expected<void> {
  // 检查 APIC 是否全局启用
  if (!cpu_io::msr::apic::IsGloballyEnabled()) {
    cpu_io::msr::apic::EnableGlobally();
  }

  // 首先尝试启用 x2APIC 模式
  if (EnableX2Apic()) {
    is_x2apic_mode_ = true;
  } else {
    if (!EnableXApic()) {
      klog::Err("Failed to enable APIC in any mode\n");
      return std::unexpected(Error(ErrorCode::kApicInitFailed));
    }
    is_x2apic_mode_ = false;
  }

  // 启用 Local APIC(通过设置 SIVR)
  uint32_t sivr;
  if (is_x2apic_mode_) {
    sivr = cpu_io::msr::apic::ReadSivr();
  } else {
    etl::io_port_ro<uint32_t> sivr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicSivrOffset)};
    sivr = sivr_reg.read();
  }

  // 设置 APIC Software Enable 位
  sivr |= kApicSoftwareEnableBit;
  // 设置虚假中断向量为 0xFF
  sivr |= kSpuriousVector;

  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteSivr(sivr);
  } else {
    etl::io_port_wo<uint32_t> sivr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicSivrOffset)};
    sivr_reg.write(sivr);
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
    etl::io_port_wo<uint32_t> lvt_timer{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtTimerOffset)};
    lvt_timer.write(kLvtMaskBit);
    etl::io_port_wo<uint32_t> lvt_lint0{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtLint0Offset)};
    lvt_lint0.write(kLvtMaskBit);
    etl::io_port_wo<uint32_t> lvt_lint1{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtLint1Offset)};
    lvt_lint1.write(kLvtMaskBit);
    etl::io_port_wo<uint32_t> lvt_error{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtErrorOffset)};
    lvt_error.write(kLvtMaskBit);
  }
  return {};
}

uint32_t LocalApic::GetApicVersion() const {
  if (is_x2apic_mode_) {
    return cpu_io::msr::apic::ReadVersion();
  } else {
    // 版本寄存器在偏移 0x30 处
    etl::io_port_ro<uint32_t> ver_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicVersionOffset)};
    return ver_reg.read();
  }
}

void LocalApic::SendEoi() const {
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteEoi(0);
  } else {
    // 写入 EOI 寄存器(偏移 0xB0)
    etl::io_port_wo<uint32_t> eoi_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicEoiOffset)};
    eoi_reg.write(0);
  }
}

auto LocalApic::SendIpi(uint32_t destination_apic_id, uint8_t vector) const
    -> Expected<void> {
  if (is_x2apic_mode_) {
    auto icr = static_cast<uint64_t>(vector);
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // ICR 分为两个 32 位寄存器：ICR_LOW (0x300) 和 ICR_HIGH (0x310)

    // 设置目标 APIC ID(ICR_HIGH 的位 24-31)
    auto icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    etl::io_port_wo<uint32_t> icr_high_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrHighOffset)};
    icr_high_reg.write(icr_high);

    // 设置向量和传递模式(ICR_LOW)
    auto icr_low = static_cast<uint32_t>(vector);
    etl::io_port_wo<uint32_t> icr_low_wo{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrLowOffset)};
    icr_low_wo.write(icr_low);

    {
      etl::io_port_ro<uint32_t> icr_low_ro{reinterpret_cast<volatile uint32_t*>(
          apic_base_ + kXApicIcrLowOffset)};
      while ((icr_low_ro.read() & kIcrDeliveryStatusBit) != 0) {
        ;
      }
    }
  }
  return {};
}

auto LocalApic::BroadcastIpi(uint8_t vector) const -> Expected<void> {
  if (is_x2apic_mode_) {
    auto icr = static_cast<uint64_t>(vector);
    // 目标简写：除自己外的所有 CPU
    icr |= 0x80000;

    cpu_io::msr::apic::WriteIcr(icr);

    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // 广播到除自己外的所有 CPU(目标简写模式)
    // ICR_HIGH 设为 0(不使用具体目标 ID)
    etl::io_port_wo<uint32_t> icr_high_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrHighOffset)};
    icr_high_reg.write(0);

    // ICR_LOW：向量 + 目标简写模式(位 18-19 = 11)
    auto icr_low = static_cast<uint32_t>(vector) | kIcrBroadcastMode;
    etl::io_port_wo<uint32_t> icr_low_wo{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrLowOffset)};
    icr_low_wo.write(icr_low);

    {
      etl::io_port_ro<uint32_t> icr_low_ro{reinterpret_cast<volatile uint32_t*>(
          apic_base_ + kXApicIcrLowOffset)};
      while ((icr_low_ro.read() & kIcrDeliveryStatusBit) != 0) {
        ;
      }
    }
  }
  return {};
}

void LocalApic::SetTaskPriority(uint8_t priority) const {
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteTpr(static_cast<uint32_t>(priority));
  } else {
    etl::io_port_wo<uint32_t> tpr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicTprOffset)};
    tpr_reg.write(static_cast<uint32_t>(priority));
  }
}

uint8_t LocalApic::GetTaskPriority() const {
  if (is_x2apic_mode_) {
    return static_cast<uint8_t>(cpu_io::msr::apic::ReadTpr() & kApicIdMask);
  } else {
    etl::io_port_ro<uint32_t> tpr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicTprOffset)};
    auto tpr = tpr_reg.read();
    return static_cast<uint8_t>(tpr & kApicIdMask);
  }
}

void LocalApic::EnableTimer(uint32_t initial_count, uint32_t divide_value,
                            uint8_t vector, bool periodic) const {
  if (is_x2apic_mode_) {
    cpu_io::msr::apic::WriteTimerDivide(divide_value);

    auto lvt_timer = static_cast<uint32_t>(vector);
    if (periodic) {
      lvt_timer |= kLvtPeriodicMode;
    }

    cpu_io::msr::apic::WriteLvtTimer(lvt_timer);
    cpu_io::msr::apic::WriteTimerInitCount(initial_count);
  } else {
    // 设置分频器(偏移 0x3E0)
    etl::io_port_wo<uint32_t> divide_reg{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicTimerDivideOffset)};
    divide_reg.write(divide_value);

    // 设置 LVT 定时器寄存器(偏移 0x320)
    auto lvt_timer = static_cast<uint32_t>(vector);
    if (periodic) {
      lvt_timer |= kLvtPeriodicMode;
    }

    etl::io_port_wo<uint32_t> lvt_timer_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtTimerOffset)};
    lvt_timer_reg.write(lvt_timer);

    // 设置初始计数值(偏移 0x380)
    etl::io_port_wo<uint32_t> init_count_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicTimerInitCountOffset)};
    init_count_reg.write(initial_count);
  }
}

void LocalApic::DisableTimer() const {
  if (is_x2apic_mode_) {
    auto lvt_timer = cpu_io::msr::apic::ReadLvtTimer();
    lvt_timer |= kLvtMaskBit;
    cpu_io::msr::apic::WriteLvtTimer(lvt_timer);
    cpu_io::msr::apic::WriteTimerInitCount(0);
  } else {
    etl::io_port_rw<uint32_t> lvt_timer_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtTimerOffset)};
    auto lvt_timer = lvt_timer_reg.read();
    lvt_timer |= kLvtMaskBit;
    lvt_timer_reg.write(lvt_timer);
    etl::io_port_wo<uint32_t> init_count_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicTimerInitCountOffset)};
    init_count_reg.write(0);
  }
}

uint32_t LocalApic::GetTimerCurrentCount() const {
  if (is_x2apic_mode_) {
    return cpu_io::msr::apic::ReadTimerCurrCount();
  } else {
    // 当前计数寄存器在偏移 0x390 处
    etl::io_port_ro<uint32_t> curr_count_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicTimerCurrCountOffset)};
    return curr_count_reg.read();
  }
}

void LocalApic::SetupPeriodicTimer(uint32_t frequency_hz,
                                   uint8_t vector) const {
  // 使用 APIC 定时器的典型配置
  // 假设 APIC 时钟频率为 100MHz(实际应从 CPU 频率计算)

  // 计算初始计数值
  auto initial_count = kDefaultApicClockHz / frequency_hz;

  // 选择合适的分频值以获得更好的精度
  auto divide_value = kTimerDivideBy1;
  if (initial_count > 0xFFFFFFFF) {
    // 如果计数值太大，使用分频
    divide_value = kTimerDivideBy16;
    initial_count = (kDefaultApicClockHz / 16) / frequency_hz;
  }

  EnableTimer(initial_count, divide_value, vector, true);
}

void LocalApic::SetupOneShotTimer(uint32_t microseconds, uint8_t vector) const {
  // 假设 APIC 时钟频率为 100MHz

  // 计算初始计数值(微秒转换为时钟周期)
  auto initial_count =
      (kDefaultApicClockHz / kMicrosecondsPerSecond) * microseconds;

  // 选择合适的分频值
  auto divide_value = kTimerDivideBy1;
  if (initial_count > 0xFFFFFFFF) {
    divide_value = kTimerDivideBy16;
    initial_count =
        ((kDefaultApicClockHz / 16) / kMicrosecondsPerSecond) * microseconds;
  }

  EnableTimer(initial_count, divide_value, vector, false);
}

void LocalApic::SendInitIpi(uint32_t destination_apic_id) const {
  if (is_x2apic_mode_) {
    auto icr = kInitIpiMode;
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // 设置目标 APIC ID(ICR_HIGH)
    auto icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    etl::io_port_wo<uint32_t> icr_high_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrHighOffset)};
    icr_high_reg.write(icr_high);

    // 发送 INIT IPI(ICR_LOW)
    auto icr_low = kInitIpiMode;
    etl::io_port_wo<uint32_t> icr_low_wo{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrLowOffset)};
    icr_low_wo.write(icr_low);

    {
      etl::io_port_ro<uint32_t> icr_low_ro{reinterpret_cast<volatile uint32_t*>(
          apic_base_ + kXApicIcrLowOffset)};
      while ((icr_low_ro.read() & kIcrDeliveryStatusBit) != 0) {
        ;
      }
    }
  }

  klog::Info("INIT IPI sent to APIC ID 0x%x\n", destination_apic_id);
}

void LocalApic::SendStartupIpi(uint32_t destination_apic_id,
                               uint8_t start_page) const {
  if (is_x2apic_mode_) {
    // SIPI with start page (delivery mode = 110b)
    auto icr = kSipiMode | start_page;
    icr |= static_cast<uint64_t>(destination_apic_id) << 32;

    cpu_io::msr::apic::WriteIcr(icr);

    while ((cpu_io::msr::apic::ReadIcr() & kIcrDeliveryStatusBit) != 0) {
      ;
    }
  } else {
    // 设置目标 APIC ID(ICR_HIGH)
    uint32_t icr_high = (destination_apic_id & kApicIdMask) << kIcrDestShift;
    etl::io_port_wo<uint32_t> icr_high_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrHighOffset)};
    icr_high_reg.write(icr_high);

    // 发送 SIPI(ICR_LOW)
    uint32_t icr_low = kSipiMode | start_page;
    etl::io_port_wo<uint32_t> icr_low_wo{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicIcrLowOffset)};
    icr_low_wo.write(icr_low);

    {
      etl::io_port_ro<uint32_t> icr_low_ro{reinterpret_cast<volatile uint32_t*>(
          apic_base_ + kXApicIcrLowOffset)};
      while ((icr_low_ro.read() & kIcrDeliveryStatusBit) != 0) {
        ;
      }
    }
  }
}

void LocalApic::ConfigureLvtEntries() const {
  if (is_x2apic_mode_) {
    // 配置 LINT0 (通常连接到 8259 PIC 的 INTR)
    cpu_io::msr::apic::WriteLvtLint0(kExtIntMode);

    // 配置 LINT1 (通常连接到 NMI)
    cpu_io::msr::apic::WriteLvtLint1(kNmiMode);

    // 配置错误中断
    cpu_io::msr::apic::WriteLvtError(kErrorVector);
  } else {
    // 配置 LINT0 (偏移 0x350)
    etl::io_port_wo<uint32_t> lint0_reg{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtLint0Offset)};
    lint0_reg.write(kExtIntMode);

    // 配置 LINT1 (偏移 0x360)
    etl::io_port_wo<uint32_t> lint1_reg{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtLint1Offset)};
    lint1_reg.write(kNmiMode);

    // 配置错误中断 (偏移 0x370)
    etl::io_port_wo<uint32_t> error_reg{reinterpret_cast<volatile uint32_t*>(
        apic_base_ + kXApicLvtErrorOffset)};
    error_reg.write(kErrorVector);
  }
}

uint32_t LocalApic::ReadErrorStatus() const {
  if (is_x2apic_mode_) {
    // x2APIC 模式下没有直接的 ESR 访问方式
    // 这里返回 0 表示没有错误
    return 0;
  } else {
    // 取 ESR (Error Status Register, 偏移 0x280)
    // 读取 ESR 之前需要先写入 0
    etl::io_port_rw<uint32_t> esr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicEsrOffset)};
    esr_reg.write(0);
    return esr_reg.read();
  }
}

void LocalApic::PrintInfo() const {
  klog::Info("APIC Version: 0x%x\n", GetApicVersion());
  klog::Info("Mode: %s\n", is_x2apic_mode_ ? "x2APIC" : "xAPIC");
  klog::Info("x2APIC Enabled: %s\n", IsX2ApicEnabled() ? "Yes" : "No");
  klog::Info("Task Priority: 0x%x\n", GetTaskPriority());
  klog::Info("Timer Current Count: %u\n", GetTimerCurrentCount());

  // 读取各种寄存器状态
  if (is_x2apic_mode_) {
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
    etl::io_port_ro<uint32_t> sivr_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ + kXApicSivrOffset)};
    uint32_t sivr = sivr_reg.read();
    klog::Info("SIVR: 0x%x (APIC %s)\n", sivr,
               (sivr & kApicSoftwareEnableBit) ? "Enabled" : "Disabled");

    etl::io_port_ro<uint32_t> lvt_timer_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtTimerOffset)};
    uint32_t lvt_timer = lvt_timer_reg.read();
    klog::Info("LVT Timer: 0x%x\n", lvt_timer);

    etl::io_port_ro<uint32_t> lvt_lint0_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtLint0Offset)};
    uint32_t lvt_lint0 = lvt_lint0_reg.read();
    klog::Info("LVT LINT0: 0x%x\n", lvt_lint0);

    etl::io_port_ro<uint32_t> lvt_lint1_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtLint1Offset)};
    uint32_t lvt_lint1 = lvt_lint1_reg.read();
    klog::Info("LVT LINT1: 0x%x\n", lvt_lint1);

    etl::io_port_ro<uint32_t> lvt_error_reg{
        reinterpret_cast<volatile uint32_t*>(apic_base_ +
                                             kXApicLvtErrorOffset)};
    uint32_t lvt_error = lvt_error_reg.read();
    klog::Info("LVT Error: 0x%x\n", lvt_error);

    klog::Info("APIC Base Address: 0x%lx\n", apic_base_);
  }
}

bool LocalApic::CheckX2ApicSupport() const {
  return cpu_io::cpuid::HasX2Apic();
}

bool LocalApic::EnableXApic() const {
  // 设置 IA32_APIC_BASE.Global_Enable (位11) = 1
  cpu_io::msr::apic::EnableGlobally();
  // 清除 IA32_APIC_BASE.x2APIC_Enable (位10) = 0
  cpu_io::msr::apic::DisableX2Apic();
  return IsXApicEnabled();
}

bool LocalApic::DisableXApic() const {
  // 清除 IA32_APIC_BASE.Global_Enable (位11) = 0
  cpu_io::msr::apic::DisableGlobally();
  return !IsXApicEnabled();
}

bool LocalApic::IsXApicEnabled() const {
  // Global_Enable = 1 && x2APIC_Enable = 0
  return cpu_io::msr::apic::IsGloballyEnabled() &&
         !cpu_io::msr::apic::IsX2ApicEnabled();
}

bool LocalApic::EnableX2Apic() const {
  // 检查 CPU 是否支持 x2APIC
  if (!CheckX2ApicSupport()) {
    return false;
  }

  // 设置 IA32_APIC_BASE.x2APIC_Enable (位10) = 1
  // 同时确保 IA32_APIC_BASE.Global_Enable (位11) = 1
  cpu_io::msr::apic::EnableX2Apic();

  // 验证 x2APIC 是否成功启用
  return IsX2ApicEnabled();
}

bool LocalApic::DisableX2Apic() const {
  // 清除 IA32_APIC_BASE.x2APIC_Enable (位10) = 0
  cpu_io::msr::apic::DisableX2Apic();
  return !IsX2ApicEnabled();
}

bool LocalApic::IsX2ApicEnabled() const {
  return cpu_io::msr::apic::IsX2ApicEnabled();
}

void LocalApic::WakeupAp(uint32_t destination_apic_id,
                         uint8_t start_vector) const {
  // 发送 INIT IPI
  SendInitIpi(destination_apic_id);

  // 等待 10ms (INIT IPI 后的标准等待时间)
  auto delay = 10 * kCalibrationDelayLoop;
  while (delay--) {
    __asm__ volatile("nop");
  }

  // 发送第一个 SIPI
  SendStartupIpi(destination_apic_id, start_vector);

  // 等待 200μs (SIPI 后的标准等待时间)
  delay = 200 * (kCalibrationDelayLoop / 1000);
  while (delay--) {
    __asm__ volatile("nop");
  }

  // 发送第二个 SIPI (为了可靠性)
  SendStartupIpi(destination_apic_id, start_vector);

  // 等待 200μs
  delay = 200 * (kCalibrationDelayLoop / 1000);
  while (delay--) {
    __asm__ volatile("nop");
  }
}
