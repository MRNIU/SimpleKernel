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

/**
 * @brief 虚拟内存管理抽象基类
 * @details 定义了必须通过继承实现的核心虚拟内存接口
 * @note 地址操作、页表项操作等实用函数请使用 cpu_io 库提供的接口
 */
class VirtualMemory {
 public:
  /// @name 构造/析构函数
  /// @{
  VirtualMemory() = default;
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
  auto MapPage(uint64_t page_dir, uint64_t virtual_addr, uint64_t physical_addr,
               uint32_t flags) -> bool {
    // 查找页表项，如果不存在则分配
    auto pte = FindPageTableEntry(page_dir, virtual_addr, true);
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
        // 重复映射，但不是错误
        return true;
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
   * @brief 取消虚拟地址映射
   * @param page_dir         页目录
   * @param virtual_addr     要取消映射的虚拟地址
   * @return true            取消映射成功
   * @return false           取消映射失败
   */
  auto UnmapPage(uint64_t page_dir, uint64_t virtual_addr) -> bool {
    uint64_t* pte = FindPageTableEntry(page_dir, virtual_addr, false);
    if (pte == nullptr) {
      return false;
    }

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
   * @brief 获取虚拟地址映射的物理地址
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param physical_addr    输出参数，存储映射的物理地址
   * @return true            虚拟地址已映射
   * @return false           虚拟地址未映射
   */
  [[nodiscard]] auto GetMapping(uint64_t page_dir, uint64_t virtual_addr,
                                uint64_t& physical_addr) -> bool {
    uint64_t* pte = FindPageTableEntry(page_dir, virtual_addr, false);
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
  [[nodiscard]] auto IsMapped(uint64_t page_dir, uint64_t virtual_addr)
      -> bool {
    uint64_t physical_addr;
    return GetMapping(page_dir, virtual_addr, physical_addr);
  }

  /**
   * @brief 创建相等映射（identity mapping）页表
   * @param page_dir         页目录
   * @param start_addr       起始物理地址
   * @param size             映射大小（字节）
   * @param flags            页面属性标志
   * @return true            映射成功
   * @return false           映射失败
   */
  auto CreateIdentityMapping(uint64_t page_dir, uint64_t start_addr,
                             uint64_t size, uint32_t flags) -> bool {
    // 确保地址和大小都是页对齐的
    uint64_t aligned_start = cpu_io::virtual_memory::PageAlign(start_addr);
    uint64_t aligned_end =
        cpu_io::virtual_memory::PageAlignUp(start_addr + size);

    // 逐页映射，虚拟地址等于物理地址
    for (uint64_t addr = aligned_start; addr < aligned_end;
         addr += cpu_io::virtual_memory::kPageSize) {
      if (!MapPage(page_dir, addr, addr, flags)) {
        // 映射失败，尝试回滚已映射的页面
        for (uint64_t rollback_addr = aligned_start; rollback_addr < addr;
             rollback_addr += cpu_io::virtual_memory::kPageSize) {
          UnmapPage(page_dir, rollback_addr);
        }
        return false;
      }
    }

    return true;
  }

  /**
   * @brief 为内核创建相等映射页表
   * @param page_dir         页目录
   * @param kernel_start     内核起始地址
   * @param kernel_end       内核结束地址
   * @return true            映射成功
   * @return false           映射失败
   */
  auto CreateKernelIdentityMapping(uint64_t page_dir, uint64_t kernel_start,
                                   uint64_t kernel_end) -> bool {
    // 内核代码段：可读可执行
    uint32_t kernel_code_flags = cpu_io::virtual_memory::kRead |
                                 cpu_io::virtual_memory::kExec |
                                 cpu_io::virtual_memory::kGlobal;

    // 内核数据段：可读可写
    uint32_t kernel_data_flags = cpu_io::virtual_memory::kRead |
                                 cpu_io::virtual_memory::kWrite |
                                 cpu_io::virtual_memory::kGlobal;

    // 为简化，整个内核区域使用可读可写可执行权限
    // 在实际使用中，可以根据段信息进行更精细的权限控制
    uint32_t kernel_flags =
        cpu_io::virtual_memory::kRead | cpu_io::virtual_memory::kWrite |
        cpu_io::virtual_memory::kExec | cpu_io::virtual_memory::kGlobal;

    uint64_t kernel_size = kernel_end - kernel_start;
    return CreateIdentityMapping(page_dir, kernel_start, kernel_size,
                                 kernel_flags);
  }

  /**
   * @brief 获取页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param pte              输出参数，存储页表项
   * @return true            获取成功
   * @return false           获取失败
   */
  [[nodiscard]] auto GetPageTableEntry(uint64_t page_dir, uint64_t virtual_addr,
                                       uint64_t& pte) -> bool {
    uint64_t* pte_ptr = FindPageTableEntry(page_dir, virtual_addr, false);
    if (pte_ptr == nullptr) {
      return false;
    }

    pte = *pte_ptr;
    return true;
  }

 private:
  /**
   * @brief 在页表中查找虚拟地址对应的页表项
   * @param page_dir         页目录
   * @param virtual_addr     虚拟地址
   * @param allocate         如果页表项不存在是否分配新的页表
   * @return uint64_t*          页表项指针，失败时返回nullptr
   */
  [[nodiscard]] auto FindPageTableEntry(uint64_t page_dir,
                                        uint64_t virtual_addr,
                                        bool allocate = false) -> uint64_t* {
    auto* current_table = reinterpret_cast<uint64_t*>(page_dir);

    // 遍历页表层级，RISC-V SV39 有 3 级页表
    for (size_t level = cpu_io::virtual_memory::kPageTableLevels - 1; level > 0;
         --level) {
      // 获取当前级别的虚拟页号
      uint64_t vpn =
          cpu_io::virtual_memory::GetVirtualPageNumber(virtual_addr, level);
      uint64_t* pte = &current_table[vpn];

      if (cpu_io::virtual_memory::IsPageTableEntryValid(*pte)) {
        // 页表项有效，获取下一级页表
        current_table = reinterpret_cast<uint64_t*>(
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

          current_table = reinterpret_cast<uint64_t*>(new_table);
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
