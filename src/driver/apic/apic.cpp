/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "apic.h"

#include <cpu_io.h>

#include <cstring>

#include "kernel_log.hpp"
#include "sk_assert.h"

Apic::Apic(size_t cpu_count) : cpu_count_(cpu_count) {
  // 禁用传统的 8259A PIC 以避免与 APIC 冲突
  cpu_io::Pic::Disable();
}

auto Apic::InitCurrentCpuLocalApic() -> Expected<void> {
  return local_apic_.Init()
      .and_then([]() -> Expected<void> {
        klog::Info(
            "Local APIC initialized successfully for CPU with APIC ID 0x%x\n",
            cpu_io::GetCurrentCoreId());
        return {};
      })
      .or_else([](Error err) -> Expected<void> {
        klog::Err("Failed to initialize Local APIC for current CPU: %s\n",
                  err.message());
        return std::unexpected(err);
      });
}

auto Apic::SetIrqRedirection(uint8_t irq, uint8_t vector,
                             uint32_t destination_apic_id, bool mask)
    -> Expected<void> {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return std::unexpected(Error(ErrorCode::kApicInvalidIrq));
  }

  // 设置重定向
  io_apic_.SetIrqRedirection(irq, vector, destination_apic_id, mask);
  return {};
}

auto Apic::MaskIrq(uint8_t irq) -> Expected<void> {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return std::unexpected(Error(ErrorCode::kApicInvalidIrq));
  }

  io_apic_.MaskIrq(irq);
  return {};
}

auto Apic::UnmaskIrq(uint8_t irq) -> Expected<void> {
  // 检查 IRQ 是否在有效范围内
  if (irq >= io_apic_.GetMaxRedirectionEntries()) {
    klog::Err("IRQ %u exceeds IO APIC range (max: %u)\n", irq,
              io_apic_.GetMaxRedirectionEntries() - 1);
    return std::unexpected(Error(ErrorCode::kApicInvalidIrq));
  }

  io_apic_.UnmaskIrq(irq);
  return {};
}

auto Apic::SendIpi(uint32_t target_apic_id, uint8_t vector) const
    -> Expected<void> {
  return local_apic_.SendIpi(target_apic_id, vector);
}

auto Apic::BroadcastIpi(uint8_t vector) const -> Expected<void> {
  return local_apic_.BroadcastIpi(vector);
}

auto Apic::StartupAp(uint32_t apic_id, uint64_t ap_code_addr,
                     size_t ap_code_size, uint64_t target_addr) const
    -> Expected<void> {
  // 检查参数有效性
  sk_assert_msg(ap_code_addr != 0 && ap_code_size != 0,
                "Invalid AP code parameters: addr=0x%llx, size=%zu",
                ap_code_addr, ap_code_size);

  // 检查目标地址是否对齐到 4KB 边界（SIPI 要求）
  sk_assert_msg((target_addr & 0xFFF) == 0,
                "Target address 0x%llx is not aligned to 4KB boundary",
                target_addr);

  // 检查目标地址是否在有效范围内（实模式可访问）
  // 1MB 以上的地址在实模式下不可访问
  sk_assert_msg(target_addr < 0x100000,
                "Target address 0x%llx exceeds real mode limit (1MB)",
                target_addr);

  std::memcpy(reinterpret_cast<void*>(target_addr),
              reinterpret_cast<void*>(ap_code_addr), ap_code_size);

  // 验证复制是否成功
  if (std::memcmp(reinterpret_cast<const void*>(target_addr),
                  reinterpret_cast<const void*>(ap_code_addr),
                  ap_code_size) != 0) {
    klog::Err("AP code copy verification failed\n");
    return std::unexpected(Error(ErrorCode::kApicCodeCopyFailed));
  }

  // 计算启动向量 (物理地址 / 4096)
  auto start_vector = static_cast<uint8_t>(target_addr >> 12);
  // 使用 Local APIC 发送 INIT-SIPI-SIPI 序列
  local_apic_.WakeupAp(apic_id, start_vector);

  return {};
}

void Apic::StartupAllAps(uint64_t ap_code_addr, size_t ap_code_size,
                         uint64_t target_addr) const {
  sk_assert_msg(ap_code_addr != 0 && ap_code_size != 0,
                "Invalid AP code parameters: addr=0x%llx, size=%zu",
                ap_code_addr, ap_code_size);

  // 启动 APIC ID 0 到 cpu_count_-1 的所有处理器
  // 跳过当前的 BSP (Bootstrap Processor)
  for (size_t apic_id = 0; apic_id < cpu_count_; apic_id++) {
    // 跳过当前 BSP
    if (static_cast<uint32_t>(apic_id) == cpu_io::GetCurrentCoreId()) {
      continue;
    }
    StartupAp(static_cast<uint32_t>(apic_id), ap_code_addr, ap_code_size,
              target_addr)
        .or_else([apic_id](Error err) -> Expected<void> {
          klog::Err("Failed to start AP with APIC ID 0x%x: %s\n", apic_id,
                    err.message());
          return std::unexpected(err);
        });
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
