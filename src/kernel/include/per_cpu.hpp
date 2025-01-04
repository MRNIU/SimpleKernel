
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

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdlib>

struct PerCpu {
  /// 核心 ID
  size_t core_id;
};

/// 最多支持 4 个核心
constexpr size_t kMaxCoreCount = 8;

/// per cpu 数据
static std::array<PerCpu, kMaxCoreCount> kPerCpu;

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_PER_CPU_HPP_ */
