/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_

#include <cstddef>

namespace kernel::config {

// ── 任务管理容量 ────────────────────────────────────────────────
/// 全局最大任务数（task_table_ 容量）
inline constexpr size_t kMaxTasks = 128;
/// task_table_ 桶数（建议 = 2 × kMaxTasks）
inline constexpr size_t kMaxTasksBuckets = 256;

/// 每个 CPU 的最大睡眠任务数（sleeping_tasks 容量）
inline constexpr size_t kMaxSleepingTasks = 64;

/// 阻塞队列：最大资源组数（blocked_tasks 的 map 容量）
inline constexpr size_t kMaxBlockedGroups = 32;
/// 阻塞队列：map 桶数
inline constexpr size_t kMaxBlockedGroupsBuckets = 64;
/// 阻塞队列：每组最大阻塞任务数（etl::list 容量）
inline constexpr size_t kMaxBlockedPerGroup = 16;

/// 调度器就绪队列容量（FIFO / RR / CFS）
inline constexpr size_t kMaxReadyTasks = 64;

// ── 中断线程映射容量 ─────────────────────────────────────────────
/// 最大中断线程数
inline constexpr size_t kMaxInterruptThreads = 32;
/// 中断线程 map 桶数
inline constexpr size_t kMaxInterruptThreadsBuckets = 64;

}  // namespace kernel::config

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_CONFIG_HPP_
