/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <cpu_io.h>
#include <etl/singleton.h>

#include <cstdint>

#include "gic.h"
#include "interrupt_base.h"
#include "sk_stdio.h"

/// @brief AArch64 中断控制器实现
class Interrupt final : public InterruptBase {
 public:
  static constexpr size_t kMaxInterrupt = 128;

  /// @name 构造/析构函数
  /// @{
  Interrupt();
  Interrupt(const Interrupt&) = delete;
  Interrupt(Interrupt&&) = delete;
  auto operator=(const Interrupt&) -> Interrupt& = delete;
  auto operator=(Interrupt&&) -> Interrupt& = delete;
  ~Interrupt() = default;
  /// @}

  /** @brief 执行中断处理 */
  auto Do(uint64_t cause, cpu_io::TrapContext* context) -> void override;
  /** @brief 注册中断处理函数 */
  auto RegisterInterruptFunc(uint64_t cause, InterruptDelegate func)
      -> void override;
  [[nodiscard]] auto SendIpi(uint64_t target_cpu_mask)
      -> Expected<void> override;
  [[nodiscard]] auto BroadcastIpi() -> Expected<void> override;
  [[nodiscard]] auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                               uint32_t priority,
                                               InterruptDelegate handler)
      -> Expected<void> override;

  /** @brief 设置 GIC */
  __always_inline auto SetUP() const -> void { gic_.SetUP(); }
  /** @brief 设置 SPI 中断 */
  __always_inline auto SPI(uint32_t intid, uint32_t cpuid) const -> void {
    gic_.SPI(intid, cpuid);
  }
  /** @brief 设置 PPI 中断 */
  __always_inline auto PPI(uint32_t intid, uint32_t cpuid) const -> void {
    gic_.PPI(intid, cpuid);
  }
  /** @brief 设置 SGI 中断 */
  __always_inline auto SGI(uint32_t intid, uint32_t cpuid) const -> void {
    gic_.SGI(intid, cpuid);
  }

 private:
  std::array<InterruptDelegate, kMaxInterrupt> interrupt_handlers_;

  Gic gic_;
};

using InterruptSingleton = etl::singleton<Interrupt>;
