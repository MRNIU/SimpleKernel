/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

#include <cpu_io.h>
#include <etl/singleton.h>

#include <array>
#include <cstdint>

#include "apic.h"
#include "interrupt_base.h"
#include "sk_stdio.h"

/// @brief x86_64 中断控制器实现
class Interrupt final : public InterruptBase {
 public:
  /// @name 构造/析构函数
  /// @{
  Interrupt();
  Interrupt(const Interrupt&) = delete;
  Interrupt(Interrupt&&) = delete;
  auto operator=(const Interrupt&) -> Interrupt& = delete;
  auto operator=(Interrupt&&) -> Interrupt& = delete;
  ~Interrupt() override = default;
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

  /// @name APIC 访问接口
  /// @{
  [[nodiscard]] __always_inline auto apic() -> Apic& { return apic_; }
  [[nodiscard]] __always_inline auto apic() const -> const Apic& {
    return apic_;
  }
  /// @}

  /**
   * @brief 初始化 APIC
   * @param cpu_count CPU 核心数
   */
  auto InitApic(size_t cpu_count) -> void;

  /// 外部中断向量基址（IO APIC IRQ 到 IDT 向量的映射）
  static constexpr uint8_t kExternalVectorBase = 0x20;

  /**
   * @brief 初始化 idtr
   */
  auto SetUpIdtr() -> void;

 private:
  /// 中断处理函数数组
  alignas(4096)
      std::array<InterruptDelegate,
                 cpu_io::IdtrInfo::kInterruptMaxCount> interrupt_handlers_;

  alignas(4096) std::array<cpu_io::IdtrInfo::Idt,
                           cpu_io::IdtrInfo::kInterruptMaxCount> idts_;

  /// APIC 中断控制器实例
  Apic apic_;

  /**
   * @brief 初始化 idtr
   * @note 注意模板展开时的栈溢出
   */
  template <uint8_t no = 0>
  auto SetUpIdtr() -> void;
};

using InterruptSingleton = etl::singleton<Interrupt>;
