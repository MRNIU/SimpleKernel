/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_SCHEDULER_BASE_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_SCHEDULER_BASE_HPP_

struct TaskControlBlock;

/**
 * @brief 调度器接口
 */
class SchedulerBase {
 public:
  // 添加任务到就绪队列
  virtual void Enqueue(TaskControlBlock*) = 0;
  // 从就绪队列移除任务 (可选实现)
  virtual void Dequeue(TaskControlBlock*) {}
  // 选择下一个要运行的任务
  virtual TaskControlBlock* PickNext() = 0;

  /// @name 构造/析构函数
  /// @{
  SchedulerBase() = default;
  SchedulerBase(const SchedulerBase&) = default;
  SchedulerBase(SchedulerBase&&) = default;
  auto operator=(const SchedulerBase&) -> SchedulerBase& = default;
  auto operator=(SchedulerBase&&) -> SchedulerBase& = default;
  virtual ~SchedulerBase() = default;
  /// @}
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_SCHEDULER_BASE_HPP_ */
