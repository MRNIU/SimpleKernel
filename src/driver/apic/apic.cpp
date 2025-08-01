/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理类实现
 */

#include "apic.h"

#include <cstring>

#include "kernel_log.hpp"

bool Apic::Init(size_t max_cpu_count) {
  max_cpu_count_ = max_cpu_count;

  klog::Info("Initializing APIC system for %zu CPUs\n", max_cpu_count_);

  // 初始化 Local APIC
  if (!local_apic_.Init()) {
    klog::Err("Failed to initialize Local APIC\n");
    return false;
  }

  klog::Info("APIC system initialized successfully\n");
  return true;
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
  bool result = local_apic_.Init();
  if (result) {
    klog::Info(
        "Local APIC initialized successfully for CPU with APIC ID 0x%x\n",
        cpu_io::GetApicInfo().apic_id);
  } else {
    klog::Err("Failed to initialize Local APIC for current CPU\n");
  }

  return result;
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

void Apic::SendIpi(uint32_t target_apic_id, uint8_t vector) const {
  local_apic_.SendIpi(target_apic_id, vector);
}

void Apic::BroadcastIpi(uint8_t vector) const {
  local_apic_.BroadcastIpi(vector);
}

bool Apic::StartupAp(uint32_t apic_id, const void* ap_code_addr,
                     size_t ap_code_size, uint64_t target_addr) {
  klog::Info("Starting up AP with APIC ID 0x%x\n", apic_id);
  klog::Info("AP code address: %p, size: %zu bytes\n", ap_code_addr,
             ap_code_size);
  klog::Info("Target address: 0x%llx\n", target_addr);

  // 检查参数有效性
  if (ap_code_addr == nullptr || ap_code_size == 0) {
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

  // 将 AP 启动代码复制到指定地址
  klog::Info("Copying AP code to physical address 0x%llx\n", target_addr);
  std::memcpy(reinterpret_cast<void*>(target_addr), ap_code_addr, ap_code_size);

  // 验证复制是否成功
  if (std::memcmp(reinterpret_cast<const void*>(target_addr), ap_code_addr,
                  ap_code_size) != 0) {
    klog::Err("AP code copy verification failed\n");
    return false;
  }

  // 计算启动向量 (物理地址 / 4096)
  uint8_t start_vector = static_cast<uint8_t>(target_addr >> 12);
  klog::Info("Calculated start vector: 0x%x (physical address: 0x%llx)\n",
             start_vector, target_addr);

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

size_t Apic::StartupAllAps(const void* ap_code_addr, size_t ap_code_size,
                           uint64_t target_addr, uint32_t max_wait_ms) {
  klog::Info("Starting up all Application Processors (APs)\n");
  klog::Info("AP code address: %p, size: %zu bytes\n", ap_code_addr,
             ap_code_size);
  klog::Info("Target address: 0x%llx\n", target_addr);
  klog::Info("Max wait time: %u ms\n", max_wait_ms);

  // 检查参数有效性
  if (ap_code_addr == nullptr || ap_code_size == 0) {
    klog::Err("Invalid AP code parameters\n");
    return 0;
  }

  auto current_apic_id = cpu_io::GetApicInfo().apic_id;
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

    if (StartupAp(static_cast<uint32_t>(apic_id), ap_code_addr, ap_code_size,
                  target_addr)) {
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

void Apic::PrintInfo() const { local_apic_.PrintInfo(); }

Apic::IoApicInfo* Apic::FindIoApicByIrq(uint8_t irq) {
  // TODO: 实现根据 IRQ 查找 IO APIC
  // 简单实现：假设 IRQ 0-23 由第一个 IO APIC 处理
  if (io_apic_count_ > 0 && irq < 24) {
    return &io_apics_[0];
  }
  return nullptr;
}
