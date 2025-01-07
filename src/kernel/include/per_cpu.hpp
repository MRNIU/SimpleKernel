
/**
 * @file per_cpu.hpp
 * @brief 多核数据结构
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_PER_CPU_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_PER_CPU_HPP_

#include <unistd.h>

#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>

class PerCpu {
 public:
  /// 最大 CPU 数
  static constexpr size_t kMaxCoreCount = 4;

  /// 核心 ID
  size_t core_id_;
  /// 中断嵌套深度
  ssize_t noff_;
  /// 在进入调度线程前是否允许中断
  bool intr_enable_;

  PerCpu() {
    core_id_ = SIZE_MAX;
    noff_ = 0;
    intr_enable_ = false;
  }
  // explicit PerCpu(size_t core_id) : core_id_(core_id) {}

  /// @name 构造/析构函数
  /// @{
  PerCpu(const PerCpu &) = default;
  PerCpu(PerCpu &&) = default;
  auto operator=(const PerCpu &) -> PerCpu & = default;
  auto operator=(PerCpu &&) -> PerCpu & = default;
  ~PerCpu() = default;
  /// @}
};

/// per cpu 数据
static std::array<PerCpu, PerCpu::kMaxCoreCount> g_per_cpu{};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_PER_CPU_HPP_ */
