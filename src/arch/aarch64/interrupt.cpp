/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "interrupt.h"

#include "io.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "virtual_memory.hpp"

std::array<Interrupt::InterruptFunc, Interrupt::kMaxInterrupt>
    Interrupt::interrupt_handlers;

Interrupt::Interrupt() {
  auto [dist_base_addr, dist_size, redist_base_addr, redist_size] =
      Singleton<KernelFdt>::GetInstance().GetGIC().value();
  Singleton<VirtualMemory>::GetInstance().MapMMIO(dist_base_addr, dist_size);
  Singleton<VirtualMemory>::GetInstance().MapMMIO(redist_base_addr,
                                                  redist_size);

  gic_ = std::move(Gic(dist_base_addr, redist_base_addr));

  // 注册默认中断处理函数
  for (auto& i : interrupt_handlers) {
    i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
      klog::Info("Default Interrupt handler 0x%X, 0x%p\n", cause, context);
      return 0;
    };
  }

  // 设置 SGI 0 用于 IPI
  auto cpuid = cpu_io::GetCurrentCoreId();
  gic_.SGI(0, cpuid);

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, cpu_io::TrapContext* context) {
  interrupt_handlers[cause](cause, context);
  cpu_io::ICC_EOIR1_EL1::Write(cause);
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (func) {
    interrupt_handlers[cause] = func;
  }
}

bool Interrupt::SendIpi(uint64_t target_cpu_mask) {
  /// @todo 默认使用 SGI 0 作为 IPI 中断
  constexpr uint64_t kIPISGI = 0;

  uint64_t sgi_value = 0;

  // 设置 INTID 为 0 (SGI 0)
  sgi_value |= (kIPISGI & 0xF) << 24;

  // 设置 TargetList (Aff0 级别，低 16 位)
  sgi_value |= (target_cpu_mask & 0xFFFF);

  // 写入 ICC_SGI1R_EL1 寄存器发送 SGI
  cpu_io::ICC_SGI1R_EL1::Write(sgi_value);

  return true;
}

bool Interrupt::BroadcastIpi() {
  /// @todo 默认使用 SGI 0 作为 IPI 中断
  constexpr uint64_t kIPISGI = 0;

  // 构造 ICC_SGI1R_EL1 寄存器的值
  uint64_t sgi_value = 0;

  // 设置 INTID 为 0 (SGI 0)
  sgi_value |= (kIPISGI & 0xF) << 24;

  // 设置 IRM (Interrupt Routing Mode) 为 1，表示广播到所有 PE
  sgi_value |= (1ULL << 40);

  // 写入 ICC_SGI1R_EL1 寄存器发送 SGI
  cpu_io::ICC_SGI1R_EL1::Write(sgi_value);

  return true;
}
