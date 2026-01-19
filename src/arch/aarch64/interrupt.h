/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief interrupt 头文件
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_

#include <cpu_io.h>

#include <cstdint>

#include "gic.h"
#include "interrupt_base.h"
#include "singleton.hpp"
#include "sk_stdio.h"

class Interrupt final : public InterruptBase {
 public:
  static constexpr const size_t kMaxInterrupt = 128;

  Interrupt();

  /// @name 构造/析构函数
  /// @{
  Interrupt(const Interrupt&) = delete;
  Interrupt(Interrupt&&) = delete;
  auto operator=(const Interrupt&) -> Interrupt& = delete;
  auto operator=(Interrupt&&) -> Interrupt& = delete;
  ~Interrupt() = default;
  /// @}

  void Do(uint64_t cause, uint8_t* context) override;
  void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) override;
  bool SendIpi(uint64_t target_cpu_mask) override;
  bool BroadcastIpi() override;

  __always_inline void SetUP() const { gic_.SetUP(); }
  __always_inline void SPI(uint32_t intid, uint32_t cpuid) const {
    gic_.SPI(intid, cpuid);
  }
  __always_inline void PPI(uint32_t intid, uint32_t cpuid) const {
    gic_.PPI(intid, cpuid);
  }

 private:
  static std::array<InterruptFunc, kMaxInterrupt> interrupt_handlers;

  Gic gic_;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_ARCH_AARCH64_INCLUDE_INTERRUPT_H_ */
