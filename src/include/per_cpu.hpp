/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_

#include <cpu_io.h>
#include <etl/singleton.h>
#include <unistd.h>

#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>

struct TaskControlBlock;
struct CpuSchedData;

namespace per_cpu {

struct PerCpu {
  /// 核心 ID
  size_t core_id;

  /// 当前运行的任务
  TaskControlBlock* running_task = nullptr;
  /// 空闲任务
  TaskControlBlock* idle_task = nullptr;
  /// 调度数据 (RunQueue) 指针
  CpuSchedData* sched_data = nullptr;

  explicit PerCpu(size_t id) : core_id(id) {}

  /// @name 构造/析构函数
  /// @{
  PerCpu() = default;
  PerCpu(const PerCpu&) = default;
  PerCpu(PerCpu&&) = default;
  auto operator=(const PerCpu&) -> PerCpu& = default;
  auto operator=(PerCpu&&) -> PerCpu& = default;
  ~PerCpu() = default;
  /// @}
} __attribute__((aligned(SIMPLEKERNEL_PER_CPU_ALIGN_SIZE)));

static_assert(sizeof(PerCpu) <= SIMPLEKERNEL_PER_CPU_ALIGN_SIZE,
              "PerCpu size should not exceed cache line size");

/// @brief PerCpu 数组单例类型
using PerCpuArraySingleton =
    etl::singleton<std::array<PerCpu, SIMPLEKERNEL_MAX_CORE_COUNT>>;

static __always_inline auto GetCurrentCore() -> PerCpu& {
  return PerCpuArraySingleton::instance()[cpu_io::GetCurrentCoreId()];
}

}  // namespace per_cpu

#endif  // SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_
