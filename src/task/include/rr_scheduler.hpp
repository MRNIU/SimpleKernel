/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_

#include "scheduler_base.hpp"
#include "sk_list"
#include "task_control_block.hpp"

/**
 * @brief Round-Robin 调度器
 *
 * 时间片轮转调度器，所有任务按照 FIFO 顺序排队。
 * 每个任务获得相同的时间片，时间片用完后放回队列尾部。
 */
class RoundRobinScheduler : public SchedulerBase {
 public:
  /**
   * @brief 将任务加入就绪队列尾部
   * @param task 任务控制块指针
   */
  void Enqueue(TaskControlBlock* task) override {
    if (task) {
      ready_queue.push_back(task);
    }
  }

  /**
   * @brief 从就绪队列中移除指定任务
   * @param task 要移除的任务控制块指针
   *
   * 用于任务主动退出或被阻塞等场景。
   */
  void Dequeue(TaskControlBlock* task) override {
    if (!task) {
      return;
    }

    for (auto it = ready_queue.begin(); it != ready_queue.end(); ++it) {
      if (*it == task) {
        ready_queue.erase(it);
        break;
      }
    }
  }

  /**
   * @brief 选择下一个要运行的任务
   * @return TaskControlBlock* 下一个任务，如果队列为空则返回 nullptr
   *
   * 从队列头部取出任务，实现 Round-Robin 轮转。
   */
  TaskControlBlock* PickNext() override {
    if (ready_queue.empty()) {
      return nullptr;
    }
    auto next = ready_queue.front();
    ready_queue.pop_front();
    return next;
  }

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
  /// 就绪队列 (双向链表，支持从头部取、向尾部放)
  sk_std::list<TaskControlBlock*> ready_queue;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_ */
