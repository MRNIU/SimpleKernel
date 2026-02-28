/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_BASIC_INFO_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_BASIC_INFO_HPP_

#include <etl/singleton.h>

#include <cstddef>
#include <cstdint>

#include "kernel_log.hpp"
#include "kstd_iostream"

// 引用链接脚本中的变量
/// @see http://wiki.osdev.org/Using_Linker_Script_Values
/// 内核开始
extern "C" void* __executable_start[];
/// 代码段结束
extern "C" void* __etext[];
/// 内核结束
extern "C" void* end[];
/// 内核入口，在 boot.S 中定义
extern "C" void _boot();

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

  /// fdt 地址
  uint64_t fdt_addr;

  /// cpu 核数
  size_t core_count;

  /// 时钟频率
  size_t interval;

  /**
   * 构造函数，在 arch_main.cpp 中定义
   * @param argc 同 _start
   * @param argv 同 _start
   */
  explicit BasicInfo(int argc, const char** argv);

  /// @name 构造/析构函数
  /// @{
  BasicInfo() = default;
  BasicInfo(const BasicInfo&) = default;
  BasicInfo(BasicInfo&&) = default;
  auto operator=(const BasicInfo&) -> BasicInfo& = default;
  auto operator=(BasicInfo&&) -> BasicInfo& = default;
  ~BasicInfo() = default;
  /// @}

  template <typename OStream>
  friend auto operator<<(OStream& ostream, const BasicInfo& basic_info)
      -> OStream& {
    klog::Info("physical_memory_addr: 0x%X, size 0x%X.\n",
               basic_info.physical_memory_addr,
               basic_info.physical_memory_size);
    klog::Info("kernel_addr: 0x%X, size 0x%X.\n", basic_info.kernel_addr,
               basic_info.kernel_size);
    klog::Info("elf_addr: 0x%X\n", basic_info.elf_addr);
    klog::Info("fdt_addr: 0x%X\n", basic_info.fdt_addr);
    klog::Info("core_count: %d\n", basic_info.core_count);
    return ostream;
  }
};

using BasicInfoSingleton = etl::singleton<BasicInfo>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_BASIC_INFO_HPP_
