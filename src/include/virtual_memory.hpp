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
#include "singleton.hpp"

/**
 * @brief 虚拟内存管理抽象基类
 * @details 定义了必须通过继承实现的核心虚拟内存接口
 * @note 地址操作、页表项操作等实用函数请使用 cpu_io 库提供的接口
 */
class VirtualMemory {
 public:
  explicit VirtualMemory(void *(*malloc)(size_t)) : malloc_(malloc) {
    // 分配根页表目录
    kernel_page_dir_ = malloc_(cpu_io::virtual_memory::kPageSize);
    if (kernel_page_dir_ == nullptr) {
      klog::Err("Failed to allocate kernel page directory\n");
      return;
    }

    // 清零页表目录
    std::memset(kernel_page_dir_, 0, cpu_io::virtual_memory::kPageSize);

    // 获取内核基本信息
    const auto &basic_info = Singleton<BasicInfo>::GetInstance();

    // 映射全部物理内存
    auto kernel_start = basic_info.physical_memory_addr;
    auto kernel_end =
        basic_info.physical_memory_addr + basic_info.physical_memory_size;

    // 将内核使用的内存进行原地映射
    // 内核需要可读、可写、可执行权限
    auto kernel_flags =
        cpu_io::virtual_memory::kRead | cpu_io::virtual_memory::kWrite |
        cpu_io::virtual_memory::kExec | cpu_io::virtual_memory::kGlobal;

    // 按页对齐映射
    auto start_page = cpu_io::virtual_memory::PageAlign(kernel_start);
    auto end_page = cpu_io::virtual_memory::PageAlignUp(kernel_end);

    for (uint64_t addr = start_page; addr < end_page;
         addr += cpu_io::virtual_memory::kPageSize) {
      if (!MapPage(kernel_page_dir_, reinterpret_cast<void *>(addr),
                   reinterpret_cast<void *>(addr), kernel_flags)) {
        klog::Err("Failed to map kernel page at address 0x%lX\n", addr);
        break;
      }
    }

    // 在构造完成后转储页表内容
    klog::Info(
        "Kernel page table initialization completed. Dumping page table...\n");
    DumpPageTable(kernel_page_dir_, start_page, end_page);
  }

  /// @name 构造/析构函数
  /// @{
  VirtualMemory() = default;
  VirtualMemory(const VirtualMemory &) = delete;
  VirtualMemory(VirtualMemory &&) = default;
  auto operator=(const VirtualMemory &) -> VirtualMemory & = delete;
  auto operator=(VirtualMemory &&) -> VirtualMemory & = default;
  ~VirtualMemory() = default;
  /// @}

  void InitCurrentCore() {
    cpu_io::virtual_memory::SetPageDirectory(
        reinterpret_cast<uint64_t>(kernel_page_dir_));
    // 开启分页功能
    cpu_io::virtual_memory::EnablePage();
  }

  auto MapPage(void *page_dir, void *virtual_addr, void *physical_addr,
               uint32_t flags) -> bool {
    // 查找页表项，如果不存在则分配
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, true);
    if (!pte_opt) {
      return false;
    }
    if (reinterpret_cast<uint64_t>(virtual_addr) > 0x805fb000) {
      klog::Debug(
          "Mapping virtual address %p to physical address %p, pte_opt: %p, "
          "*pte_opt: %p\n",
          virtual_addr, physical_addr, pte_opt, *pte_opt);
    }
    auto *pte = *pte_opt;

    // 检查是否已经映射且标志位相同
    if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      // 如果物理地址和标志位都相同，则认为是重复映射（警告但不失败）
      auto existing_pa = cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
      if (existing_pa == reinterpret_cast<uint64_t>(physical_addr) &&
          (*pte & ((1ULL << cpu_io::virtual_memory::kPteAttributeBits) - 1)) ==
              flags) {
        // 重复映射，但不是错误
        return true;
      }
    }

    // 设置页表项
    *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
        reinterpret_cast<uint64_t>(physical_addr),
        flags | cpu_io::virtual_memory::kValid);

    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  auto UnmapPage(void *page_dir, void *virtual_addr) -> bool {
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, false);
    if (!pte_opt) {
      return false;
    }
    auto *pte = *pte_opt;

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return false;
    }

    // 清除页表项
    *pte = 0;

    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  /**
   * @brief 转储页表的详细内容
   * @param page_dir 页目录
   * @param start_addr 开始转储的虚拟地址（可选，默认为0）
   * @param end_addr 结束转储的虚拟地址（可选，默认为整个地址空间）
   */
  void DumpPageTable(void *page_dir = nullptr, uint64_t start_addr = 0,
                     uint64_t end_addr = 0xFFFFFFFFFFFFFFFF) {
    if (page_dir == nullptr) {
      page_dir = kernel_page_dir_;
    }

    if (page_dir == nullptr) {
      klog::Err("Page directory is null\n");
      return;
    }

    klog::Info("========== Page Table Dump ==========\n");
    klog::Info("Page Directory: 0x%p\n", page_dir);
    klog::Info("Address Range: 0x%lX - 0x%lX\n", start_addr, end_addr);
    klog::Info("====================================\n");

    // 简单转储页表内容
    uint64_t *table = reinterpret_cast<uint64_t *>(page_dir);
    const size_t entries_per_table = 4096 / sizeof(uint64_t);  // 512 entries

    klog::Info("Root page table entries:\n");
    for (size_t i = 0; i < entries_per_table; ++i) {
      uint64_t pte = table[i];
      if (pte != 0) {
        klog::Info("  Entry[%zu]: 0x%016lX (", i, pte);
        // 直接打印页表项的原始值和基本标志位
        if (pte & 0x1) klog::Info("V");   // Valid
        if (pte & 0x2) klog::Info("R");   // Read
        if (pte & 0x4) klog::Info("W");   // Write
        if (pte & 0x8) klog::Info("X");   // Execute
        if (pte & 0x10) klog::Info("U");  // User
        if (pte & 0x20) klog::Info("G");  // Global
        if (pte & 0x40) klog::Info("A");  // Accessed
        if (pte & 0x80) klog::Info("D");  // Dirty
        klog::Info(") PPN: 0x%lX\n", (pte >> 10) & 0xFFFFFFFFFFF);
      }
    }
  }

  [[nodiscard]] auto GetMapping(void *page_dir, void *virtual_addr)
      -> std::optional<void *> {
    auto pte_opt = FindPageTableEntry(page_dir, virtual_addr, false);
    if (!pte_opt) {
      return std::nullopt;
    }
    auto *pte = *pte_opt;

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return std::nullopt;
    }

    return reinterpret_cast<void *>(
        cpu_io::virtual_memory::PageTableEntryToPhysical(*pte));
  }

 private:
  void *(*malloc_)(size_t) = [](size_t) -> void * { return nullptr; };

  void *kernel_page_dir_ = nullptr;

  /**
   * @brief 在页表中查找虚拟地址对应的页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param allocate         如果页表项不存在是否分配新的页表
   * @return std::optional<uint64_t*>  页表项指针，失败时返回std::nullopt
   */
  [[nodiscard]] auto FindPageTableEntry(void *page_dir, void *virtual_addr,
                                        bool allocate = false)
      -> std::optional<uint64_t *> {
    auto *current_table = reinterpret_cast<uint64_t *>(page_dir);
    auto vaddr = reinterpret_cast<uint64_t>(virtual_addr);
    if (vaddr > 0x805fb000) {
      klog::Debug("Finding PTE for virtual address %p in page directory %p\n",
                  virtual_addr, page_dir);
    }

    // 遍历页表层级
    for (size_t level = cpu_io::virtual_memory::kPageTableLevels - 1; level > 0;
         --level) {
      // 获取当前级别的虚拟页号
      auto vpn = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, level);
      auto *pte = &current_table[vpn];

      if (vaddr > 0x805fb000) {
        // klog::Debug(
        //     "current_table: %p, vpn: %lu, current_table[vpn]: %p, "
        //     "&current_table[vpn]: %p\n",
        //     current_table, vpn, current_table[vpn], &current_table[vpn]);
        // klog::Debug("!!!!!!!!!!!\n");

        // klog::Debug("current_table: %p, vpn: %lu\n", current_table, vpn);
      }

      if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
        // 页表项有效，获取下一级页表
        current_table = reinterpret_cast<uint64_t *>(
            cpu_io::virtual_memory::PageTableEntryToPhysical(*pte));
      } else {
        // 页表项无效
        if (allocate) {
          auto *new_table = malloc_(cpu_io::virtual_memory::kPageSize);
          if (new_table == nullptr) {
            return std::nullopt;
          }

          // 清零新页表
          std::memset(new_table, 0, cpu_io::virtual_memory::kPageSize);

          // 设置页表项
          *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
              reinterpret_cast<uint64_t>(new_table),
              cpu_io::virtual_memory::kValid);

          current_table = reinterpret_cast<uint64_t *>(new_table);
        } else {
          return std::nullopt;
        }
      }
    }

    // 返回最底层页表中的页表项
    auto vpn = cpu_io::virtual_memory::GetVirtualPageNumber(vaddr, 0);

    // if (vaddr > 0x805fb000) {
    // klog::Debug(
    //     "current_table: %p, vpn: %lu, current_table[vpn]: %p, "
    //     "&current_table[vpn]: %p\n",
    //     current_table, vpn, current_table[vpn], &current_table[vpn]);
    // }

    return &current_table[vpn];
  }
};

#endif /* SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_ */
