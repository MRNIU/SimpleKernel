/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "spinlock.hpp"

#include <atomic>
#include <cstdint>

#include "basic_info.hpp"
#include "cpu_io.h"
#include "sk_stdio.h"
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
constexpr int BUFFER_SIZE = 8192;
int shared_buffer[BUFFER_SIZE];
int buffer_index = 0;
SpinLock buffer_lock("buffer_lock");
std::atomic<int> buffer_test_finished_cores = 0;

auto spinlock_smp_buffer_test() -> bool {
  // 每个核心尝试写入一定次数
  int writes_per_core = 500;

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

// STRING Test
constexpr int STR_BUFFER_SIZE = 512 * 1024;
// Use a larger buffer to hold all strings
char shared_str_buffer[STR_BUFFER_SIZE];
int str_buffer_offset = 0;
// A separate lock for this test
SpinLock str_lock("str_lock");
std::atomic<int> str_test_finished_cores = 0;
std::atomic<int> str_test_start_barrier = 0;

auto spinlock_smp_string_test() -> bool {
  size_t core_id = cpu_io::GetCurrentCoreId();
  size_t core_count = Singleton<BasicInfo>::GetInstance().core_count;

  // Requirement 1: Ensure more than one core
  if (core_count < 2) {
    if (core_id == 0) {
      sk_printf("Skipping SMP string test: need more than 1 core.\n");
    }
    return true;
  }

  // Barrier: Wait for all cores to arrive here
  // This ensures they start writing roughly at the same time to cause
  // contention.
  str_test_start_barrier.fetch_add(1);
  while (str_test_start_barrier.load() < (int)core_count) {
    ;
  }

  int writes_per_core = 500;
  char local_buf[128];

  for (int i = 0; i < writes_per_core; ++i) {
    // 2. Write distinguishable string to local buffer first
    // Increase data length to increase critical section duration
    int len = sk_snprintf(local_buf, sizeof(local_buf),
                          "[C:%d-%d|LongStringPaddingForContention]",
                          (int)core_id, i);

    // Critical Section
    str_lock.lock();
    if (str_buffer_offset + len < STR_BUFFER_SIZE - 1) {
      // Copy char by char
      for (int k = 0; k < len; ++k) {
        shared_str_buffer[str_buffer_offset + k] = local_buf[k];
      }
      str_buffer_offset += len;
      shared_str_buffer[str_buffer_offset] = '\0';
    }
    str_lock.unlock();
  }

  int finished = str_test_finished_cores.fetch_add(1) + 1;
  if (finished == (int)core_count) {
    // 3. Verify
    sk_printf(
        "All cores finished string writes. Verifying string integrity...\n");
    bool failed = false;
    int current_idx = 0;
    int tokens_found = 0;

    while (current_idx < str_buffer_offset) {
      if (shared_str_buffer[current_idx] != '[') {
        failed = true;
        sk_printf("FAIL: Expected '[' at %d, got '%c'\n", current_idx,
                  shared_str_buffer[current_idx]);
        break;
      }

      // Should find matching ']'
      int end_idx = current_idx + 1;
      bool closed = false;
      while (end_idx < str_buffer_offset) {
        if (shared_str_buffer[end_idx] == ']') {
          closed = true;
          break;
        }
        if (shared_str_buffer[end_idx] == '[') {
          // Encountered another start before end -> corruption/interleaving
          break;
        }
        end_idx++;
      }

      if (!closed) {
        failed = true;
        sk_printf("FAIL: Broken token starting at %d\n", current_idx);
        break;
      }

      // Verify content format slightly C:ID-Seq
      if (shared_str_buffer[current_idx + 1] != 'C' ||
          shared_str_buffer[current_idx + 2] != ':') {
        failed = true;
        sk_printf("FAIL: Invalid content in token at %d\n", current_idx);
        break;
      }

      // Verify padding integrity
      const char *padding = "|LongStringPaddingForContention";
      int padding_len = 31;  // Length of "|LongStringPaddingForContention"
      int token_content_len = end_idx - current_idx - 1;
      bool padding_ok = true;

      if (token_content_len < padding_len) {
        padding_ok = false;
      } else {
        int padding_start = end_idx - padding_len;
        for (int p = 0; p < padding_len; ++p) {
          if (shared_str_buffer[padding_start + p] != padding[p]) {
            padding_ok = false;
            break;
          }
        }
      }

      if (!padding_ok) {
        failed = true;
        sk_printf("FAIL: Broken padding in token at %d. Content len: %d\n",
                  current_idx, token_content_len);
        break;
      }

      tokens_found++;
      current_idx = end_idx + 1;
    }

    int expected_tokens = writes_per_core * core_count;
    // It is possible buffer runs out if too small, but reasonably we should
    // match If buffer is large enough we expect exact match.

    if (tokens_found != expected_tokens) {
      failed = true;
      sk_printf("FAIL: Expected %d tokens, found %d\n", expected_tokens,
                tokens_found);
    }

    if (!failed) {
      sk_printf("String test passed. Length: %d, Tokens: %d\n",
                str_buffer_offset, tokens_found);
    }
    return !failed;
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
  // Use sequential execution to ensure barriers don't deadlock if a previous
  // test fails
  if (!spinlock_smp_test()) ret = false;
  if (!spinlock_smp_buffer_test()) ret = false;
  if (!spinlock_smp_string_test()) ret = false;

  if (core_id == 0) {
    if (ret) {
      sk_printf("spinlock_test passed\n");
    } else {
      sk_printf("spinlock_test failed\n");
    }
  }
  return ret;
}
