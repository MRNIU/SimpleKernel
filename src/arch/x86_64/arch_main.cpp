/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief arch_main cpp
 */

#include <cpu_io.h>

#include <array>
#include <cstdint>

#include "apic.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sipi.h"
#include "sk_iostream"

namespace {
// 基本输出实现
cpu_io::Serial *serial = nullptr;

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

cpu_io::detail::register_info::GdtrInfo::Gdtr gdtr{
    .limit =
        (sizeof(cpu_io::detail::register_info::GdtrInfo::SegmentDescriptor) *
         cpu_io::detail::register_info::GdtrInfo::kMaxCount) -
        1,
    .base = kSegmentDescriptors.data(),
};

/// 设置 GDT 和段寄存器
void SetupGdtAndSegmentRegisters() {
  // 设置 gdt
  cpu_io::Gdtr::Write(gdtr);

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
}

}  // namespace

extern "C" void sk_putchar(int c, [[maybe_unused]] void *ctx) {
  if (serial) {
    serial->Write(c);
  }
}

BasicInfo::BasicInfo(int, const char **) {
  physical_memory_addr = 0;
  physical_memory_size = 0;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = kernel_addr;
  elf_size = kernel_size;

  fdt_addr = 0;

  core_count = cpu_io::cpuid::GetLogicalProcessorCount();
}

auto ArchInit(int, const char **) -> int {
  Singleton<cpu_io::Serial>::GetInstance() = cpu_io::Serial(cpu_io::kCom1);
  serial = &Singleton<cpu_io::Serial>::GetInstance();

  Singleton<BasicInfo>::GetInstance() = BasicInfo(0, nullptr);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr,
                Singleton<BasicInfo>::GetInstance().elf_size);

  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  // 初始化 APIC
  Singleton<Apic>::GetInstance() =
      Apic(Singleton<BasicInfo>::GetInstance().core_count);
  Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic();

  klog::Info("Hello x86_64 ArchInit\n");

  // 填充 sipi_params 结构体
  auto target_sipi_params = reinterpret_cast<sipi_params_t *>(sipi_params);
  target_sipi_params->cr3 = cpu_io::Cr3::Read();

  // 唤醒其它 core
  Singleton<Apic>::GetInstance().StartupAllAps(
      reinterpret_cast<uint64_t>(ap_start16),
      reinterpret_cast<size_t>(ap_start64_end) -
          reinterpret_cast<size_t>(ap_start16),
      kDefaultAPBase);

  return 0;
}

auto ArchInitSMP(int, const char **) -> int {
  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic();
  return 0;
}
