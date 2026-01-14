/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断初始化
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
      Singleton<KernelFdt>::GetInstance().GetGIC();
  Singleton<VirtualMemory>::GetInstance().MapMMIO(dist_base_addr, dist_size);
  Singleton<VirtualMemory>::GetInstance().MapMMIO(redist_base_addr,
                                                  redist_size);

  gic_ = std::move(Gic(dist_base_addr, redist_base_addr));

  // 注册默认中断处理函数
  for (auto& i : interrupt_handlers) {
    i = [](uint64_t cause, uint8_t* context) -> uint64_t {
      klog::Info("Default Interrupt handler 0x%X, 0x%p\n", cause, context);
      return 0;
    };
  }

  klog::Info("Interrupt init.\n");
}

void Interrupt::Do(uint64_t cause, uint8_t* context) {
  auto ret = interrupt_handlers[cause](cause, context);

  if (ret) {
    cpu_io::ICC_EOIR1_EL1::Write(cause);
  }
}

void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptFunc func) {
  if (func) {
    interrupt_handlers[cause] = func;
  }
}

bool Interrupt::SendIpi(uint64_t target_cpu_mask) {
  /// @todo
  return false;
}

bool Interrupt::BroadcastIpi() {
  /// @todo
  return false;
}
