/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_

#include <cpu_io.h>

#include <cstdint>

#include "expected.hpp"

class InterruptBase {
 public:
  /**
   * @brief 中断/异常处理函数指针
   * @param  cause 中断或异常号
   * @param  context 中断上下文
   * @return uint64_t 返回值，0 成功
   */
  typedef uint64_t (*InterruptFunc)(uint64_t cause,
                                    cpu_io::TrapContext* context);

  /// @name 构造/析构函数
  /// @{
  InterruptBase() = default;
  InterruptBase(const InterruptBase&) = delete;
  InterruptBase(InterruptBase&&) = delete;
  auto operator=(const InterruptBase&) -> InterruptBase& = delete;
  auto operator=(InterruptBase&&) -> InterruptBase& = delete;
  virtual ~InterruptBase() = default;
  /// @}

  /**
   * @brief 执行中断处理
   * @param 不同平台有不同含义
   */
  virtual void Do(uint64_t, cpu_io::TrapContext*) = 0;

  /**
   * @brief 注册中断处理函数
   * @param cause 中断号，不同平台有不同含义
   * @param func 处理函数
   */
  virtual void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) = 0;

  /**
   * @brief 发送 IPI 到指定核心
   * @param target_cpu_mask 目标核心的位掩码
   * @return Expected<void> 成功时返回 void，失败时返回错误
   */
  virtual auto SendIpi(uint64_t target_cpu_mask) -> Expected<void> = 0;

  /**
   * @brief 广播 IPI 到所有其他核心
   * @return Expected<void> 成功时返回 void，失败时返回错误
   */
  virtual auto BroadcastIpi() -> Expected<void> = 0;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_ */
