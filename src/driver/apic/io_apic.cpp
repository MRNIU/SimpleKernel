/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief IO APIC 驱动实现
 */

#include "io_apic.h"

#include "io.hpp"
#include "kernel_log.hpp"

IoApic::IoApic() {
  klog::Info("Initializing IO APIC at address 0x%lx\n", base_address_);

  // 检查 IO APIC 是否可访问
  uint32_t id = GetId();
  uint32_t version = GetVersion();
  uint32_t max_entries = GetMaxRedirectionEntries();

  klog::Info("IO APIC ID: 0x%x, Version: 0x%x, Max Entries: %u\n", id, version,
             max_entries);

  // 禁用所有重定向条目（设置为屏蔽状态）
  for (uint32_t i = 0; i < max_entries; i++) {
    uint64_t entry = ReadRedirectionEntry(i);
    entry |= kMaskBit;  // 设置屏蔽位
    WriteRedirectionEntry(i, entry);
  }

  klog::Info("IO APIC initialization completed\n");
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
  entry |= vector & kVectorMask;
  // 设置屏蔽位 (位 16)
  if (mask) {
    entry |= kMaskBit;
  }

  // 设置目标 APIC ID (位 56-63)
  entry |= (static_cast<uint64_t>(destination_apic_id & kDestApicIdMask)
            << kDestApicIdShift);

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
  entry |= kMaskBit;  // 设置屏蔽位
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
  entry &= ~kMaskBit;  // 清除屏蔽位
  WriteRedirectionEntry(irq, entry);
}

uint32_t IoApic::GetId() const {
  // ID 位于位 24-27
  return (Read(kRegId) >> 24) & 0x0F;
}

uint32_t IoApic::GetVersion() const {
  // 版本位于位 0-7
  return Read(kRegVer) & 0xFF;
}

uint32_t IoApic::GetMaxRedirectionEntries() const {
  // MRE 位于位 16-23，实际数量需要 +1
  return ((Read(kRegVer) >> 16) & 0xFF) + 1;
}

void IoApic::PrintInfo() const {
  klog::Info("IO APIC Information\n");
  klog::Info("Base Address: 0x%lx\n", base_address_);
  klog::Info("ID: 0x%x\n", GetId());
  klog::Info("Version: 0x%x\n", GetVersion());
  klog::Info("Max Redirection Entries: %u\n", GetMaxRedirectionEntries());
}

uint32_t IoApic::Read(uint32_t reg) const {
  // 写入寄存器选择器
  io::Out<uint32_t>(base_address_ + kRegSel, reg);
  // 读取寄存器窗口
  return io::In<uint32_t>(base_address_ + kRegWin);
}

void IoApic::Write(uint32_t reg, uint32_t value) const {
  // 写入寄存器选择器
  io::Out<uint32_t>(base_address_ + kRegSel, reg);
  // 写入寄存器窗口
  io::Out<uint32_t>(base_address_ + kRegWin, value);
}

uint64_t IoApic::ReadRedirectionEntry(uint8_t irq) const {
  uint32_t low_reg = kRedTblBase + (irq * 2);
  uint32_t high_reg = low_reg + 1;

  uint32_t low = Read(low_reg);
  uint32_t high = Read(high_reg);

  return (static_cast<uint64_t>(high) << 32) | low;
}

void IoApic::WriteRedirectionEntry(uint8_t irq, uint64_t value) {
  uint32_t low_reg = kRedTblBase + (irq * 2);
  uint32_t high_reg = low_reg + 1;

  uint32_t low = static_cast<uint32_t>(value & 0xFFFFFFFF);
  uint32_t high = static_cast<uint32_t>((value >> 32) & 0xFFFFFFFF);

  Write(low_reg, low);
  Write(high_reg, high);
}
