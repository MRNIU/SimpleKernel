/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理类实现
 */

#include "apic.h"
#include "kernel_log.hpp"

bool Apic::Init(size_t max_cpu_count) {
  // TODO: 实现 APIC 系统初始化
  max_cpu_count_ = max_cpu_count > kMaxCpus ? kMaxCpus : max_cpu_count;

  klog::Info("Initializing APIC system for %zu CPUs\n", max_cpu_count_);
  return false;
}

bool Apic::AddIoApic(uint64_t base_address, uint32_t gsi_base) {
  // TODO: 实现添加 IO APIC
  if (io_apic_count_ >= kMaxIoApics) {
    klog::Err("Cannot add more IO APICs, maximum %zu reached\n", kMaxIoApics);
    return false;
  }

  klog::Info("Adding IO APIC at address 0x%lx, GSI base %u\n", base_address,
             gsi_base);
  return false;
}

bool Apic::InitCurrentCpuLocalApic() {
  // TODO: 实现当前 CPU 的 Local APIC 初始化
  local_apic_.Init();
  klog::Info("Initializing Local APIC for current CPU\n");
  return false;
}

LocalApic& Apic::GetCurrentLocalApic() { return local_apic_; }

IoApic* Apic::GetIoApic(size_t index) {
  // TODO: 实现获取 IO APIC
  if (index >= io_apic_count_) {
    return nullptr;
  }
  return &io_apics_[index].instance;
}

IoApic* Apic::FindIoApicByGsi(uint32_t gsi) {
  // TODO: 实现根据 GSI 查找 IO APIC
  for (size_t i = 0; i < io_apic_count_; ++i) {
    auto& io_apic_info = io_apics_[i];
    if (io_apic_info.valid && gsi >= io_apic_info.gsi_base &&
        gsi < io_apic_info.gsi_base + io_apic_info.gsi_count) {
      return &io_apic_info.instance;
    }
  }
  return nullptr;
}

size_t Apic::GetIoApicCount() const { return io_apic_count_; }

bool Apic::SetIrqRedirection(uint8_t irq, uint8_t vector,
                             uint32_t destination_apic_id, bool mask) {
  // TODO: 实现 IRQ 重定向设置
  klog::Info("Setting IRQ %u redirection to vector 0x%x, APIC ID 0x%x\n", irq,
             vector, destination_apic_id);
  return false;
}

bool Apic::MaskIrq(uint8_t irq) {
  // TODO: 实现屏蔽 IRQ
  klog::Info("Masking IRQ %u\n", irq);
  return false;
}

bool Apic::UnmaskIrq(uint8_t irq) {
  // TODO: 实现取消屏蔽 IRQ
  klog::Info("Unmasking IRQ %u\n", irq);
  return false;
}

void Apic::SendIpi(uint32_t target_apic_id, uint8_t vector) {
  // TODO: 实现发送 IPI
  klog::Info("Sending IPI to APIC ID 0x%x, vector 0x%x\n", target_apic_id,
             vector);
}

void Apic::BroadcastIpi(uint8_t vector) {
  // TODO: 实现广播 IPI
  klog::Info("Broadcasting IPI with vector 0x%x\n", vector);
}

bool Apic::StartupAp(uint32_t apic_id, uint8_t start_vector) {
  klog::Info("Starting up AP with APIC ID 0x%x, start vector 0x%x\n", apic_id,
             start_vector);

  // 使用 Local APIC 发送 INIT-SIPI-SIPI 序列
  bool result = local_apic_.WakeupAp(apic_id, start_vector);

  if (result) {
    klog::Info("INIT-SIPI-SIPI sequence sent successfully to APIC ID 0x%x\n",
               apic_id);

    // 注意：这里不立即标记 CPU 为在线状态
    // AP 启动后应该调用 SetCpuOnline() 来报告自己的状态
    klog::Info("Waiting for AP 0x%x to come online...\n", apic_id);
  } else {
    klog::Err("Failed to send startup sequence to APIC ID 0x%x\n", apic_id);
  }

  return result;
}

size_t Apic::StartupAllAps(uint8_t start_vector, uint32_t max_wait_ms) {
  klog::Info("Starting up all Application Processors (APs)\n");
  klog::Info("Start vector: 0x%x, Max wait time: %u ms\n", start_vector,
             max_wait_ms);

  uint32_t current_apic_id = GetCurrentApicId();
  klog::Info("Current BSP APIC ID: 0x%x\n", current_apic_id);

  size_t startup_attempts = 0;
  size_t startup_success = 0;

  // 尝试启动 APIC ID 0 到 max_cpu_count_-1 的所有处理器
  // 跳过当前的 BSP (Bootstrap Processor)
  for (size_t apic_id = 0; apic_id < max_cpu_count_; ++apic_id) {
    // 跳过当前 BSP
    if (static_cast<uint32_t>(apic_id) == current_apic_id) {
      continue;
    }

    klog::Info("Attempting to start AP with APIC ID 0x%x\n",
               static_cast<uint32_t>(apic_id));

    startup_attempts++;

    if (StartupAp(static_cast<uint32_t>(apic_id), start_vector)) {
      startup_success++;
      klog::Info("Successfully sent startup sequence to APIC ID 0x%x\n",
                 static_cast<uint32_t>(apic_id));
    } else {
      klog::Err("Failed to start AP with APIC ID 0x%x\n",
                static_cast<uint32_t>(apic_id));
    }

    // 在启动下一个 AP 前稍作等待
    volatile uint32_t delay = 100000;  // 短暂延时
    while (delay--) {
      __asm__ volatile("nop");
    }
  }

  klog::Info(
      "AP startup summary: %zu attempts, %zu successful sequences sent\n",
      startup_attempts, startup_success);
  klog::Info("Note: Actual AP online status depends on AP self-reporting\n");

  // 等待一段时间让 AP 有机会启动并报告状态
  klog::Info("Waiting %u ms for APs to come online...\n", max_wait_ms);

  // 简单的等待实现（实际使用中应该使用精确的定时器）
  volatile uint32_t wait_delay = max_wait_ms * 1000;  // 转换为更小的延时单位
  while (wait_delay--) {
    __asm__ volatile("nop");
  }

  return startup_success;
}

uint32_t Apic::GetCurrentApicId() { return local_apic_.GetApicId(); }

void Apic::PrintInfo() const { local_apic_.PrintInfo(); }

Apic::IoApicInfo* Apic::FindIoApicByIrq(uint8_t irq) {
  // TODO: 实现根据 IRQ 查找 IO APIC
  // 简单实现：假设 IRQ 0-23 由第一个 IO APIC 处理
  if (io_apic_count_ > 0 && irq < 24) {
    return &io_apics_[0];
  }
  return nullptr;
}
