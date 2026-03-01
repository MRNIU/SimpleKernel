/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <array>
#include <cstdint>
#include <cstring>

#include "basic_info.hpp"
#include "interrupt.h"
#include "kernel.h"
#include "kernel_elf.hpp"
#include "kernel_log.hpp"
#include "kstd_iostream"
#include "per_cpu.hpp"
#include "sipi.h"

namespace {

/// gdt 描述符表，顺序与 cpu_io::GdtrInfo 中的定义一致
std::array<cpu_io::GdtrInfo::SegmentDescriptor, cpu_io::GdtrInfo::kMaxCount>
    kSegmentDescriptors = {
        // 第一个全 0
        cpu_io::GdtrInfo::SegmentDescriptor(),
        // 内核代码段描述符
        cpu_io::GdtrInfo::SegmentDescriptor(
            cpu_io::GdtrInfo::SegmentDescriptor::Type::kCodeExecuteRead,
            cpu_io::GdtrInfo::SegmentDescriptor::S::kCodeData,
            cpu_io::GdtrInfo::SegmentDescriptor::DPL::kRing0,
            cpu_io::GdtrInfo::SegmentDescriptor::P::kPresent,
            cpu_io::GdtrInfo::SegmentDescriptor::AVL::kNotAvailable,
            cpu_io::GdtrInfo::SegmentDescriptor::L::k64Bit),
        // 内核数据段描述符
        cpu_io::GdtrInfo::SegmentDescriptor(
            cpu_io::GdtrInfo::SegmentDescriptor::Type::kDataReadWrite,
            cpu_io::GdtrInfo::SegmentDescriptor::S::kCodeData,
            cpu_io::GdtrInfo::SegmentDescriptor::DPL::kRing0,
            cpu_io::GdtrInfo::SegmentDescriptor::P::kPresent,
            cpu_io::GdtrInfo::SegmentDescriptor::AVL::kNotAvailable,
            cpu_io::GdtrInfo::SegmentDescriptor::L::k64Bit),
        // 用户代码段描述符
        cpu_io::GdtrInfo::SegmentDescriptor(
            cpu_io::GdtrInfo::SegmentDescriptor::Type::kCodeExecuteRead,
            cpu_io::GdtrInfo::SegmentDescriptor::S::kCodeData,
            cpu_io::GdtrInfo::SegmentDescriptor::DPL::kRing3,
            cpu_io::GdtrInfo::SegmentDescriptor::P::kPresent,
            cpu_io::GdtrInfo::SegmentDescriptor::AVL::kNotAvailable,
            cpu_io::GdtrInfo::SegmentDescriptor::L::k64Bit),
        // 用户数据段描述符
        cpu_io::GdtrInfo::SegmentDescriptor(
            cpu_io::GdtrInfo::SegmentDescriptor::Type::kDataReadWrite,
            cpu_io::GdtrInfo::SegmentDescriptor::S::kCodeData,
            cpu_io::GdtrInfo::SegmentDescriptor::DPL::kRing3,
            cpu_io::GdtrInfo::SegmentDescriptor::P::kPresent,
            cpu_io::GdtrInfo::SegmentDescriptor::AVL::kNotAvailable,
            cpu_io::GdtrInfo::SegmentDescriptor::L::k64Bit),
};

cpu_io::GdtrInfo::Gdtr gdtr{
    .limit = (sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
              cpu_io::GdtrInfo::kMaxCount) -
             1,
    .base = kSegmentDescriptors.data(),
};

/// 设置 GDT 和段寄存器
void SetupGdtAndSegmentRegisters() {
  // 设置 gdt
  cpu_io::Gdtr::Write(gdtr);

  // 加载内核数据段描述符
  cpu_io::Ds::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelDataIndex);
  cpu_io::Es::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelDataIndex);
  cpu_io::Fs::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelDataIndex);
  cpu_io::Gs::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelDataIndex);
  cpu_io::Ss::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelDataIndex);
  // 加载内核代码段描述符
  cpu_io::Cs::Write(sizeof(cpu_io::GdtrInfo::SegmentDescriptor) *
                    cpu_io::GdtrInfo::kKernelCodeIndex);
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
  BasicInfoSingleton::create(0, nullptr);
  kstd::cout << BasicInfoSingleton::instance();

  // 解析内核 elf 信息
  KernelElfSingleton::create(BasicInfoSingleton::instance().elf_addr);

  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  klog::Info("Hello x86_64 ArchInit\n");

  return 0;
}

auto ArchInitSMP(int, const char**) -> int {
  // 设置 GDT 和段寄存器
  SetupGdtAndSegmentRegisters();

  InterruptSingleton::instance().apic().InitCurrentCpuLocalApic().or_else(
      [](Error err) -> Expected<void> {
        klog::Err("Failed to initialize APIC for AP: {}\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return std::unexpected(err);
      });
  return 0;
}

void WakeUpOtherCores() {
  // 填充 sipi_params 结构体
  auto target_sipi_params = reinterpret_cast<sipi_params_t*>(sipi_params);
  target_sipi_params->cr3 = cpu_io::Cr3::Read();

  InterruptSingleton::instance().apic().StartupAllAps(
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
