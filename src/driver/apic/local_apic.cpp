/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Local APIC 实现
 */

#include <cpu_io.h>

#include "apic.h"

auto LocalApic::Init() -> bool {
  // 检查是否支持 x2APIC
  if (!IsX2ApicSupported()) {
    return false;
  }

  // 启用 x2APIC 模式
  EnableX2Apic();

  // 验证 x2APIC 是否成功启用
  if (!IsX2ApicEnabled()) {
    return false;
  }

  // 设置虚假中断向量和启用 APIC
  SetSpuriousVector(0xFF, true);

  // 初始化 LVT 表项，屏蔽所有本地中断
  ConfigureLvt(cpu_io::msr::apic::kLvtTimer, 0, kFixed, true);
  ConfigureLvt(cpu_io::msr::apic::kLvtLint0, 0, kFixed, true);
  ConfigureLvt(cpu_io::msr::apic::kLvtLint1, 0, kFixed, true);
  ConfigureLvt(cpu_io::msr::apic::kLvtError, 0, kFixed, true);

  // 清除任务优先级
  SetTaskPriority(0);

  return true;
}

auto LocalApic::IsX2ApicSupported() -> bool {
  uint32_t eax, ebx, ecx, edx;
  // CPUID.01H:ECX.x2APIC[bit 21] = 1
  __cpuid(1, eax, ebx, ecx, edx);
  return (ecx & (1U << 21)) != 0;
}

auto LocalApic::IsX2ApicEnabled() -> bool {
  return cpu_io::msr::apic::IsX2ApicEnabled();
}

void LocalApic::EnableX2Apic() { cpu_io::msr::apic::EnableX2Apic(); }

auto LocalApic::GetApicId() -> uint32_t { return cpu_io::msr::apic::ReadId(); }

auto LocalApic::GetVersion() -> uint32_t {
  return cpu_io::msr::apic::ReadVersion();
}

void LocalApic::SendEoi() { cpu_io::msr::apic::WriteEoi(0); }

void LocalApic::SetTaskPriority(uint32_t priority) {
  cpu_io::msr::apic::WriteTpr(priority & 0xFF);
}

auto LocalApic::GetTaskPriority() -> uint32_t {
  return cpu_io::msr::apic::ReadTpr();
}

void LocalApic::SendIpi(uint32_t destination_id, uint32_t vector,
                        uint32_t delivery_mode) {
  // x2APIC ICR 格式：
  // Bits 63:32 - 目标 APIC ID
  // Bits 31:20 - 保留
  // Bits 19:18 - 目标简写 (00 = 无简写)
  // Bit  15    - 触发模式 (0 = 边沿)
  // Bit  14    - 电平 (0 = 去断言, 1 = 断言)
  // Bit  12    - 传递状态 (只读)
  // Bit  11    - 目标模式 (0 = 物理, 1 = 逻辑)
  // Bits 10:8  - 传递模式
  // Bits 7:0   - 向量

  uint64_t icr = 0;
  icr |= static_cast<uint64_t>(destination_id) << 32;  // 目标 APIC ID
  icr |= (delivery_mode & 0x7) << 8;                   // 传递模式
  icr |= vector & 0xFF;                                // 向量号

  cpu_io::msr::apic::WriteIcr(icr);
}

void LocalApic::BroadcastIpi(uint32_t vector, uint32_t delivery_mode) {
  // x2APIC 广播 IPI：目标简写 = 11 (所有处理器除自己外)
  uint64_t icr = 0;
  icr |= 0x3ULL << 18;  // 目标简写：所有处理器除自己外
  icr |= (delivery_mode & 0x7) << 8;  // 传递模式
  icr |= vector & 0xFF;               // 向量号

  cpu_io::msr::apic::WriteIcr(icr);
}

void LocalApic::StartTimer(uint32_t initial_count, uint32_t vector,
                           bool periodic, uint32_t divide_value) {
  // 停止当前定时器
  StopTimer();

  // 设置分频配置
  uint32_t divide_config = GetDivideConfig(divide_value);
  cpu_io::msr::apic::WriteTimerDivide(divide_config);

  // 配置 LVT Timer 寄存器
  uint32_t lvt_timer = vector & 0xFF;  // 向量号
  if (periodic) {
    lvt_timer |= (kPeriodic << 17);  // 周期模式
  } else {
    lvt_timer |= (kOneShot << 17);  // 单次模式
  }

  cpu_io::msr::apic::WriteLvtTimer(lvt_timer);

  // 设置初始计数，这将启动定时器
  cpu_io::msr::apic::WriteTimerInitCount(initial_count);
}

void LocalApic::StopTimer() {
  // 屏蔽定时器中断
  uint32_t lvt_timer = cpu_io::msr::apic::ReadLvtTimer();
  lvt_timer |= (1U << 16);  // 设置屏蔽位
  cpu_io::msr::apic::WriteLvtTimer(lvt_timer);

  // 设置初始计数为 0
  cpu_io::msr::apic::WriteTimerInitCount(0);
}

auto LocalApic::GetTimerCurrentCount() -> uint32_t {
  return cpu_io::msr::apic::ReadTimerCurrCount();
}

void LocalApic::SetSpuriousVector(uint32_t vector, bool enable) {
  uint32_t sivr = vector & 0xFF;  // 虚假中断向量
  if (enable) {
    sivr |= (1U << 8);  // APIC 软件启用位
  }

  cpu_io::msr::apic::WriteSivr(sivr);
}

void LocalApic::ConfigureLvt(uint32_t lvt_register, uint32_t vector,
                             uint32_t delivery_mode, bool masked) {
  uint32_t lvt_value = vector & 0xFF;       // 向量号
  lvt_value |= (delivery_mode & 0x7) << 8;  // 传递模式

  if (masked) {
    lvt_value |= (1U << 16);  // 屏蔽位
  }

  // 根据寄存器类型设置特定配置
  switch (lvt_register) {
    case cpu_io::msr::apic::kLvtTimer:
      cpu_io::msr::apic::WriteLvtTimer(lvt_value);
      break;
    case cpu_io::msr::apic::kLvtLint0:
      cpu_io::msr::apic::WriteLvtLint0(lvt_value);
      break;
    case cpu_io::msr::apic::kLvtLint1:
      cpu_io::msr::apic::WriteLvtLint1(lvt_value);
      break;
    case cpu_io::msr::apic::kLvtError:
      cpu_io::msr::apic::WriteLvtError(lvt_value);
      break;
    default:
      // 不支持的 LVT 寄存器
      break;
  }
}

auto LocalApic::GetDivideConfig(uint32_t divide_value) -> uint32_t {
  // x2APIC 定时器分频配置映射
  switch (divide_value) {
    case 1:
      return 0xB;  // 1 (实际上是除以2)
    case 2:
      return 0x0;  // 2
    case 4:
      return 0x1;  // 4
    case 8:
      return 0x2;  // 8
    case 16:
      return 0x3;  // 16
    case 32:
      return 0x8;  // 32
    case 64:
      return 0x9;  // 64
    case 128:
      return 0xA;  // 128
    default:
      return 0x0;  // 默认为 2
  }
}
