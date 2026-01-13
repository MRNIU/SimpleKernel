/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_

#include <MPMCQueue.hpp>

#include "scheduler_base.hpp"

/**
 * @brief Round-Robin 调度器
 */
class RoundRobinScheduler : public SchedulerBase {
 public:
  void Enqueue(TaskControlBlock* task) override { ready_queue.push(task); }
  void Dequeue(TaskControlBlock* task) override {}

  TaskControlBlock* PickNext() override { return ready_queue.pop(); }

  /// @name 构造/析构函数
  /// @{
  RoundRobinScheduler() = default;
  RoundRobinScheduler(const RoundRobinScheduler&) = default;
  RoundRobinScheduler(RoundRobinScheduler&&) = default;
  auto operator=(const RoundRobinScheduler&) -> RoundRobinScheduler& = default;
  auto operator=(RoundRobinScheduler&&) -> RoundRobinScheduler& = default;
  ~RoundRobinScheduler() override = default;
  /// @}

 private:
  mpmc_queue::MPMCQueue<TaskControlBlock*, 1024> ready_queue;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_ */
