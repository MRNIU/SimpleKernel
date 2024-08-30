
/**
 * @file arch_main.cpp
 * @brief arch_main cpp
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

#include <elf.h>

#include "basic_info.hpp"
#include "cpu/cpu.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "sk_cstdio"
#include "sk_cstring"

// printf_bare_metal 基本输出实现
/// @note 这里要注意，保证在 serial 初始化之前不能使用 printf
/// 函数，否则会有全局对象依赖问题
static cpu::Serial kSerial(cpu::kCom1);
extern "C" void _putchar(char character) { kSerial.Write(character); }

// 引用链接脚本中的变量
/// @see http://wiki.osdev.org/Using_Linker_Script_Values
/// 内核开始
extern "C" void *__executable_start[];
/// 内核结束
extern "C" void *end[];
BasicInfo::BasicInfo(uint32_t argc, uint8_t *argv) {
  (void)argc;

  auto basic_info = *reinterpret_cast<BasicInfo *>(argv);
  physical_memory_addr = basic_info.physical_memory_addr;
  physical_memory_size = basic_info.physical_memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = basic_info.elf_addr;
  elf_size = basic_info.elf_size;

  fdt_addr = 0;
}

uint32_t ArchInit(uint32_t argc, uint8_t *argv) {
  if (argc != 1) {
    klog::Err("argc != 1 [%d]\n", argc);
    throw;
  }

  kBasicInfo.GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << kBasicInfo.GetInstance();

  // 解析内核 elf 信息
  kKernelElf.GetInstance() = KernelElf(kBasicInfo.GetInstance().elf_addr,
                                       kBasicInfo.GetInstance().elf_size);

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}
