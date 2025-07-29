/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief APIC 管理类实现
 */

#include "apic.h"
#include "kernel_log.hpp"

bool Apic::Init(size_t max_cpu_count) {
  // TODO: 实现 APIC 系统初始化
  max_cpu_count_ = max_cpu_count > kMaxCpus ? kMaxCpus : max_cpu_count;
  online_cpu_count_ = 0;

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

size_t Apic::GetCpuCount() const { return online_cpu_count_; }

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
  // TODO: 实现启动 AP
  klog::Info("Starting up AP with APIC ID 0x%x, start vector 0x%x\n", apic_id,
             start_vector);
  return false;
}

uint32_t Apic::GetCurrentApicId() {
  // TODO: 实现获取当前 APIC ID
  return local_apic_.GetApicId();
}

bool Apic::IsCpuOnline(uint32_t apic_id) {
  // TODO: 实现检查 CPU 是否在线
  for (size_t i = 0; i < online_cpu_count_; ++i) {
    if (online_cpus_[i] == apic_id) {
      return true;
    }
  }
  return false;
}

void Apic::SetCpuOnline(uint32_t apic_id) {
  // TODO: 实现标记 CPU 为在线
  if (online_cpu_count_ < kMaxCpus && !IsCpuOnline(apic_id)) {
    online_cpus_[online_cpu_count_++] = apic_id;
    klog::Info("CPU with APIC ID 0x%x is now online\n", apic_id);
  }
}

const std::array<uint32_t, 256>& Apic::GetOnlineCpus() const {
  return online_cpus_;
}

void Apic::PrintInfo() const {
  // TODO: 实现打印 APIC 信息
  klog::Info("APIC System Information\n");
  klog::Info("Max CPU Count: %zu\n", max_cpu_count_);
  klog::Info("Online CPU Count: %zu\n", online_cpu_count_);
  klog::Info("IO APIC Count: %zu\n", io_apic_count_);
}

Apic::IoApicInfo* Apic::FindIoApicByIrq(uint8_t irq) {
  // TODO: 实现根据 IRQ 查找 IO APIC
  // 简单实现：假设 IRQ 0-23 由第一个 IO APIC 处理
  if (io_apic_count_ > 0 && irq < 24) {
    return &io_apics_[0];
  }
  return nullptr;
}
