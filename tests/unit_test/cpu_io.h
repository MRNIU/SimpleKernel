/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <thread>
#include <unordered_map>

namespace cpu_io {

// 使用内部函数的静态变量来避免多重定义
inline auto GetInterruptStatusRef() -> bool& {
  thread_local bool interrupt_enabled = true;
  return interrupt_enabled;
}

// 使用线程 ID 映射到核心 ID（用于测试多核场景）
inline auto GetCurrentCoreId() -> size_t {
  // 使用线程 ID 的哈希值作为核心 ID
  return std::hash<std::thread::id>{}(std::this_thread::get_id());
}

inline void EnableInterrupt() { GetInterruptStatusRef() = true; }

inline void DisableInterrupt() { GetInterruptStatusRef() = false; }

inline bool GetInterruptStatus() { return GetInterruptStatusRef(); }

namespace virtual_memory {

// 页大小常量
static constexpr size_t kPageSize = 4096;
static constexpr size_t kPteAttributeBits = 12;
static constexpr size_t kPageOffsetBits = 12;
static constexpr size_t kVpnBits = 9;
static constexpr size_t kVpnMask = 0x1FF;
static constexpr size_t kPageTableLevels = 4;

// 页表项权限位定义
static constexpr uint64_t kValid = 0x1;
static constexpr uint64_t kWrite = 0x2;
static constexpr uint64_t kUser = 0x4;
static constexpr uint64_t kRead = 0x200;
static constexpr uint64_t kExec = 0x400;
static constexpr uint64_t kGlobal = 0x100;

// 获取用户页面权限
inline auto GetUserPagePermissions(bool readable = true, bool writable = false,
                                   bool executable = false, bool global = false)
    -> uint64_t {
  uint64_t flags = kValid | kUser;
  if (readable) {
    flags |= kRead;
  }
  if (writable) {
    flags |= kWrite;
  }
  if (executable) {
    flags |= kExec;
  }
  if (global) {
    flags |= kGlobal;
  }
  return flags;
}

// 获取内核页面权限
inline auto GetKernelPagePermissions(bool readable = true,
                                     bool writable = false,
                                     bool executable = false,
                                     bool global = false) -> uint64_t {
  uint64_t flags = kValid;  // Kernel pages don't need kUser
  if (readable) {
    flags |= kRead;
  }
  if (writable) {
    flags |= kWrite;
  }
  if (executable) {
    flags |= kExec;
  }
  if (global) {
    flags |= kGlobal;
  }
  return flags;
}

// 页表操作函数
inline void SetPageDirectory(uint64_t) {}
inline void EnablePage() {}
inline void FlushTLBAll() {}

// 获取页表项权限
inline auto GetTableEntryPermissions() -> uint64_t {
  return kValid | kWrite | kUser | kRead | kExec;
}

// 获取虚拟页号
inline auto GetVirtualPageNumber(uint64_t virtual_addr, size_t level)
    -> uint64_t {
  return (virtual_addr >> (kPageOffsetBits + level * kVpnBits)) & kVpnMask;
}

// 页对齐函数
inline auto PageAlign(uint64_t addr) -> uint64_t {
  return addr & ~(kPageSize - 1);
}

inline auto PageAlignUp(uint64_t addr) -> uint64_t {
  return (addr + kPageSize - 1) & ~(kPageSize - 1);
}

inline auto IsPageAligned(uint64_t addr) -> bool {
  return (addr & (kPageSize - 1)) == 0;
}

// 页表项操作函数
inline auto IsPageTableEntryValid(uint64_t pte) -> bool {
  return (pte & kValid) != 0;
}

inline auto PageTableEntryToPhysical(uint64_t pte) -> uint64_t {
  return pte & 0x000FFFFFFFFFF000ULL;
}

inline auto PhysicalToPageTableEntry(uint64_t physical_addr, uint64_t flags)
    -> uint64_t {
  return (physical_addr & 0x000FFFFFFFFFF000ULL) | (flags & 0xFFF) |
         (flags & (1ULL << 63));
}

}  // namespace virtual_memory

}  // namespace cpu_io

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_CPU_IO_H_ */
