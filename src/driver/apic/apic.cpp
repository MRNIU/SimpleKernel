/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理类实现
 */

#include "apic.h"

#include <cstring>

#include "kernel_log.hpp"

Apic::Apic(size_t cpu_count) : cpu_count_(cpu_count) {}

bool Apic::AddIoApic(uint64_t base_address, uint32_t gsi_base) {
  if (io_apic_count_ >= kMaxIoApics) {
    klog::Err("Cannot add more IO APICs, maximum %zu reached\n", kMaxIoApics);
    return false;
  }

  klog::Info("Adding IO APIC at address 0x%lx, GSI base %u\n", base_address,
             gsi_base);

  auto& io_apic_info = io_apics_[io_apic_count_];

  // 初始化 IO APIC
  if (!io_apic_info.instance.Init(base_address)) {
    klog::Err("Failed to initialize IO APIC at address 0x%lx\n", base_address);
    return false;
  }

  // 设置 IO APIC 信息
  io_apic_info.base_address = base_address;
  io_apic_info.gsi_base = gsi_base;
  io_apic_info.gsi_count = io_apic_info.instance.GetMaxRedirectionEntries();
  io_apic_info.valid = true;

  io_apic_count_++;

  klog::Info("IO APIC added successfully. GSI range: %u - %u\n", gsi_base,
             gsi_base + io_apic_info.gsi_count - 1);

  return true;
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
  if (index >= io_apic_count_) {
    return nullptr;
  }
  return &io_apics_[index].instance;
}

IoApic* Apic::FindIoApicByGsi(uint32_t gsi) {
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
  klog::Info("Setting IRQ %u redirection to vector 0x%x, APIC ID 0x%x\n", irq,
             vector, destination_apic_id);

  // 查找处理该 IRQ 的 IO APIC
  auto* io_apic_info = FindIoApicByIrq(irq);
  if (io_apic_info == nullptr || !io_apic_info->valid) {
    klog::Err("No IO APIC found for IRQ %u\n", irq);
    return false;
  }

  // 检查 IRQ 是否在有效范围内
  uint32_t gsi = io_apic_info->gsi_base + irq;
  if (gsi >= io_apic_info->gsi_base + io_apic_info->gsi_count) {
    klog::Err("IRQ %u (GSI %u) exceeds IO APIC range\n", irq, gsi);
    return false;
  }

  // 设置重定向
  io_apic_info->instance.SetIrqRedirection(irq, vector, destination_apic_id,
                                           mask);
  return true;
}

bool Apic::MaskIrq(uint8_t irq) {
  klog::Info("Masking IRQ %u\n", irq);

  // 查找处理该 IRQ 的 IO APIC
  auto* io_apic_info = FindIoApicByIrq(irq);
  if (io_apic_info == nullptr || !io_apic_info->valid) {
    klog::Err("No IO APIC found for IRQ %u\n", irq);
    return false;
  }

  io_apic_info->instance.MaskIrq(irq);
  return true;
}

bool Apic::UnmaskIrq(uint8_t irq) {
  klog::Info("Unmasking IRQ %u\n", irq);

  // 查找处理该 IRQ 的 IO APIC
  auto* io_apic_info = FindIoApicByIrq(irq);
  if (io_apic_info == nullptr || !io_apic_info->valid) {
    klog::Err("No IO APIC found for IRQ %u\n", irq);
    return false;
  }

  io_apic_info->instance.UnmaskIrq(irq);
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

  // 尝试启动 APIC ID 0 到 cpu_count_-1 的所有处理器
  // 跳过当前的 BSP (Bootstrap Processor)
  for (size_t apic_id = 0; apic_id < cpu_count_; apic_id++) {
    // 跳过当前 BSP
    if (static_cast<uint32_t>(apic_id) == cpu_io::GetApicInfo().apic_id) {
      continue;
    }
    StartupAp(static_cast<uint32_t>(apic_id), ap_code_addr, ap_code_size,
              target_addr);
  }
}

void Apic::PrintInfo() const {
  local_apic_.PrintInfo();

  klog::Info("System has %zu IO APIC(s)\n", io_apic_count_);
  for (size_t i = 0; i < io_apic_count_; ++i) {
    const auto& io_apic_info = io_apics_[i];
    if (io_apic_info.valid) {
      klog::Info("IO APIC %zu:\n", i);
      io_apic_info.instance.PrintInfo();
      klog::Info("  GSI Base: %u\n", io_apic_info.gsi_base);
      klog::Info("  GSI Count: %u\n", io_apic_info.gsi_count);
      klog::Info("  GSI Range: %u - %u\n", io_apic_info.gsi_base,
                 io_apic_info.gsi_base + io_apic_info.gsi_count - 1);
    }
  }
}

Apic::IoApicInfo* Apic::FindIoApicByIrq(uint8_t irq) {
  // 首先尝试在第一个 IO APIC 中查找（通常处理 IRQ 0-23）
  if (io_apic_count_ > 0 && io_apics_[0].valid) {
    if (irq < io_apics_[0].gsi_count) {
      return &io_apics_[0];
    }
  }

  // 如果第一个 IO APIC 不能处理该 IRQ，则遍历所有 IO APIC
  for (size_t i = 0; i < io_apic_count_; ++i) {
    auto& io_apic_info = io_apics_[i];
    if (io_apic_info.valid && irq >= io_apic_info.gsi_base &&
        irq < io_apic_info.gsi_base + io_apic_info.gsi_count) {
      return &io_apic_info;
    }
  }

  return nullptr;
}
