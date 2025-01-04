
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

#include "kernel_log.hpp"
#include "singleton.hpp"
#include "sk_iostream"

struct BasicInfo {
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

  /// fdt 地址
  uint64_t fdt_addr;

  /// cpu 核数
  size_t core_count;

  /**
   * 构造函数，在 arch_main.cpp 中定义
   * @param argc 同 _start
   * @param argv 同 _start
   */
  explicit BasicInfo(uint32_t argc, const uint8_t *argv);

  /// @name 构造/析构函数
  /// @{
  BasicInfo() = default;
  BasicInfo(const BasicInfo &) = default;
  BasicInfo(BasicInfo &&) = default;
  auto operator=(const BasicInfo &) -> BasicInfo & = default;
  auto operator=(BasicInfo &&) -> BasicInfo & = default;
  ~BasicInfo() = default;
  /// @}

  friend auto operator<<(sk_std::ostream &ostream, const BasicInfo &basic_info)
      -> sk_std::ostream & {
    klog::Info("physical_memory_addr: 0x%X, size 0x%X.\n",
               basic_info.physical_memory_addr,
               basic_info.physical_memory_size);
    klog::Info("kernel_addr: 0x%X, size 0x%X.\n", basic_info.kernel_addr,
               basic_info.kernel_size);
    klog::Info("elf_addr: 0x%X, size 0x%X\n", basic_info.elf_addr,
               basic_info.elf_size);
    klog::Info("fdt_addr: 0x%X\n", basic_info.fdt_addr);
    return ostream;
  }
};

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_BASIC_INFO_HPP_ */
