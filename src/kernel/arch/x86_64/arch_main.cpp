
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

#include <cpu_io.h>

#include <cstdint>

#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sk_iostream"

// printf_bare_metal 基本输出实现
/// @note 这里要注意，保证在 serial 初始化之前不能使用 printf
/// 函数，否则会有全局对象依赖问题
namespace {
cpu_io::Serial kSerial(cpu_io::kCom1);
extern "C" void _putchar(char character) { kSerial.Write(character); }
}  // namespace

BasicInfo::BasicInfo(uint32_t argc, const uint8_t *argv) {
  (void)argc;

  auto basic_info = *reinterpret_cast<const BasicInfo *>(argv);
  physical_memory_addr = basic_info.physical_memory_addr;
  physical_memory_size = basic_info.physical_memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = basic_info.elf_addr;
  elf_size = basic_info.elf_size;

  fdt_addr = 0;
}

auto ArchInit(uint32_t argc, const uint8_t *argv) -> uint32_t {
  if (argc != 1) {
    klog::Err("argc != 1 [%d]\n", argc);
    throw;
  }

  GetCurrentCore().core_id_ = cpu_io::GetCurrentCoreId();

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  Singleton<BasicInfo>::GetInstance().core_count++;
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr,
                Singleton<BasicInfo>::GetInstance().elf_size);

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}

auto ArchInitSMP(uint32_t argc, const uint8_t *argv) -> uint32_t {
  (void)argc;
  (void)argv;
  return 0;
}
