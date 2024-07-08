
/**
 * @file basic_info.hpp
 * @brief 基础信息
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

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_BASIC_INFO_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_BASIC_INFO_HPP_

#include <cstddef>
#include <cstdint>

#include "singleton.hpp"

struct BasicInfo {
  /// 最大内存表数量
  static constexpr const uint32_t kMemoryMapMaxCount = 256;
  struct MemoryMap {
    /// 内存类型
    enum {
      /// 可用
      kTypeRam = 1,
      /// 保留
      kTypeReserved,
      /// ACPI
      kTypeAcpi,
      /// NV
      kTypeNvs,
      /// 不可用
      kTypeUnUsable
    };
    /// 地址
    uint64_t base_addr;
    /// 长度
    uint64_t length;
    /// 类型
    uint32_t type;
    /// 保留
    uint32_t reserved;
  } memory_map[kMemoryMapMaxCount];
  /// 内存表数量
  uint32_t memory_map_count;

  /// physical_memory 地址
  uint64_t physical_memory_addr;
  /// physical_memory 大小
  size_t physical_memory_size;

  /// kernel 地址
  uint64_t kernel_addr;
  /// kernel 大小
  size_t kernel_size;

  /// elf 地址
  uint64_t elf_addr;
  /// elf 大小
  size_t elf_size;
};

/// 保存基本信息
[[maybe_unused]] static Singleton<BasicInfo> kBasicInfo;

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_BASIC_INFO_HPP_ */
