/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_CFS_SCHEDULER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_CFS_SCHEDULER_HPP_

#include <etl/priority_queue.h>
#include <etl/vector.h>

#include <cassert>
#include <cstdint>

#include "kernel_config.hpp"
#include "kernel_log.hpp"
#include "scheduler_base.hpp"
#include "task_control_block.hpp"

/**
 * @brief CFS (Completely Fair Scheduler) 调度器
 *
 * 基于虚拟运行时间 (vruntime) 的完全公平调度器。
 * 每个任务根据其权重累积 vruntime，调度器总是选择 vruntime 最小的任务。
 *
 * 特点：
 * - 完全公平：每个任务获得与其权重成正比的 CPU 时间
 * - 支持优先级：通过权重影响 vruntime 增长速度
 * - 防止饥饿：新任务的 vruntime 设置为 min_vruntime
 * - 动态抢占：在 OnTick 中检查是否需要抢占当前任务
 */
class CfsScheduler : public SchedulerBase {
 public:
  /// 默认权重 (对应 nice 值为 0)
  static constexpr uint32_t kDefaultWeight = 1024;

  /// vruntime 粒度 (用于计算抢占阈值)
  static constexpr uint64_t kMinGranularity = 10;  // 10 ticks

  /**
   * @brief vruntime 比较器 (用于优先队列)
   *
   * 按 vruntime 从小到大排序，确保最小 vruntime 的任务在队列顶部。
   */
  struct VruntimeCompare {
    auto operator()(const TaskControlBlock* a, const TaskControlBlock* b) const
        -> bool {
      return a->sched_data.cfs.vruntime > b->sched_data.cfs.vruntime;
    }
  };

  /**
   * @brief 将任务加入就绪队列
   * @param task 任务控制块指针
   *
   * 新加入的任务的 vruntime 设置为当前 min_vruntime，防止饥饿。
   */
  void Enqueue(TaskControlBlock* task) override {
    if (!task) {
      return;
    }

    // 如果是新任务 (vruntime == 0)，设置为当前 min_vruntime
    if (task->sched_data.cfs.vruntime == 0) {
      task->sched_data.cfs.vruntime = min_vruntime_;
    }

    // 确保任务有合理的权重
    if (task->sched_data.cfs.weight == 0) {
      task->sched_data.cfs.weight = kDefaultWeight;
    }

    if (ready_queue_.full()) {
      klog::Err("CfsScheduler::Enqueue: ready_queue_ full, dropping task\n");
      return;
    }
    ready_queue_.push(task);
    stats_.total_enqueues++;
  }

  /**
   * @brief 从就绪队列中移除指定任务
   * @param task 要移除的任务控制块指针
   *
   * 注意：优先队列不支持高效的任意元素删除，这里需要重建队列。
   * 实际实现中建议使用红黑树替代优先队列以支持 O(log n) 删除。
   */
  void Dequeue(TaskControlBlock* task) override {
    if (!task) {
      return;
    }

    // 临时向量用于重建队列
    etl::vector<TaskControlBlock*, kernel::config::kMaxReadyTasks> temp;
    bool found = false;

    // 将所有元素弹出，除了要删除的任务
    while (!ready_queue_.empty()) {
      auto* t = ready_queue_.top();
      ready_queue_.pop();
      if (t == task) {
        found = true;
      } else {
        if (!temp.full()) {
        }

        // 重建队列
        for (auto* t : temp) {
          if (!ready_queue_.full()) {
            ready_queue_.push(t);
          }

          if (found) {
            stats_.total_dequeues++;
          }
        }

        /**
         * @brief 选择下一个要运行的任务
         * @return TaskControlBlock* 下一个任务，如果队列为空则返回 nullptr
         *
         * 选择 vruntime 最小的任务，实现完全公平调度。
         */
        TaskControlBlock* PickNext() override {
          if (ready_queue_.empty()) {
            return nullptr;
          }

          auto* next = ready_queue_.top();
          ready_queue_.pop();
          stats_.total_picks++;

          // 更新 min_vruntime 为队列中最小的 vruntime
          if (!ready_queue_.empty()) {
            min_vruntime_ = ready_queue_.top()->sched_data.cfs.vruntime;
          } else {
            min_vruntime_ = next->sched_data.cfs.vruntime;
          }

          return next;
        }

        /**
         * @brief 获取就绪队列大小
         * @return size_t 队列中的任务数量
         */
        auto GetQueueSize() const -> size_t override {
          return ready_queue_.size();
        }

        /**
         * @brief 判断队列是否为空
         * @return bool 队列为空返回 true
         */
        auto IsEmpty() const -> bool override { return ready_queue_.empty(); }

        /**
         * @brief 每个 tick 更新任务的 vruntime
         * @param current 当前运行的任务
         * @return bool 返回 true 表示需要抢占当前任务
         *
         * CFS 核心逻辑：
         * 1. 更新当前任务的 vruntime（考虑权重）
         * 2. 检查是否有 vruntime 更小的任务需要抢占
         */
        auto OnTick(TaskControlBlock * current) -> bool override {
          assert(current != nullptr &&
                 "CfsScheduler::OnTick: current task must not be null");

          // 更新 vruntime：delta = tick * (kDefaultWeight / weight)
          // 权重越大，vruntime 增长越慢，获得更多 CPU 时间
          uint64_t delta =
              (kDefaultWeight * 1000) / current->sched_data.cfs.weight;
          current->sched_data.cfs.vruntime += delta;

          // 检查是否需要抢占：队列中有 vruntime 更小的任务
          if (!ready_queue_.empty()) {
            auto* next = ready_queue_.top();
            // 如果队列顶部任务的 vruntime 比当前任务小超过阈值，则需要抢占
            if (next->sched_data.cfs.vruntime + kMinGranularity <
                current->sched_data.cfs.vruntime) {
              stats_.total_preemptions++;
              return true;  // 需要抢占
            }
          }

          return false;  // 不需要抢占
        }

        /**
         * @brief 任务被抢占时调用
         * @param task 被抢占的任务
         *
         * 被抢占的任务需要重新入队，保持其 vruntime。
         */
        void OnPreempted(TaskControlBlock * task) override {
          if (task) {
            stats_.total_preemptions++;
            // CFS 不需要额外处理，Schedule() 会将任务重新入队
          }
        }

        /**
         * @brief 获取当前 min_vruntime
         * @return uint64_t 最小虚拟运行时间
         */
        auto GetMinVruntime() const -> uint64_t { return min_vruntime_; }

        /// @name 构造/析构函数
        /// @{
        CfsScheduler() = default;
        CfsScheduler(const CfsScheduler&) = delete;
        CfsScheduler(CfsScheduler&&) = delete;
        auto operator=(const CfsScheduler&)->CfsScheduler& = delete;
        auto operator=(CfsScheduler&&)->CfsScheduler& = delete;
        ~CfsScheduler() override = default;
        /// @}

       private:
        /// 就绪队列 (优先队列，按 vruntime 排序)
        etl::priority_queue<TaskControlBlock*, kernel::config::kMaxReadyTasks,
                            VruntimeCompare>
            ready_queue_;

        /// 当前最小 vruntime (用于新任务初始化)
        uint64_t min_vruntime_ = 0;
      };

#endif  // SIMPLEKERNEL_SRC_INCLUDE_SCHEDULER_CFS_SCHEDULER_HPP_
