
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

// 基本输出实现
namespace {
cpu_io::Serial *serial = nullptr;
}  // namespace
extern "C" void sk_putchar(int c, [[maybe_unused]] void *ctx) {
  if (serial) {
    serial->Write(c);
  }
}

BasicInfo::BasicInfo(int argc, const char **argv) {
  (void)argc;

  auto basic_info = *reinterpret_cast<const BasicInfo *>(argv);
  physical_memory_addr = basic_info.physical_memory_addr;
  physical_memory_size = basic_info.physical_memory_size;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = 0;

  /// @todo 获取 core 数量
  core_count = 1;
}

auto ArchInit(int argc, const char **argv) -> int {
  if (argc != 1) {
    klog::Err("argc != 1 [%d]\n", argc);
    throw;
  }

  Singleton<cpu_io::Serial>::GetInstance() = cpu_io::Serial(cpu_io::kCom1);
  serial = &Singleton<cpu_io::Serial>::GetInstance();

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr,
                Singleton<BasicInfo>::GetInstance().elf_size);

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}

auto ArchInitSMP(int argc, const char **argv) -> int {
  (void)argc;
  (void)argv;
  return 0;
}
