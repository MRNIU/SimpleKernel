/**
 * Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_
#define SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_

#include <cpu_io.h>

#include <bmalloc.hpp>
#include <cstddef>
#include <cstdint>

#include "spinlock.hpp"

/// 页表项类型定义
using pte_t = uint64_t;
/// 页目录类型定义
using pgd_t = uint64_t;

/**
 * @brief 虚拟内存管理抽象基类
 * @details 定义了必须通过继承实现的核心虚拟内存接口
 * @note 地址操作、页表项操作等实用函数请使用 cpu_io 库提供的接口
 */
class VirtualMemory {
 public:
  VirtualMemory() : spinlock("VirtualMemory") {}

  /// @name 构造/析构函数
  /// @{
  VirtualMemory(const VirtualMemory&) = delete;
  VirtualMemory(VirtualMemory&&) = default;
  auto operator=(const VirtualMemory&) -> VirtualMemory& = delete;
  auto operator=(VirtualMemory&&) -> VirtualMemory& = default;
  ~VirtualMemory() = default;
  /// @}

  void InitCurrentCore() {
    // 开启分页功能
    cpu_io::virtual_memory::EnablePage();
  }

  /**
   * @brief 创建新的页目录
   * @return pgd_t           新的页目录，失败时返回0
   */
  [[nodiscard]] auto CreatePageDirectory() -> pgd_t {
    // 分配一页用于页目录
    void* pgd_page = MemoryManager{}.AllocatePhysicalPages(1);
    if (pgd_page == nullptr) {
      return 0;
    }

    // 清零页目录
    std::memset(pgd_page, 0, cpu_io::virtual_memory::kPageSize);

    return reinterpret_cast<pgd_t>(pgd_page);
  }

  /**
   * @brief 销毁页目录
   * @param page_dir         要销毁的页目录
   */
  void DestroyPageDirectory(pgd_t page_dir) {
    if (page_dir == 0) {
      return;
    }

    // TODO: 递归释放所有页表占用的物理内存
    // 这里需要遍历页目录中的所有页表并释放它们

    // 释放页目录本身
    MemoryManager{}.FreePhysicalPages(reinterpret_cast<void*>(page_dir), 1);
  }

  /// @name 页面映射操作
  /// @{
  /**
   * @brief 映射虚拟地址到物理地址
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param physical_addr    物理地址
   * @param flags            页面属性标志（使用 cpu_io::virtual_memory
   * 中的常量）
   * @return true            映射成功
   * @return false           映射失败
   */
  auto MapPage(pgd_t page_dir, uint64_t virtual_addr, uint64_t physical_addr,
               uint32_t flags) -> bool {
    std::lock_guard<SpinLock> lock(spinlock);

    // 查找页表项，如果不存在则分配
    pte_t* pte = FindPageTableEntry(page_dir, virtual_addr, true);
    if (pte == nullptr) {
      return false;
    }

    // 检查是否已经映射且标志位相同
    if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      // 如果物理地址和标志位都相同，则认为是重复映射（警告但不失败）
      uint64_t existing_pa =
          cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
      if (existing_pa == physical_addr &&
          (*pte & ((1ULL << cpu_io::virtual_memory::kPteAttributeBits) - 1)) ==
              flags) {
        return true;  // 重复映射，但不是错误
      }
    }

    // 设置页表项
    *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
        physical_addr, flags | cpu_io::virtual_memory::kValid);

    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  /**
   * @brief 映射连续的虚拟地址范围到物理地址
   * @param page_dir         页目录
   * @param virtual_start    虚拟地址起始
   * @param physical_start   物理地址起始
   * @param size             映射大小（字节）
   * @param flags            页面属性标志
   * @return true            映射成功
   * @return false           映射失败
   */
  auto MapPages(pgd_t page_dir, uint64_t virtual_start, uint64_t physical_start,
                size_t size, uint32_t flags) -> bool {
    if (size == 0) {
      return true;
    }

    // 计算页数，向上取整
    size_t page_count = (size + cpu_io::virtual_memory::kPageSize - 1) /
                        cpu_io::virtual_memory::kPageSize;

    for (size_t i = 0; i < page_count; ++i) {
      uint64_t va = virtual_start + (i * cpu_io::virtual_memory::kPageSize);
      uint64_t pa = physical_start + (i * cpu_io::virtual_memory::kPageSize);

      if (!MapPage(page_dir, va, pa, flags)) {
        // 映射失败，回滚已映射的页面
        for (size_t j = 0; j < i; ++j) {
          uint64_t rollback_va =
              virtual_start + (j * cpu_io::virtual_memory::kPageSize);
          UnmapPage(page_dir, rollback_va);
        }
        return false;
      }
    }

    return true;
  }

  /**
   * @brief 取消虚拟地址映射
   * @param page_dir         页目录
   * @param virtual_addr     要取消映射的虚拟地址
   * @return true            取消映射成功
   * @return false           取消映射失败
   */
  auto UnmapPage(pgd_t page_dir, uint64_t virtual_addr) -> bool {
    std::lock_guard<SpinLock> lock(spinlock);

    pte_t* pte = FindPageTableEntry(page_dir, virtual_addr, false);
    if (pte == nullptr) {
      return false;  // 页表项不存在
    }

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return false;  // 页表项无效（未映射）
    }

    // 清除页表项
    *pte = 0;

    // 刷新 TLB
    cpu_io::virtual_memory::FlushTLBAll();

    return true;
  }

  /**
   * @brief 取消连续虚拟地址范围的映射
   * @param page_dir         页目录
   * @param virtual_start    虚拟地址起始
   * @param size             取消映射大小（字节）
   * @return true            取消映射成功
   * @return false           取消映射失败
   */
  auto UnmapPages(pgd_t page_dir, uint64_t virtual_start, size_t size) -> bool {
    if (size == 0) {
      return true;
    }

    // 计算页数，向上取整
    size_t page_count = (size + cpu_io::virtual_memory::kPageSize - 1) /
                        cpu_io::virtual_memory::kPageSize;

    bool all_success = true;
    for (size_t i = 0; i < page_count; ++i) {
      uint64_t va = virtual_start + (i * cpu_io::virtual_memory::kPageSize);
      if (!UnmapPage(page_dir, va)) {
        all_success = false;
      }
    }

    return all_success;
  }

  /// @name 地址查询操作
  /// @{
  /**
   * @brief 获取虚拟地址映射的物理地址
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param physical_addr    输出参数，存储映射的物理地址
   * @return true            虚拟地址已映射
   * @return false           虚拟地址未映射
   */
  [[nodiscard]] auto GetMapping(pgd_t page_dir, uint64_t virtual_addr,
                                uint64_t& physical_addr) -> bool {
    std::lock_guard<SpinLock> lock(spinlock);

    pte_t* pte = FindPageTableEntry(page_dir, virtual_addr, false);
    if (pte == nullptr) {
      return false;
    }

    if (!cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
      return false;
    }

    physical_addr = cpu_io::virtual_memory::PageTableEntryToPhysical(*pte);
    return true;
  }

  /**
   * @brief 检查虚拟地址是否已映射
   * @param page_dir         页目录
   * @param virtual_addr     要检查的虚拟地址
   * @return true            已映射
   * @return false           未映射
   */
  [[nodiscard]] auto IsMapped(pgd_t page_dir, uint64_t virtual_addr) -> bool {
    uint64_t physical_addr;
    return GetMapping(page_dir, virtual_addr, physical_addr);
  }

  /**
   * @brief 获取页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param pte              输出参数，存储页表项
   * @return true            获取成功
   * @return false           获取失败
   */
  [[nodiscard]] auto GetPageTableEntry(pgd_t page_dir, uint64_t virtual_addr,
                                       pte_t& pte) -> bool {
    std::lock_guard<SpinLock> lock(spinlock);

    pte_t* pte_ptr = FindPageTableEntry(page_dir, virtual_addr, false);
    if (pte_ptr == nullptr) {
      return false;
    }

    pte = *pte_ptr;
    return true;
  }
  /// @}

 private:
  SpinLock spinlock;

  /**
   * @brief 在页表中查找虚拟地址对应的页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param allocate         如果页表项不存在是否分配新的页表
   * @return pte_t*          页表项指针，失败时返回nullptr
   */
  [[nodiscard]] auto FindPageTableEntry(pgd_t page_dir, uint64_t virtual_addr,
                                        bool allocate = false) -> pte_t* {
    auto* current_table = reinterpret_cast<pte_t*>(page_dir);

    // 遍历页表层级，RISC-V SV39 有 3 级页表
    for (size_t level = cpu_io::virtual_memory::kPageTableLevels - 1; level > 0;
         --level) {
      // 获取当前级别的虚拟页号
      uint64_t vpn =
          cpu_io::virtual_memory::GetVirtualPageNumber(virtual_addr, level);
      pte_t* pte = &current_table[vpn];

      if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
        // 页表项有效，获取下一级页表
        current_table = reinterpret_cast<pte_t*>(
            cpu_io::virtual_memory::PageTableEntryToPhysical(*pte));
      } else {
        // 页表项无效
        if (allocate) {
          // 分配新的页表
          void* new_table = MemoryManager{}.AllocatePhysicalPages(1);
          if (new_table == nullptr) {
            return nullptr;
          }

          // 清零新页表
          std::memset(new_table, 0, cpu_io::virtual_memory::kPageSize);

          // 设置页表项
          *pte = cpu_io::virtual_memory::PhysicalToPageTableEntry(
              reinterpret_cast<uint64_t>(new_table),
              cpu_io::virtual_memory::kValid);

          current_table = reinterpret_cast<pte_t*>(new_table);
        } else {
          return nullptr;
        }
      }
    }

    // 返回最底层页表中的页表项
    uint64_t vpn =
        cpu_io::virtual_memory::GetVirtualPageNumber(virtual_addr, 0);
    return &current_table[vpn];
  }
};

#endif /* SIMPLEKERNEL_SRC_MEMORY_INCLUDE_VIRTUAL_MEMORY_HPP_ */
