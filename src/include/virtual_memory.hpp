/**
 * Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_
#define SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_

#include <cpu_io.h>

#include <bmalloc.hpp>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <optional>

#include "basic_info.hpp"
#include "kernel_log.hpp"
#include "per_cpu.hpp"
#include "singleton.hpp"

/**
 * @brief 虚拟内存管理抽象基类
 * @details 定义了必须通过继承实现的核心虚拟内存接口
 * @note 地址操作、页表项操作等实用函数请使用 cpu_io 库提供的接口
 */
class VirtualMemory {
 public:
  explicit VirtualMemory(void* (*aligned_alloc)(size_t, size_t))
      : aligned_alloc_(aligned_alloc) {
    // 分配根页表目录
    kernel_page_dir_ = aligned_alloc_(cpu_io::virtual_memory::kPageSize,
                                      cpu_io::virtual_memory::kPageSize);
    klog::Info("Allocated kernel page directory at address: %p\n",
               kernel_page_dir_);
    if (kernel_page_dir_ == nullptr) {
      klog::Err("Failed to allocate kernel page directory\n");
      return;
    }

    // 清零页表目录
    std::memset(kernel_page_dir_, 0, cpu_io::virtual_memory::kPageSize);

    // 获取内核基本信息
    const auto& basic_info = Singleton<BasicInfo>::GetInstance();

    // 映射全部物理内存
    MapMMIO(basic_info.physical_memory_addr, basic_info.physical_memory_size);
    klog::Info(
        "Kernel memory mapped from 0x%lX to 0x%lX\n",
        basic_info.physical_memory_addr,
        basic_info.physical_memory_addr + basic_info.physical_memory_size);
  }

  /// @name 构造/析构函数
  /// @{
  VirtualMemory() = default;
  VirtualMemory(const VirtualMemory&) = delete;
  VirtualMemory(VirtualMemory&&) = default;
  auto operator=(const VirtualMemory&) -> VirtualMemory& = delete;
  auto operator=(VirtualMemory&&) -> VirtualMemory& = default;
  ~VirtualMemory() = default;
  /// @}

  void InitCurrentCore() const {
    cpu_io::virtual_memory::SetPageDirectory(
        reinterpret_cast<uint64_t>(kernel_page_dir_));
    // 开启分页
    cpu_io::virtual_memory::EnablePage();
  }

  /**
   * @brief 映射设备内存 (MMIO)
   * @param phys_addr 设备物理基地址
   * @param size 映射大小
   * @param flags 页表属性，默认为内核设备内存属性（如果架构支持区分的话）
   * @return 映射后的虚拟地址 (通常在内核空间，可能是恒等映射)
   */
  auto MapMMIO(
      uint64_t phys_addr, size_t size,
      uint32_t flags = cpu_io::virtual_memory::GetKernelPagePermissions())
      -> void* {
    // 计算对齐后的起始和结束页
    auto start_page = cpu_io::virtual_memory::PageAlign(phys_addr);
    auto end_page = cpu_io::virtual_memory::PageAlignUp(phys_addr + size);

    // 遍历并映射
    for (uint64_t addr = start_page; addr < end_page;
         addr += cpu_io::virtual_memory::kPageSize) {
      if (!MapPage(kernel_page_dir_, reinterpret_cast<void*>(addr),
                   reinterpret_cast<void*>(addr), flags)) {
        klog::Err("MapMMIO: Failed to map address 0x%lX\n", addr);
        return nullptr;
      }
    }
    return reinterpret_cast<void*>(phys_addr);
  }

  auto MapPage(void* page_dir, void* virtual_addr, void* physical_addr,
               uint32_t flags) -> bool {
    // 查找页表项，如果不存在则分配
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, true);
    if (!pte_opt) {
      klog::Err("MapPage: FindPageTableEntry failed for va = %p\n",
                virtual_addr);
      return false;
    }
    auto* pte = *pte_opt;

    // 检查是否已经映射且标志位相同
    if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      // 如果物理地址和标志位都相同，则认为是重复映射（警告但不失败）
      auto existing_pa = cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
      if (existing_pa == reinterpret_cast<uint64_t>(physical_addr) &&
          (*pte & ((1ULL << cpu_io::virtual_memory::kPteAttributeBits) - 1)) ==
              flags) {
        klog::Debug(
            "MapPage: duplicate va = %p, pa = 0x%lX, flags = 0x%X, skip\n",
            virtual_addr, existing_pa, flags);
        // 重复映射，但不是错误
        return true;
      }
      klog::Warn("MapPage: remap va = %p from pa = 0x%lX to pa = %p\n",
                 virtual_addr, existing_pa, physical_addr);
    }

    // 设置页表项
    *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
        reinterpret_cast<uint64_t>(physical_addr), flags);
    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  auto UnmapPage(void* page_dir, void* virtual_addr) -> bool {
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, false);
    if (!pte_opt) {
      return false;
    }
    auto* pte = *pte_opt;

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return false;
    }

    // 清除页表项
    *pte = 0;

    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  [[nodiscard]] auto GetMapping(void* page_dir, void* virtual_addr)
      -> std::optional<void*> {
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, false);
    if (!pte_opt) {
      return std::nullopt;
    }
    auto* pte = *pte_opt;

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return std::nullopt;
    }

    return reinterpret_cast<void*>(
        cpu_io::virtual_memory::PageTableEntryToPhysical(*pte));
  }

 private:
  void* (*aligned_alloc_)(size_t, size_t) = [](size_t, size_t) -> void* {
    return nullptr;
  };

  void* kernel_page_dir_ = nullptr;

  /**
   * @brief 在页表中查找虚拟地址对应的页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param allocate         如果页表项不存在是否分配新的页表
   * @return std::optional<uint64_t*>  页表项指针，失败时返回std::nullopt
   */
  [[nodiscard]] auto FindPageTableEntry(void* page_dir, void* virtual_addr,
                                        bool allocate = false)
      -> std::optional<uint64_t*> {
    auto* current_table = reinterpret_cast<uint64_t*>(page_dir);
    auto vaddr = reinterpret_cast<uint64_t>(virtual_addr);

    // 遍历页表层级
    for (size_t level = cpu_io::virtual_memory::kPageTableLevels - 1; level > 0;
         --level) {
      // 获取当前级别的虚拟页号
      auto vpn = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, level);
      auto* pte = &current_table[vpn];
      if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
        // 页表项有效，获取下一级页表
        current_table = reinterpret_cast<uint64_t*>(
            cpu_io::virtual_memory::PageTableEntryToPhysical(*pte));
      } else {
        // 页表项无效
        if (allocate) {
          auto* new_table = aligned_alloc_(cpu_io::virtual_memory::kPageSize,
                                           cpu_io::virtual_memory::kPageSize);
          if (new_table == nullptr) {
            return std::nullopt;
          }
          // 清零新页表
          std::memset(new_table, 0, cpu_io::virtual_memory::kPageSize);

          // 设置页表项
          *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
              reinterpret_cast<uint64_t>(new_table),
              cpu_io::virtual_memory::GetKernelPagePermissions());

          current_table = reinterpret_cast<uint64_t*>(new_table);
        } else {
          return std::nullopt;
        }
      }
    }

    // 返回最底层页表中的页表项
    auto vpn = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, 0);

    return &current_table[vpn];
  }
};

#endif /* SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_ */
