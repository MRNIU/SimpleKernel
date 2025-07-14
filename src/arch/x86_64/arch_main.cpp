
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

#include <array>
#include <cstdint>

#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sk_iostream"

namespace {
// 基本输出实现
/// @note 这里要注意，保证在 serial 初始化之前不能使用 printf
/// 函数，否则会有全局对象依赖问题
cpu_io::Serial kSerial(cpu_io::kCom1);
extern "C" void sk_putchar(int c, [[maybe_unused]] void *ctx) {
  kSerial.Write(c);
}

/// gdt 描述符表，顺序与 cpu_io::detail::register_info::GdtrInfo 中的定义一致
std::array<cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor,
           cpu_io::detail::register_info::GdtrInfo::kMaxCount>
    kSegmentDescriptors = {
        // 第一个全 0
        cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor(),
        // 内核代码段描述符
        cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor(
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::Type::
                kCodeExecuteRead,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::S::
                kCodeData,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::DPL::
                kRing0,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::P::
                kPresent,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::AVL::
                kNotAvailable,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::L::
                k64Bit),
        // 内核数据段描述符
        cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor(
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::Type::
                kDataReadWrite,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::S::
                kCodeData,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::DPL::
                kRing0,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::P::
                kPresent,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::AVL::
                kNotAvailable,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::L::
                k64Bit),
        // 用户代码段描述符
        cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor(
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::Type::
                kCodeExecuteRead,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::S::
                kCodeData,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::DPL::
                kRing3,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::P::
                kPresent,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::AVL::
                kNotAvailable,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::L::
                k64Bit),
        // 用户数据段描述符
        cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor(
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::Type::
                kDataReadWrite,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::S::
                kCodeData,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::DPL::
                kRing3,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::P::
                kPresent,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::AVL::
                kNotAvailable,
            cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor::L::
                k64Bit),
};

}  // namespace

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

  Singleton<BasicInfo>::GetInstance() = BasicInfo(argc, argv);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr,
                Singleton<BasicInfo>::GetInstance().elf_size);

  // 加载描述符
  cpu_io::detail::register_info::GdtrInfo::Gdtr gdtr{
      .limit =
          (sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
           cpu_io::detail::register_info::GdtrInfo::kMaxCount) -
          1,
      .base = kSegmentDescriptors.data(),
  };
  cpu_io::Gdtr::Write(gdtr);

  klog::Debug(
      "sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor): "
      "%d\n",
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor));
  klog::Debug("kSegmentDescriptors: 0x%X\n", kSegmentDescriptors);

  // 加载内核数据段描述符
  cpu_io::Ds::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelDataIndex);
  cpu_io::Es::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelDataIndex);
  cpu_io::Fs::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelDataIndex);
  cpu_io::Gs::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelDataIndex);
  cpu_io::Ss::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelDataIndex);
  // 加载内核代码段描述符
  cpu_io::Cs::Write(
      sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
      cpu_io::detail::register_info::GdtrInfo::kKernelCodeIndex);

  //   sk_std::cout << "es: " << cpu_io::kAllCr.es << sk_std::endl;
  //   sk_std::cout << "cs: " << cpu_io::kAllCr.cs << sk_std::endl;
  //   sk_std::cout << "ss: " << cpu_io::kAllCr.ss << sk_std::endl;
  //   sk_std::cout << "ds: " << cpu_io::kAllCr.ds << sk_std::endl;
  //   sk_std::cout << "fs: " << cpu_io::kAllCr.fs << sk_std::endl;
  //   sk_std::cout << "gs: " << cpu_io::kAllCr.gs << sk_std::endl;

  for (size_t i = 0;
       i <
       (cpu_io::Gdtr::Read().limit + 1) /
           sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor);
       i++) {
    klog::Debug("gdtr[%d] 0x%p\n", i, cpu_io::Gdtr::Read().base + i);
    // klog::debug << *(cpu_io::kAllCr.gdtr.Read().base + i) << sk_std::endl;
  }

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}

auto ArchInitSMP(int argc, const char **argv) -> int {
  (void)argc;
  (void)argv;
  return 0;
}
