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

#include "singleton.hpp"

namespace per_cpu {
class PerCpu {
 public:
  /// 最大 CPU 数
  static constexpr size_t kMaxCoreCount = 4;

  /// 核心 ID
  const size_t core_id_;
  /// 中断嵌套深度
  ssize_t noff_{0};
  /// 在进入调度线程前是否允许中断
  bool intr_enable_{false};

  PerCpu() : core_id_(GetCurrentCoreId()) {}
  explicit PerCpu(size_t id) : core_id_(id) {}

  /// @name 构造/析构函数
  /// @{
  PerCpu(const PerCpu &) = default;
  PerCpu(PerCpu &&) = default;
  auto operator=(const PerCpu &) -> PerCpu & = default;
  auto operator=(PerCpu &&) -> PerCpu & = default;
  ~PerCpu() = default;
  /// @}

 protected:
  /**
   * @brief 获取当前核心 ID
   * @return size_t 当前核心 ID
   */
  [[nodiscard]] virtual __always_inline auto GetCurrentCoreId() -> size_t {
    return cpu_io::GetCurrentCoreId();
  }
};

/// per cpu 数据
// static std::array<PerCpu, PerCpu::kMaxCoreCount> g_per_cpu{};

static __always_inline auto GetCurrentCore() -> PerCpu & {
  return Singleton<std::array<PerCpu, PerCpu::kMaxCoreCount>>::GetInstance()
      [cpu_io::GetCurrentCoreId()];
}

}  // namespace per_cpu

#endif /* SIMPLEKERNEL_SRC_INCLUDE_PER_CPU_HPP_ */
