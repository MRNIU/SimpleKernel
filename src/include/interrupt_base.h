/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 中断处理接口
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_

#include <atomic>
#include <cstdint>
#include <functional>

// 供汇编调用：上下文切换
extern "C" void switch_to(cpu_io::CalleeSavedContext* prev,
                          cpu_io::CalleeSavedContext* next);

// 汇编入口跳板
extern "C" void kernel_thread_entry();

extern "C" void trap_return(void*);

// 在 trap.S 中定义
extern "C" void trap_entry();

class InterruptBase {
 public:
  /// @todo functional 报错
  // typedef std::function<uint64_t(uint64_t, uint8_t *)> InterruptFunc;
  /**
   * @brief 中断/异常处理函数指针
   * @param  cause 中断或异常号
   * @param  context 中断上下文
   * @return uint64_t 返回值，0 成功
   */
  typedef uint64_t (*InterruptFunc)(uint64_t cause, uint8_t* context);

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
  virtual void Do(uint64_t, uint8_t*) = 0;

  /**
   * @brief 注册中断处理函数
   * @param cause 中断号，不同平台有不同含义
   * @param func 处理函数
   */
  virtual void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) = 0;
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_INTERRUPT_BASE_H_ */
