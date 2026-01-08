/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "spinlock.hpp"
#include "system_test.h"

class TestSpinLock : public SpinLock {
 public:
  using SpinLock::IsLockedByCurrentCore;
  using SpinLock::SpinLock;
};

static auto test_basic_lock() -> bool {
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

static auto test_recursive_lock() -> bool {
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

static auto test_lock_guard() -> bool {
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

auto spinlock_test() -> bool {
  bool ret = true;
  sk_printf("Starting spinlock_test\n");
  ret = ret && test_basic_lock();
  ret = ret && test_recursive_lock();
  ret = ret && test_lock_guard();
  if (ret) {
    sk_printf("spinlock_test passed\n");
  } else {
    sk_printf("spinlock_test failed\n");
  }
  return ret;
}
