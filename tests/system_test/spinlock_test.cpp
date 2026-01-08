/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "spinlock.hpp"

#include <atomic>
#include <cstdint>

#include "basic_info.hpp"
#include "cpu_io.h"
#include "system_test.h"

namespace {
class TestSpinLock : public SpinLock {
 public:
  using SpinLock::IsLockedByCurrentCore;
  using SpinLock::SpinLock;
};

auto test_basic_lock() -> bool {
  sk_printf("Running test_basic_lock...\n");
  TestSpinLock lock("basic");
  EXPECT_TRUE(lock.lock(), "Basic lock failed");
  EXPECT_TRUE(lock.IsLockedByCurrentCore(),
              "IsLockedByCurrentCore failed after lock");
  EXPECT_TRUE(lock.unlock(), "Basic unlock failed");
  EXPECT_TRUE(!lock.IsLockedByCurrentCore(),
              "IsLockedByCurrentCore failed after unlock");
  sk_printf("test_basic_lock passed\n");
  return true;
}

auto test_recursive_lock() -> bool {
  sk_printf("Running test_recursive_lock...\n");
  TestSpinLock lock("recursive");
  EXPECT_TRUE(lock.lock(), "Lock failed in recursive test");
  // lock() returns false if already locked by current core
  if (lock.lock()) {
    sk_printf("FAIL: Recursive lock should return false\n");
    lock.unlock();  // try to recover
    lock.unlock();
    return false;
  }

  EXPECT_TRUE(lock.unlock(), "Unlock failed in recursive test");
  // Unlock again should fail
  if (lock.unlock()) {
    sk_printf("FAIL: Double unlock should return false\n");
    return false;
  }
  sk_printf("test_recursive_lock passed\n");
  return true;
}

auto test_lock_guard() -> bool {
  sk_printf("Running test_lock_guard...\n");
  TestSpinLock lock("guard");
  {
    LockGuard<TestSpinLock> guard(lock);
    EXPECT_TRUE(lock.IsLockedByCurrentCore(), "LockGuard failed to lock");
  }
  EXPECT_TRUE(!lock.IsLockedByCurrentCore(), "LockGuard failed to unlock");
  sk_printf("test_lock_guard passed\n");
  return true;
}

SpinLock smp_lock("smp_lock");
int shared_counter = 0;
std::atomic<int> finished_cores = 0;

auto spinlock_smp_test() -> bool {
  for (int i = 0; i < 10000; ++i) {
    smp_lock.lock();
    shared_counter++;
    smp_lock.unlock();
  }

  int finished = finished_cores.fetch_add(1) + 1;
  int total_cores = Singleton<BasicInfo>::GetInstance().core_count;

  if (finished == total_cores) {
    bool passed = (shared_counter == total_cores * 10000);
    if (passed) {
      sk_printf(" All cores finished. shared_counter = %d. OK.\n",
                shared_counter);
    } else {
      sk_printf(
          " All cores finished. shared_counter = %d. EXPECTED %d. FAIL.\n",
          shared_counter, total_cores * 10000);
    }
    return passed;
  }

  return true;
}

// BUFFER Test
constexpr int BUFFER_SIZE = 1024;
int shared_buffer[BUFFER_SIZE];
int buffer_index = 0;
SpinLock buffer_lock("buffer_lock");
std::atomic<int> buffer_test_finished_cores = 0;

auto spinlock_smp_buffer_test() -> bool {
  // 每个核心尝试写入一定次数
  int writes_per_core = 100;

  for (int i = 0; i < writes_per_core; ++i) {
    buffer_lock.lock();
    if (buffer_index < BUFFER_SIZE) {
      // 写入 Core ID
      shared_buffer[buffer_index++] = cpu_io::GetCurrentCoreId();
    }
    buffer_lock.unlock();
  }

  int finished = buffer_test_finished_cores.fetch_add(1) + 1;
  int total_cores = Singleton<BasicInfo>::GetInstance().core_count;

  if (finished == total_cores) {
    // 最后一个完成的核心进行检查
    sk_printf("All cores finished buffer writes. Checking buffer...\n");

    // 检查写入总数
    int expected_writes = writes_per_core * total_cores;
    if (expected_writes > BUFFER_SIZE) expected_writes = BUFFER_SIZE;

    if (buffer_index != expected_writes) {
      sk_printf("FAIL: Buffer index %d, expected %d\n", buffer_index,
                expected_writes);
      return false;
    }

    sk_printf("Buffer test passed. Final index: %d\n", buffer_index);
    return true;
  }

  return true;
}

}  // namespace

auto spinlock_test() -> bool {
  bool ret = true;
  size_t core_id = cpu_io::GetCurrentCoreId();

  // Unit tests run only on core 0 to avoid messy logs
  if (core_id == 0) {
    sk_printf("Starting spinlock_test\n");
    ret = ret && test_basic_lock();
    ret = ret && test_recursive_lock();
    ret = ret && test_lock_guard();
  }

  // SMP (Multi-core) tests run on all cores
  ret = ret && spinlock_smp_test();
  ret = ret && spinlock_smp_buffer_test();

  if (core_id == 0) {
    if (ret) {
      sk_printf("spinlock_test passed\n");
    } else {
      sk_printf("spinlock_test failed\n");
    }
  }
  return ret;
}
