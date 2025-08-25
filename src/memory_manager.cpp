/**
 * @file memory_manager.cpp
 * @brief 内存管理器实现
 * @author SimpleKernel Contributors
 */

#include "memory_manager.hpp"

#include <algorithm>
#include <cstring>

#include "kernel_log.hpp"

namespace kernel {

bool MemoryManager::Initialize(void* physical_memory_start,
                               size_t physical_memory_size, void* kernel_end) {
  LockGuard lock(memory_lock_);

  if (initialized_) {
    return false;
  }

  // 保存内存信息
  physical_memory_start_ = physical_memory_start;
  physical_memory_size_ = physical_memory_size;

  // 计算内核堆起始地址（内核结束后对齐到页边界）
  uintptr_t kernel_end_addr = reinterpret_cast<uintptr_t>(kernel_end);
  kernel_end_addr = (kernel_end_addr + kPageSize - 1) & ~(kPageSize - 1);
  kernel_heap_start_ = reinterpret_cast<void*>(kernel_end_addr);
  kernel_heap_size_ = kKernelHeapSize;

  // 确保内核堆不超出物理内存范围
  uintptr_t physical_end = reinterpret_cast<uintptr_t>(physical_memory_start_) +
                           physical_memory_size_;
  if (kernel_end_addr + kernel_heap_size_ > physical_end) {
    kernel_heap_size_ = physical_end - kernel_end_addr;
    if (kernel_heap_size_ < 1024 * 1024) {  // 至少需要1MB内核堆
      KLOG_ERROR("Insufficient memory for kernel heap\n");
      return false;
    }
  }

  // 初始化物理内存分配器（管理内核堆之后的内存）
  void* physical_pool_start =
      reinterpret_cast<void*>(kernel_end_addr + kernel_heap_size_);
  size_t physical_pool_size =
      physical_end - (kernel_end_addr + kernel_heap_size_);

  if (physical_pool_size < kPageSize) {
    KLOG_ERROR("Insufficient memory for physical allocator\n");
    return false;
  }

  try {
    physical_allocator_ = std::make_unique<PhysicalAllocator>(
        physical_pool_start, physical_pool_size);
    kernel_allocator_ =
        std::make_unique<KernelAllocator>(kernel_heap_start_, kernel_heap_size_);
  } catch (...) {
    KLOG_ERROR("Failed to create memory allocators\n");
    return false;
  }

  // 分配初始页目录
  current_page_directory_ = AllocatePageTable();
  if (!current_page_directory_) {
    KLOG_ERROR("Failed to allocate initial page directory\n");
    return false;
  }

  // 清零页目录
  std::memset(current_page_directory_, 0, kPageSize);

  initialized_ = true;

  KLOG_INFO("Memory manager initialized\n");
  KLOG_INFO("  Physical memory: %p - %p (%zu MB)\n", physical_memory_start_,
            reinterpret_cast<void*>(physical_end),
            physical_memory_size_ / (1024 * 1024));
  KLOG_INFO("  Kernel heap: %p - %p (%zu MB)\n", kernel_heap_start_,
            reinterpret_cast<void*>(kernel_end_addr + kernel_heap_size_),
            kernel_heap_size_ / (1024 * 1024));
  KLOG_INFO("  Physical pool: %p - %p (%zu MB)\n", physical_pool_start,
            reinterpret_cast<void*>(physical_end),
            physical_pool_size / (1024 * 1024));

  return true;
}

void* MemoryManager::AllocatePhysicalPages(size_t pages) {
  if (!initialized_ || pages == 0) {
    return nullptr;
  }

  LockGuard lock(memory_lock_);
  return physical_allocator_->malloc(pages * kPageSize);
}

void MemoryManager::FreePhysicalPages(void* addr, size_t pages) {
  if (!initialized_ || !addr || pages == 0) {
    return;
  }

  LockGuard lock(memory_lock_);
  physical_allocator_->free(addr);
}

void* MemoryManager::AllocateKernelMemory(size_t size) {
  if (!initialized_ || size == 0) {
    return nullptr;
  }

  LockGuard lock(memory_lock_);
  return kernel_allocator_->malloc(size);
}

void MemoryManager::FreeKernelMemory(void* addr) {
  if (!initialized_ || !addr) {
    return;
  }

  LockGuard lock(memory_lock_);
  kernel_allocator_->free(addr);
}

bool MemoryManager::MapVirtualMemory(void* virtual_addr, void* physical_addr,
                                     size_t size, MemoryProtection protection,
                                     MemoryType type) {
  if (!initialized_ || !virtual_addr || !physical_addr || size == 0) {
    return false;
  }

  LockGuard lock(memory_lock_);

  uintptr_t vaddr = reinterpret_cast<uintptr_t>(virtual_addr);
  uintptr_t paddr = reinterpret_cast<uintptr_t>(physical_addr);

  // 确保地址和大小都是页对齐的
  if ((vaddr & (kPageSize - 1)) != 0 || (paddr & (kPageSize - 1)) != 0 ||
      (size & (kPageSize - 1)) != 0) {
    return false;
  }

  size_t pages = size / kPageSize;

#ifdef __riscv
  // RISC-V 虚拟内存映射
  for (size_t i = 0; i < pages; i++) {
    void* current_vaddr = reinterpret_cast<void*>(vaddr + i * kPageSize);
    void* current_paddr = reinterpret_cast<void*>(paddr + i * kPageSize);

    // 为所有页表级别创建或获取页表项
    uint64_t* pte = GetOrCreatePageTableEntry(current_page_directory_,
                                              current_vaddr, 0, true);
    if (!pte) {
      // 回滚已经映射的页面
      UnmapVirtualMemory(virtual_addr, i * kPageSize);
      return false;
    }

    // 创建页表项
    *pte = CreatePageTableEntry(current_paddr, protection, type);
  }
#else
  // 其他架构的实现（待添加）
  KLOG_WARNING("Virtual memory mapping not implemented for this architecture\n");
  return false;
#endif

  return true;
}

bool MemoryManager::UnmapVirtualMemory(void* virtual_addr, size_t size) {
  if (!initialized_ || !virtual_addr || size == 0) {
    return false;
  }

  LockGuard lock(memory_lock_);

  uintptr_t vaddr = reinterpret_cast<uintptr_t>(virtual_addr);
  if ((vaddr & (kPageSize - 1)) != 0 || (size & (kPageSize - 1)) != 0) {
    return false;
  }

  size_t pages = size / kPageSize;

#ifdef __riscv
  for (size_t i = 0; i < pages; i++) {
    void* current_vaddr = reinterpret_cast<void*>(vaddr + i * kPageSize);
    uint64_t* pte = GetOrCreatePageTableEntry(current_page_directory_,
                                              current_vaddr, 0, false);
    if (pte) {
      *pte = 0;  // 清除页表项
    }
  }
#endif

  return true;
}

void* MemoryManager::VirtualToPhysical(void* virtual_addr) {
  if (!initialized_ || !virtual_addr) {
    return nullptr;
  }

#ifdef __riscv
  LockGuard lock(memory_lock_);

  uint64_t* pte = GetOrCreatePageTableEntry(current_page_directory_,
                                            virtual_addr, 0, false);
  if (!pte || !cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
    return nullptr;
  }

  uint64_t physical_page = cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
  uintptr_t offset = reinterpret_cast<uintptr_t>(virtual_addr) & (kPageSize - 1);
  return reinterpret_cast<void*>(physical_page + offset);
#else
  // 简单映射，用于没有实现虚拟内存的架构
  return virtual_addr;
#endif
}

void* MemoryManager::PhysicalToVirtual(void* physical_addr) {
  if (!initialized_ || !physical_addr) {
    return nullptr;
  }

#ifdef __riscv
  // 对于内核空间，简单地加上偏移
  return reinterpret_cast<void*>(
      reinterpret_cast<uintptr_t>(physical_addr) + 
      cpu_io::virtual_memory::kKernelOffset);
#else
  // 简单映射
  return physical_addr;
#endif
}

void MemoryManager::SetPageDirectory(void* page_directory) {
  if (!initialized_ || !page_directory) {
    return;
  }

  LockGuard lock(memory_lock_);
  current_page_directory_ = page_directory;

#ifdef __riscv
  cpu_io::virtual_memory::SetPageDirectory(
      reinterpret_cast<uint64_t>(page_directory));
#endif
}

void* MemoryManager::GetPageDirectory() {
  if (!initialized_) {
    return nullptr;
  }

#ifdef __riscv
  return reinterpret_cast<void*>(cpu_io::virtual_memory::GetPageDirectory());
#else
  return current_page_directory_;
#endif
}

void MemoryManager::EnablePaging() {
  if (!initialized_) {
    return;
  }

#ifdef __riscv
  cpu_io::virtual_memory::EnablePage();
#endif

  KLOG_INFO("Paging enabled\n");
}

void MemoryManager::DisablePaging() {
  if (!initialized_) {
    return;
  }

#ifdef __riscv
  cpu_io::virtual_memory::DisablePage();
#endif

  KLOG_INFO("Paging disabled\n");
}

void MemoryManager::GetMemoryStatistics(size_t& total_pages,
                                        size_t& used_pages,
                                        size_t& free_pages) {
  if (!initialized_) {
    total_pages = used_pages = free_pages = 0;
    return;
  }

  LockGuard lock(memory_lock_);

  total_pages = physical_memory_size_ / kPageSize;
  free_pages = physical_allocator_->GetFreeCount();
  used_pages = total_pages - free_pages;
}

bool MemoryManager::IsValidAddress(void* addr) {
  if (!initialized_ || !addr) {
    return false;
  }

  uintptr_t address = reinterpret_cast<uintptr_t>(addr);
  uintptr_t phys_start = reinterpret_cast<uintptr_t>(physical_memory_start_);
  uintptr_t phys_end = phys_start + physical_memory_size_;

  // 检查是否在物理内存范围内
  return (address >= phys_start && address < phys_end);
}

uint64_t MemoryManager::CreatePageTableEntry(void* physical_addr,
                                             MemoryProtection protection,
                                             MemoryType type) {
#ifdef __riscv
  uint64_t flags = cpu_io::virtual_memory::kValid;

  // 设置权限位
  if (static_cast<uint8_t>(protection) & static_cast<uint8_t>(MemoryProtection::kRead)) {
    flags |= cpu_io::virtual_memory::kRead;
  }
  if (static_cast<uint8_t>(protection) & static_cast<uint8_t>(MemoryProtection::kWrite)) {
    flags |= cpu_io::virtual_memory::kWrite;
  }
  if (static_cast<uint8_t>(protection) & static_cast<uint8_t>(MemoryProtection::kExecute)) {
    flags |= cpu_io::virtual_memory::kExec;
  }

  // 设置类型相关的标志
  switch (type) {
    case MemoryType::kUser:
      flags |= cpu_io::virtual_memory::kUser;
      break;
    case MemoryType::kKernel:
      flags |= cpu_io::virtual_memory::kGlobal;
      break;
    default:
      break;
  }

  return cpu_io::virtual_memory::PhysicalToPageTableEntry(
      reinterpret_cast<uint64_t>(physical_addr), flags);
#else
  // 其他架构的实现
  return 0;
#endif
}

void* MemoryManager::AllocatePageTable() {
  void* page_table = AllocatePhysicalPages(1);
  if (page_table) {
    std::memset(page_table, 0, kPageSize);
  }
  return page_table;
}

uint64_t* MemoryManager::GetOrCreatePageTableEntry(void* page_directory,
                                                   void* virtual_addr,
                                                   int level, bool allocate) {
#ifdef __riscv
  if (level < 0 || level >= static_cast<int>(cpu_io::virtual_memory::kPageTableLevels)) {
    return nullptr;
  }

  uint64_t* current_table = static_cast<uint64_t*>(page_directory);
  uintptr_t vaddr = reinterpret_cast<uintptr_t>(virtual_addr);

  // 从最高级页表开始遍历
  for (int current_level = cpu_io::virtual_memory::kPageTableLevels - 1;
       current_level > level; current_level--) {
    size_t index = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, current_level);
    uint64_t* pte = &current_table[index];

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      if (!allocate) {
        return nullptr;
      }

      // 分配新的页表
      void* new_table = AllocatePageTable();
      if (!new_table) {
        return nullptr;
      }

      *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
          reinterpret_cast<uint64_t>(new_table), 
          cpu_io::virtual_memory::kValid);
    }

    // 获取下一级页表
    uint64_t next_table_phys = cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
    current_table = static_cast<uint64_t*>(PhysicalToVirtual(
        reinterpret_cast<void*>(next_table_phys)));
  }

  // 返回目标级别的页表项
  size_t final_index = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, level);
  return &current_table[final_index];
#else
  return nullptr;
#endif
}

}  // namespace kernel
