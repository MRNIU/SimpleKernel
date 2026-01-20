/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <array>
#include <cstdint>
#include <cstring>

#include "apic.h"
#include "basic_info.hpp"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"
#include "sipi.h"
#include "sk_iostream"

namespace {

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

BasicInfo::BasicInfo(int, const char**) {
  physical_memory_addr = 0;
  physical_memory_size = 0;

  kernel_addr = reinterpret_cast<uint64_t>(__executable_start);
  kernel_size = reinterpret_cast<uint64_t>(end) -
                reinterpret_cast<uint64_t>(__executable_start);

  elf_addr = kernel_addr;

  fdt_addr = 0;

  core_count = cpu_io::cpuid::GetLogicalProcessorCount();
}

auto ArchInit(int, const char**) -> int {
  Singleton<BasicInfo>::GetInstance() = BasicInfo(0, nullptr);
  sk_std::cout << Singleton<BasicInfo>::GetInstance();

  // 解析内核 elf 信息
  Singleton<KernelElf>::GetInstance() =
      KernelElf(Singleton<BasicInfo>::GetInstance().elf_addr);

  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  // 初始化 APIC
  Singleton<Apic>::GetInstance() =
      Apic(Singleton<BasicInfo>::GetInstance().core_count);
  Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic();

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}

auto ArchInitSMP(int, const char**) -> int {
  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  Singleton<Apic>::GetInstance().InitCurrentCpuLocalApic();
  return 0;
}

void WakeUpOtherCores() {
  // 填充 sipi_params 结构体
  auto target_sipi_params = reinterpret_cast<sipi_params_t*>(sipi_params);
  target_sipi_params->cr3 = cpu_io::Cr3::Read();

  Singleton<Apic>::GetInstance().StartupAllAps(
      reinterpret_cast<uint64_t>(ap_start16),
      reinterpret_cast<size_t>(ap_start64_end) -
          reinterpret_cast<size_t>(ap_start16),
      kDefaultAPBase);
}

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     void (*entry)(void*), void* arg, uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  /// @todo x86_64 实现待补充
  (void)task_context;
  (void)entry;
  (void)arg;
  (void)stack_top;
}

void InitTaskContext(cpu_io::CalleeSavedContext* task_context,
                     cpu_io::TrapContext* trap_context_ptr,
                     uint64_t stack_top) {
  // 清零上下文
  std::memset(task_context, 0, sizeof(cpu_io::CalleeSavedContext));

  /// @todo x86_64 实现待补充
  (void)task_context;
  (void)trap_context_ptr;
  (void)stack_top;
}
