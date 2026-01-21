/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "task_control_block.hpp"

#include <cpu_io.h>
#include <elf.h>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt_base.h"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "sk_cstring"
#include "sk_stdlib.h"
#include "sk_vector"
#include "virtual_memory.hpp"

namespace {

uint64_t LoadElf(const uint8_t* elf_data, uint64_t* page_table) {
  // Check ELF magic
  auto* ehdr = reinterpret_cast<const Elf64_Ehdr*>(elf_data);
  if (ehdr->e_ident[EI_MAG0] != ELFMAG0 || ehdr->e_ident[EI_MAG1] != ELFMAG1 ||
      ehdr->e_ident[EI_MAG2] != ELFMAG2 || ehdr->e_ident[EI_MAG3] != ELFMAG3) {
    klog::Err("Invalid ELF magic\n");
    return 0;
  }

  auto* phdr = reinterpret_cast<const Elf64_Phdr*>(elf_data + ehdr->e_phoff);
  auto& vm = Singleton<VirtualMemory>::GetInstance();

  for (int i = 0; i < ehdr->e_phnum; ++i) {
    if (phdr[i].p_type != PT_LOAD) continue;

    uintptr_t vaddr = phdr[i].p_vaddr;
    uintptr_t memsz = phdr[i].p_memsz;
    uintptr_t filesz = phdr[i].p_filesz;
    uintptr_t offset = phdr[i].p_offset;

    uint32_t flags = cpu_io::virtual_memory::GetUserPagePermissions(
        (phdr[i].p_flags & PF_R) != 0, (phdr[i].p_flags & PF_W) != 0,
        (phdr[i].p_flags & PF_X) != 0);

    uintptr_t start_page = cpu_io::virtual_memory::PageAlign(vaddr);
    uintptr_t end_page = cpu_io::virtual_memory::PageAlignUp(vaddr + memsz);

    for (uintptr_t page = start_page; page < end_page;
         page += cpu_io::virtual_memory::kPageSize) {
      void* p_page = aligned_alloc(cpu_io::virtual_memory::kPageSize,
                                   cpu_io::virtual_memory::kPageSize);
      if (!p_page) {
        klog::Err("Failed to allocate page for ELF\n");
        return 0;
      }
      std::memset(p_page, 0, cpu_io::virtual_memory::kPageSize);

      // Mapping logic
      uintptr_t v_start = page;
      uintptr_t v_end = page + cpu_io::virtual_memory::kPageSize;

      // Map intersection with file data
      uintptr_t file_start = vaddr;
      uintptr_t file_end = vaddr + filesz;

      uintptr_t copy_start = std::max(v_start, file_start);
      uintptr_t copy_end = std::min(v_end, file_end);

      if (copy_end > copy_start) {
        uintptr_t dst_off = copy_start - v_start;
        uintptr_t src_off = (copy_start - vaddr) + offset;
        std::memcpy((uint8_t*)p_page + dst_off, elf_data + src_off,
                    copy_end - copy_start);
      }

      if (!vm.MapPage(page_table, (void*)page, p_page, flags)) {
        klog::Err("MapPage failed\n");
        return 0;
      }
    }
  }
  return ehdr->e_entry;
};

}  // namespace

TaskControlBlock::TaskControlBlock(const char* name, int priority,
                                   ThreadEntry entry, void* arg)
    : name(name) {
  // 设置优先级
  sched_info.priority = priority;
  sched_info.base_priority = priority;

  // 分配内核栈
  kernel_stack = static_cast<uint8_t*>(aligned_alloc(
      cpu_io::virtual_memory::kPageSize, kDefaultKernelStackSize));
  if (!kernel_stack) {
    klog::Err("Failed to allocate kernel stack for task %s\n", name);
    status = TaskStatus::kExited;
    return;
  }

  // 设置 trap_context_ptr 指向内核栈顶预留的位置
  trap_context_ptr = reinterpret_cast<cpu_io::TrapContext*>(
      kernel_stack + kDefaultKernelStackSize - sizeof(cpu_io::TrapContext));

  // 设置内核栈顶
  auto stack_top =
      reinterpret_cast<uint64_t>(kernel_stack) + kDefaultKernelStackSize;

  // 初始化任务上下文
  InitTaskContext(&task_context, entry, arg, stack_top);

  // 状态设为就绪
  status = TaskStatus::kReady;
}

TaskControlBlock::TaskControlBlock(const char* name, int priority, uint8_t* elf,
                                   int argc, char** argv)
    : name(name) {
  // 设置优先级
  sched_info.priority = priority;
  sched_info.base_priority = priority;

  /// @todo
  (void)name;
  (void)priority;
  (void)elf;
  (void)argc;
  (void)argv;
  LoadElf(nullptr, nullptr);
}

TaskControlBlock::~TaskControlBlock() {
  // 释放内核栈
  if (kernel_stack) {
    free(kernel_stack);
    kernel_stack = nullptr;
  }

  /// @todo 释放页表等其他资源
}
