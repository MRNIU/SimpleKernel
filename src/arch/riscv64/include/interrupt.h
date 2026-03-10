/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <cpu_io.h>
#include <etl/singleton.h>

#include <array>
#include <cstdint>

#include "interrupt_base.h"
#include "plic.h"
#include "sk_stdio.h"

/// @brief RISC-V 中断控制器实现
class Interrupt final : public InterruptBase {
 public:
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

  /// @name PLIC 访问接口
  /// @{
  [[nodiscard]] __always_inline auto plic() -> Plic& { return plic_; }
  [[nodiscard]] __always_inline auto plic() const -> const Plic& {
    return plic_;
  }

  /**
   * @brief 初始化 PLIC
   * @param dev_addr PLIC 设备地址
   * @param ndev 支持的中断源数量
   * @param context_count 上下文数量
   */
  auto InitPlic(uint64_t dev_addr, size_t ndev, size_t context_count) -> void;
  /// @}

 private:
  /// 中断处理函数数组
  std::array<InterruptDelegate, cpu_io::ScauseInfo::kInterruptMaxCount>
      interrupt_handlers_;
  /// 异常处理函数数组
  std::array<InterruptDelegate, cpu_io::ScauseInfo::kExceptionMaxCount>
      exception_handlers_;
  Plic plic_;
};

using InterruptSingleton = etl::singleton<Interrupt>;
