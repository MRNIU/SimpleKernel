/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief IO APIC 驱动实现
 */

#include "apic.h"
#include "kernel_log.hpp"

bool IoApic::Init(uint64_t base_address) {
  base_address_ = base_address;
  klog::Info("Initializing IO APIC at address 0x%lx\n", base_address);

  // 检查 IO APIC 是否可访问
  uint32_t id = GetId();
  uint32_t version = GetVersion();
  uint32_t max_entries = GetMaxRedirectionEntries();

  klog::Info("IO APIC ID: 0x%x, Version: 0x%x, Max Entries: %u\n", id, version,
             max_entries);

  // 禁用所有重定向条目（设置为屏蔽状态）
  for (uint32_t i = 0; i < max_entries; i++) {
    uint64_t entry = ReadRedirectionEntry(i);
    entry |= (1ULL << 16);  // 设置屏蔽位
    WriteRedirectionEntry(i, entry);
  }

  klog::Info("IO APIC initialization completed\n");
  return true;
}

void IoApic::SetIrqRedirection(uint8_t irq, uint8_t vector,
                               uint32_t destination_apic_id, bool mask) {
  klog::Info("Setting IRQ %u redirection: vector=0x%x, dest=0x%x, mask=%s\n",
             irq, vector, destination_apic_id, mask ? "true" : "false");

  // 检查 IRQ 是否在有效范围内
  uint32_t max_entries = GetMaxRedirectionEntries();
  if (irq >= max_entries) {
    klog::Err("IRQ %u exceeds maximum entries %u\n", irq, max_entries);
    return;
  }

  // 构造重定向表项
  uint64_t entry = 0;

  // 设置中断向量 (位 0-7)
  entry |= vector & 0xFF;

  // 设置传递模式为固定 (位 8-10 = 000)
  // entry |= (0 << 8);  // 固定模式，默认为 0

  // 设置目标模式为物理 (位 11 = 0)
  // entry |= (0 << 11);  // 物理模式，默认为 0

  // 设置传递状态为空闲 (位 12 = 0)
  // entry |= (0 << 12);  // 空闲状态，默认为 0

  // 设置极性为高电平有效 (位 13 = 0)
  // entry |= (0 << 13);  // 高电平有效，默认为 0

  // 设置触发模式为边沿触发 (位 15 = 0)
  // entry |= (0 << 15);  // 边沿触发，默认为 0

  // 设置屏蔽位 (位 16)
  if (mask) {
    entry |= (1ULL << 16);
  }

  // 设置目标 APIC ID (位 56-63)
  entry |= (static_cast<uint64_t>(destination_apic_id & 0xFF) << 56);

  WriteRedirectionEntry(irq, entry);
}

void IoApic::MaskIrq(uint8_t irq) {
  klog::Info("Masking IRQ %u\n", irq);

  uint32_t max_entries = GetMaxRedirectionEntries();
  if (irq >= max_entries) {
    klog::Err("IRQ %u exceeds maximum entries %u\n", irq, max_entries);
    return;
  }

  uint64_t entry = ReadRedirectionEntry(irq);
  entry |= (1ULL << 16);  // 设置屏蔽位
  WriteRedirectionEntry(irq, entry);
}

void IoApic::UnmaskIrq(uint8_t irq) {
  klog::Info("Unmasking IRQ %u\n", irq);

  uint32_t max_entries = GetMaxRedirectionEntries();
  if (irq >= max_entries) {
    klog::Err("IRQ %u exceeds maximum entries %u\n", irq, max_entries);
    return;
  }

  uint64_t entry = ReadRedirectionEntry(irq);
  entry &= ~(1ULL << 16);  // 清除屏蔽位
  WriteRedirectionEntry(irq, entry);
}

uint32_t IoApic::GetId() const {
  return (ReadReg(kRegId) >> 24) & 0x0F;  // ID 位于位 24-27
}

uint32_t IoApic::GetVersion() const {
  return ReadReg(kRegVer) & 0xFF;  // 版本位于位 0-7
}

uint32_t IoApic::GetMaxRedirectionEntries() const {
  return ((ReadReg(kRegVer) >> 16) & 0xFF) +
         1;  // MRE 位于位 16-23，实际数量需要 +1
}

uint64_t IoApic::GetBaseAddress() const { return base_address_; }

void IoApic::PrintInfo() const {
  klog::Info("IO APIC Information\n");
  klog::Info("Base Address: 0x%lx\n", base_address_);
  klog::Info("ID: 0x%x\n", GetId());
  klog::Info("Version: 0x%x\n", GetVersion());
  klog::Info("Max Redirection Entries: %u\n", GetMaxRedirectionEntries());
}

uint32_t IoApic::ReadReg(uint32_t reg) const {
  // 写入寄存器选择器
  *reinterpret_cast<volatile uint32_t*>(base_address_ + kRegSel) = reg;
  // 读取寄存器窗口
  return *reinterpret_cast<volatile uint32_t*>(base_address_ + kRegWin);
}

void IoApic::WriteReg(uint32_t reg, uint32_t value) {
  // 写入寄存器选择器
  *reinterpret_cast<volatile uint32_t*>(base_address_ + kRegSel) = reg;
  // 写入寄存器窗口
  *reinterpret_cast<volatile uint32_t*>(base_address_ + kRegWin) = value;
}

uint64_t IoApic::ReadRedirectionEntry(uint8_t irq) const {
  uint32_t low_reg = kRedTblBase + (irq * 2);
  uint32_t high_reg = low_reg + 1;

  uint32_t low = ReadReg(low_reg);
  uint32_t high = ReadReg(high_reg);

  return (static_cast<uint64_t>(high) << 32) | low;
}

void IoApic::WriteRedirectionEntry(uint8_t irq, uint64_t value) {
  uint32_t low_reg = kRedTblBase + (irq * 2);
  uint32_t high_reg = low_reg + 1;

  uint32_t low = static_cast<uint32_t>(value & 0xFFFFFFFF);
  uint32_t high = static_cast<uint32_t>((value >> 32) & 0xFFFFFFFF);

  WriteReg(low_reg, low);
  WriteReg(high_reg, high);
}
