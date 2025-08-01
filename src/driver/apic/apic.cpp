/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理类实现
 */

#include "apic.h"

#include <cstring>

#include "kernel_log.hpp"

Apic::Apic(size_t cpu_count) : cpu_count_(cpu_count) {}

bool Apic::InitCurrentCpuLocalApic() {
  auto result = local_apic_.Init();
  if (result) {
    klog::Info(
        "Local APIC initialized successfully for CPU with APIC ID 0x%x\n",
        cpu_io::GetApicInfo().apic_id);
  } else {
    klog::Err("Failed to initialize Local APIC for current CPU\n");
  }

  return result;
}

bool Apic::SetIrqRedirection(uint8_t irq, uint8_t vector,
                             uint32_t destination_apic_id, bool mask) {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return false;
  }

  // 设置重定向
  io_apic_.SetIrqRedirection(irq, vector, destination_apic_id, mask);
  return true;
}

bool Apic::MaskIrq(uint8_t irq) {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return false;
  }

  io_apic_.MaskIrq(irq);
  return true;
}

bool Apic::UnmaskIrq(uint8_t irq) {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return false;
  }

  io_apic_.UnmaskIrq(irq);
  return true;
}

void Apic::SendIpi(uint32_t target_apic_id, uint8_t vector) const {
  local_apic_.SendIpi(target_apic_id, vector);
}

void Apic::BroadcastIpi(uint8_t vector) const {
  local_apic_.BroadcastIpi(vector);
}

bool Apic::StartupAp(uint32_t apic_id, uint64_t ap_code_addr,
                     size_t ap_code_size, uint64_t target_addr) const {
  // 检查参数有效性
  if (ap_code_addr == 0 || ap_code_size == 0) {
    klog::Err("Invalid AP code parameters\n");
    return false;
  }

  // 检查目标地址是否对齐到 4KB 边界（SIPI 要求）
  if (target_addr & 0xFFF) {
    klog::Err("Target address 0x%llx is not aligned to 4KB boundary\n",
              target_addr);
    return false;
  }

  // 检查目标地址是否在有效范围内（实模式可访问）
  // 1MB 以上的地址在实模式下不可访问
  if (target_addr >= 0x100000) {
    klog::Err("Target address 0x%llx exceeds real mode limit (1MB)\n",
              target_addr);
    return false;
  }

  std::memcpy(reinterpret_cast<void*>(target_addr),
              reinterpret_cast<void*>(ap_code_addr), ap_code_size);

  // 验证复制是否成功
  if (std::memcmp(reinterpret_cast<const void*>(target_addr),
                  reinterpret_cast<const void*>(ap_code_addr),
                  ap_code_size) != 0) {
    klog::Err("AP code copy verification failed\n");
    return false;
  }

  // 计算启动向量 (物理地址 / 4096)
  auto start_vector = static_cast<uint8_t>(target_addr >> 12);
  // 使用 Local APIC 发送 INIT-SIPI-SIPI 序列
  local_apic_.WakeupAp(apic_id, start_vector);

  return true;
}

void Apic::StartupAllAps(uint64_t ap_code_addr, size_t ap_code_size,
                         uint64_t target_addr) const {
  if (ap_code_addr == 0 || ap_code_size == 0) {
    klog::Err("Invalid AP code parameters\n");
    return;
  }

  // 启动 APIC ID 0 到 cpu_count_-1 的所有处理器
  // 跳过当前的 BSP (Bootstrap Processor)
  for (size_t apic_id = 0; apic_id < cpu_count_; apic_id++) {
    // 跳过当前 BSP
    if (static_cast<uint32_t>(apic_id) == cpu_io::GetApicInfo().apic_id) {
      continue;
    }
    auto ret = StartupAp(static_cast<uint32_t>(apic_id), ap_code_addr,
                         ap_code_size, target_addr);
    if (!ret) {
      klog::Err("Failed to start AP with APIC ID 0x%x\n", apic_id);
    }
  }
}

void Apic::SendEoi() const { local_apic_.SendEoi(); }

void Apic::SetupPeriodicTimer(uint32_t frequency_hz, uint8_t vector) const {
  local_apic_.SetupPeriodicTimer(frequency_hz, vector);
}

void Apic::PrintInfo() const {
  local_apic_.PrintInfo();
  io_apic_.PrintInfo();
}
