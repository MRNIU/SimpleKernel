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
   *
   * 重置任务的时间片并将其加入队列尾部，实现公平的时间片轮转。
   */
  void Enqueue(TaskControlBlock* task) override {
    if (task) {
      // 重新分配时间片
      task->sched_info.time_slice_remaining =
          task->sched_info.time_slice_default;
      ready_queue.push_back(task);
      stats_.total_enqueues++;
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
        stats_.total_dequeues++;
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
    stats_.total_picks++;
    return next;
  }

  /**
   * @brief 获取就绪队列大小
   * @return size_t 队列中的任务数量
   */
  auto GetQueueSize() const -> size_t override { return ready_queue.size(); }

  /**
   * @brief 判断队列是否为空
   * @return bool 队列为空返回 true
   */
  auto IsEmpty() const -> bool override { return ready_queue.empty(); }

  /**
   * @brief 时间片耗尽处理
   * @param task 时间片耗尽的任务
   * @return bool 返回 true 表示需要重新入队
   *
   * Round-Robin 调度器在时间片耗尽时重置时间片并将任务放回队列尾部。
   */
  auto OnTimeSliceExpired(TaskControlBlock* task) -> bool override {
    if (task) {
      // 重新分配时间片
      task->sched_info.time_slice_remaining =
          task->sched_info.time_slice_default;
    }
    return true;
  }

  /**
   * @brief 任务被抢占时调用
   * @param task 被抢占的任务
   */
  void OnPreempted([[maybe_unused]] TaskControlBlock* task) override {
    stats_.total_preemptions++;
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

  /// 统计信息
  Stats stats_;
};

#endif /* SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_RR_SCHEDULER_HPP_ */
