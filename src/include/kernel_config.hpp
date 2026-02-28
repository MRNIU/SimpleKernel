/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_

#include <cstddef>

#include "project_config.h"

namespace kernel::config {

/// 全局最大任务数（task_table_ 容量）
static constexpr size_t kMaxTasks = 128;
/// task_table_ 桶数（建议 = 2 × kMaxTasks）
static constexpr size_t kMaxTasksBuckets = 256;

/// 每个 CPU 的最大睡眠任务数（sleeping_tasks 容量）
static constexpr size_t kMaxSleepingTasks = 64;

/// 阻塞队列：最大资源组数（blocked_tasks 的 map 容量）
static constexpr size_t kMaxBlockedGroups = 32;
/// 阻塞队列：map 桶数
static constexpr size_t kMaxBlockedGroupsBuckets = 64;
/// 阻塞队列：每组最大阻塞任务数（etl::list 容量）
static constexpr size_t kMaxBlockedPerGroup = 16;

/// 调度器就绪队列容量（FIFO / RR / CFS）
static constexpr size_t kMaxReadyTasks = 64;

/// 最大中断线程数
static constexpr size_t kMaxInterruptThreads = 32;
/// 中断线程 map 桶数
static constexpr size_t kMaxInterruptThreadsBuckets = 64;

/// 最大 tick 观察者数
static constexpr size_t kTickObservers = 8;
/// 最大 panic 观察者数
static constexpr size_t kPanicObservers = 4;
}  // namespace kernel::config

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
