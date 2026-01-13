/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_

#include <cpu_io.h>
#include <unistd.h>

#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>

#include "config.h"
#include "singleton.hpp"

namespace per_cpu {
struct PerCpu {
  /// 核心 ID
  size_t core_id;

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
} __attribute__((aligned(kSimpleKernelPerCpuAlignSize)));

static_assert(sizeof(PerCpu) <= kSimpleKernelPerCpuAlignSize,
              "PerCpu size should not exceed cache line size");

static __always_inline auto GetCurrentCore() -> PerCpu& {
  return Singleton<std::array<PerCpu, kSimpleKernelMaxCoreCount>>::GetInstance()
      [cpu_io::GetCurrentCoreId()];
}

}  // namespace per_cpu

#endif /* SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_ */
