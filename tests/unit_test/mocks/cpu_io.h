/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_CPU_IO_H_
#define SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_CPU_IO_H_

#include <atomic>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <mutex>
#include <thread>
#include <unordered_map>

#include "test_environment_state.hpp"

namespace cpu_io {

inline void Pause() {
  // 在单元测试中使用 yield 避免死循环
  std::this_thread::yield();
}

// 使用线程 ID 映射到核心 ID（用于测试多核场景）
inline auto GetCurrentCoreId() -> size_t {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env &&
         "TestEnvironmentState not set for current thread. "
         "Did you forget to call SetCurrentThreadEnvironment()?");
  return env->GetCoreIdForThread(std::this_thread::get_id());
}

inline void EnableInterrupt() {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  core.interrupt_enabled = true;
}

inline void DisableInterrupt() {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  core.interrupt_enabled = false;
}

inline bool GetInterruptStatus() {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  return core.interrupt_enabled;
}

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
inline void SetPageDirectory(uint64_t pd) {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  core.page_directory = pd;
}

inline auto GetPageDirectory() -> uint64_t {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  return core.page_directory;
}

inline void EnablePage() {
  auto* env = test_env::TestEnvironmentState::GetCurrentThreadEnvironment();
  assert(env && "TestEnvironmentState not set for current thread");
  auto& core = env->GetCurrentCoreEnv();
  core.paging_enabled = true;
}

inline void FlushTLBAll() {
  // 在测试环境中不需要实际操作
}

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

struct TrapContext {
  uint64_t sp;
  uint64_t a0;
  uint64_t tp;

  uint64_t padding[61];

  // 统一的跨架构访问器方法
  __always_inline uint64_t& UserStackPointer() { return sp; }
  __always_inline uint64_t& ThreadPointer() { return tp; }
  __always_inline uint64_t& ReturnValue() { return a0; }
};

struct CalleeSavedContext {
  uint64_t ra;
  uint64_t sp;
  uint64_t s0;
  uint64_t s1;

  uint64_t padding[18];

  __always_inline uint64_t& ReturnAddress() { return ra; }
  __always_inline uint64_t& EntryFunction() { return s0; }
  __always_inline uint64_t& EntryArgument() { return s1; }
  __always_inline uint64_t& StackPointer() { return sp; }
};

}  // namespace cpu_io

#endif /* SIMPLEKERNEL_TESTS_UNIT_TEST_MOCKS_CPU_IO_H_ */
