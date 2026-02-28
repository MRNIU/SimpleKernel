/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断处理
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_ARCH_RISCV64_INTERRUPT_H_
#define SIMPLEKERNEL_SRC_KERNEL_ARCH_RISCV64_INTERRUPT_H_

#include <cpu_io.h>
#include <etl/singleton.h>

#include <array>
#include <cstdint>

#include "interrupt_base.h"
#include "plic.h"
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
  ~Interrupt() = default;
  /// @}

  void Do(uint64_t cause, cpu_io::TrapContext* context) override;
  void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) override;
  auto SendIpi(uint64_t target_cpu_mask) -> Expected<void> override;
  auto BroadcastIpi() -> Expected<void> override;
  auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                 uint32_t priority, InterruptFunc handler)
      -> Expected<void> override;

  /// @name PLIC 访问接口
  /// @{
  __always_inline auto plic() -> Plic& { return plic_; }
  __always_inline auto plic() const -> const Plic& { return plic_; }

  /**
   * @brief 初始化 PLIC
   * @param dev_addr PLIC 设备地址
   * @param ndev 支持的中断源数量
   * @param context_count 上下文数量
   */
  void InitPlic(uint64_t dev_addr, size_t ndev, size_t context_count);
  /// @}

 private:
  /// 中断处理函数数组
  std::array<InterruptFunc,
             cpu_io::detail::register_info::csr::ScauseInfo::kInterruptMaxCount>
      interrupt_handlers_;
  /// 异常处理函数数组
  std::array<InterruptFunc,
             cpu_io::detail::register_info::csr::ScauseInfo::kExceptionMaxCount>
      exception_handlers_;
  Plic plic_;
};

using InterruptSingleton = etl::singleton<Interrupt>;

#endif /* SIMPLEKERNEL_SRC_KERNEL_ARCH_RISCV64_INTERRUPT_H_ */
