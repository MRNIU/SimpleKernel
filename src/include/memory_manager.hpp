/**
 * Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_MEMORY_MANAGER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_MEMORY_MANAGER_HPP_

#include <cpu_io.h>

#include <bmalloc.hpp>
#include <cstddef>
#include <cstdint>
#include <memory>

#include "spinlock.hpp"

/**
 * @brief 内存区域类型
 */
enum class MemoryType : uint8_t {
  kKernel = 0,  ///< 内核空间
  kUser = 1,    ///< 用户空间
  kDevice = 2,  ///< 设备内存
  kDma = 3,     ///< DMA 内存
};

/**
 * @brief 内存保护标志
 */
enum class MemoryProtection : uint8_t {
  kNone = 0,                                      ///< 无权限
  kRead = 1,                                      ///< 可读
  kWrite = 2,                                     ///< 可写
  kExecute = 4,                                   ///< 可执行
  kReadWrite = kRead | kWrite,                    ///< 读写
  kReadExecute = kRead | kExecute,                ///< 读执行
  kReadWriteExecute = kRead | kWrite | kExecute,  ///< 读写执行
};

/**
 * @brief 内存管理器类
 * 负责管理物理内存分配和虚拟内存映射
 */
class MemoryManager {
 public:
  explicit MemoryManager(int argc, const char** argv);

  /// @name 构造/析构函数
  /// @{
  MemoryManager() = default;
  MemoryManager(const MemoryManager&) = delete;
  MemoryManager(MemoryManager&&) = default;
  auto operator=(const MemoryManager&) -> MemoryManager& = delete;
  auto operator=(MemoryManager&&) -> MemoryManager& = default;
  ~MemoryManager() = default;
  /// @}

  /**
   * @brief 分配物理内存页
   * @param pages 要分配的页数
   * @return void* 分配的物理地址，失败时返回 nullptr
   */
  void* AllocatePhysicalPages(size_t pages);

  /**
   * @brief 释放物理内存页
   * @param addr 要释放的物理地址
   * @param pages 要释放的页数
   */
  void FreePhysicalPages(void* addr, size_t pages);

  /**
   * @brief 分配内核堆内存
   * @param size 要分配的字节数
   * @return void* 分配的虚拟地址，失败时返回 nullptr
   */
  void* AllocateKernelMemory(size_t size);

  /**
   * @brief 释放内核堆内存
   * @param addr 要释放的虚拟地址
   */
  void FreeKernelMemory(void* addr);

  /**
   * @brief 创建虚拟内存映射
   * @param virtual_addr 虚拟地址
   * @param physical_addr 物理地址
   * @param size 映射大小（字节）
   * @param protection 内存保护标志
   * @param type 内存类型
   * @return true 映射成功
   * @return false 映射失败
   */
  bool MapVirtualMemory(void* virtual_addr, void* physical_addr, size_t size,
                        MemoryProtection protection, MemoryType type);

  /**
   * @brief 取消虚拟内存映射
   * @param virtual_addr 虚拟地址
   * @param size 取消映射的大小（字节）
   * @return true 取消映射成功
   * @return false 取消映射失败
   */
  bool UnmapVirtualMemory(void* virtual_addr, size_t size);

  /**
   * @brief 虚拟地址转物理地址
   * @param virtual_addr 虚拟地址
   * @return void* 对应的物理地址，失败时返回 nullptr
   */
  void* VirtualToPhysical(void* virtual_addr);

  /**
   * @brief 物理地址转虚拟地址（内核空间）
   * @param physical_addr 物理地址
   * @return void* 对应的虚拟地址
   */
  void* PhysicalToVirtual(void* physical_addr);

  /**
   * @brief 设置页目录基址
   * @param page_directory 页目录物理地址
   */
  void SetPageDirectory(void* page_directory);

  /**
   * @brief 获取当前页目录基址
   * @return void* 页目录物理地址
   */
  void* GetPageDirectory();

  /**
   * @brief 启用分页机制
   */
  void EnablePaging();

  /**
   * @brief 禁用分页机制
   */
  void DisablePaging();

  /**
   * @brief 获取内存使用统计
   * @param total_pages 总页数
   * @param used_pages 已使用页数
   * @param free_pages 空闲页数
   */
  void GetMemoryStatistics(size_t& total_pages, size_t& used_pages,
                           size_t& free_pages);

  /**
   * @brief 检查内存地址是否有效
   * @param addr 要检查的地址
   * @return true 地址有效
   * @return false 地址无效
   */
  bool IsValidAddress(void* addr);

 private:
  MemoryManager() = default;
  ~MemoryManager() = default;

  /**
   * @brief 创建页表项
   * @param physical_addr 物理地址
   * @param protection 内存保护标志
   * @param type 内存类型
   * @return uint64_t 页表项值
   */
  uint64_t CreatePageTableEntry(void* physical_addr,
                                MemoryProtection protection, MemoryType type);

  /**
   * @brief 分配页表
   * @return void* 页表物理地址，失败时返回 nullptr
   */
  void* AllocatePageTable();

  /**
   * @brief 获取或创建页表项
   * @param page_directory 页目录地址
   * @param virtual_addr 虚拟地址
   * @param level 页表级别
   * @param allocate 是否分配新页表
   * @return uint64_t* 页表项指针，失败时返回 nullptr
   */
  uint64_t* GetOrCreatePageTableEntry(void* page_directory, void* virtual_addr,
                                      int level, bool allocate);

  // 内存管理锁
  SpinLock memory_lock_;

  // 物理内存分配器（使用 bmalloc）
  using PhysicalAllocator = bmalloc::Bmalloc<>;
  std::unique_ptr<PhysicalAllocator> physical_allocator_;

  // 内核堆分配器
  using KernelAllocator = bmalloc::Bmalloc<>;
  std::unique_ptr<KernelAllocator> kernel_allocator_;

  // 内存区域信息
  void* physical_memory_start_;
  size_t physical_memory_size_;
  void* kernel_heap_start_;
  size_t kernel_heap_size_;

  // 页目录地址
  void* current_page_directory_;

  // 初始化状态
  bool initialized_;

  // 常量定义
  static constexpr size_t kPageSize = 4096;
  static constexpr size_t kKernelHeapSize = 16 * 1024 * 1024;  // 16MB 内核堆
  static constexpr uintptr_t kKernelBaseVirtual =
      0xFFFFFF8000000000ULL;  // 内核虚拟基址
};

#endif  // SIMPLEKERNEL_SRC_INCLUDE_MEMORY_MANAGER_HPP_
