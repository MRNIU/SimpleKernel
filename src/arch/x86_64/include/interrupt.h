/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断处理
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_

#include <cpu_io.h>
#include <etl/singleton.h>

#include <array>
#include <cstdint>

#include "apic.h"
#include "interrupt_base.h"
#include "sk_stdio.h"

class Interrupt final : public InterruptBase {
 public:
  Interrupt();

  /// @name 构造/析构函数
  /// @{
  Interrupt(const Interrupt&) = delete;
  Interrupt(Interrupt&&) = delete;
  auto operator=(const Interrupt&) -> Interrupt& = delete;
  auto operator=(Interrupt&&) -> Interrupt& = delete;
  ~Interrupt() override = default;
  /// @}

  void Do(uint64_t cause, cpu_io::TrapContext* context) override;
  void RegisterInterruptFunc(uint64_t cause, InterruptDelegate func) override;
  auto SendIpi(uint64_t target_cpu_mask) -> Expected<void> override;
  auto BroadcastIpi() -> Expected<void> override;
  auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                 uint32_t priority, InterruptDelegate handler)
      -> Expected<void> override;

  /// @name APIC 访问接口
  /// @{
  __always_inline auto apic() -> Apic& { return apic_; }
  __always_inline auto apic() const -> const Apic& { return apic_; }
  /// @}

  /**
   * @brief 初始化 APIC
   * @param cpu_count CPU 核心数
   */
  void InitApic(size_t cpu_count);

  /// 外部中断向量基址（IO APIC IRQ 到 IDT 向量的映射）
  static constexpr uint8_t kExternalVectorBase = 0x20;

  /**
   * @brief 初始化 idtr
   */
  void SetUpIdtr();

 private:
  /// 中断处理函数数组
  alignas(4096)
      std::array<InterruptDelegate, cpu_io::detail::register_info::IdtrInfo::
                                        kInterruptMaxCount> interrupt_handlers_;

  alignas(4096) std::array<
      cpu_io::detail::register_info::IdtrInfo::Idt,
      cpu_io::detail::register_info::IdtrInfo::kInterruptMaxCount> idts_;

  /// APIC 中断控制器实例
  Apic apic_;

  /**
   * @brief 初始化 idtr
   * @note 注意模板展开时的栈溢出
   */
  template <uint8_t no = 0>
  void SetUpIdtr();
};

using InterruptSingleton = etl::singleton<Interrupt>;

#endif  // SIMPLEKERNEL_SRC_KERNEL_ARCH_X86_64_INTERRUPT_H_
