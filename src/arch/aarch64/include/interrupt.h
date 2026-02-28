/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_

#include <cpu_io.h>
#include <etl/singleton.h>

#include <cstdint>

#include "gic.h"
#include "interrupt_base.h"
#include "sk_stdio.h"

class Interrupt final : public InterruptBase {
 public:
  static constexpr size_t kMaxInterrupt = 128;

  Interrupt();

  /// @name 构造/析构函数
  /// @{
  Interrupt(const Interrupt&) = delete;
  Interrupt(Interrupt&&) = delete;
  auto operator=(const Interrupt&) -> Interrupt& = delete;
  auto operator=(Interrupt&&) -> Interrupt& = delete;
  ~Interrupt() = default;
  /// @}

  void Do(uint64_t cause, cpu_io::TrapContext* context) override;
  void RegisterInterruptFunc(uint64_t cause, InterruptDelegate func) override;
  auto SendIpi(uint64_t target_cpu_mask) -> Expected<void> override;
  auto BroadcastIpi() -> Expected<void> override;
  auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                 uint32_t priority, InterruptDelegate handler)
      -> Expected<void> override;

  __always_inline void SetUP() const { gic_.SetUP(); }
  __always_inline void SPI(uint32_t intid, uint32_t cpuid) const {
    gic_.SPI(intid, cpuid);
  }
  __always_inline void PPI(uint32_t intid, uint32_t cpuid) const {
    gic_.PPI(intid, cpuid);
  }
  __always_inline void SGI(uint32_t intid, uint32_t cpuid) const {
    gic_.SGI(intid, cpuid);
  }

 private:
  std::array<InterruptDelegate, kMaxInterrupt> interrupt_handlers_;

  Gic gic_;
};

using InterruptSingleton = etl::singleton<Interrupt>;

#endif /* SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_ */
