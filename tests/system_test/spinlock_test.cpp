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
  // lock() 如果已经被当前核心锁定则返回 false
  if (lock.lock()) {
    sk_printf("FAIL: Recursive lock should return false\n");
    lock.unlock();  // 尝试恢复
    lock.unlock();
    return false;
  }

  EXPECT_TRUE(lock.unlock(), "Unlock failed in recursive test");
  // 再次解锁应该失败
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

auto test_interrupt_restore() -> bool {
  sk_printf("Running test_interrupt_restore...\n");
  TestSpinLock lock("intr");

  // Case 1: Interrupts enabled
  cpu_io::EnableInterrupt();
  if (!cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Failed to enable interrupts\n");
    return false;
  }

  lock.lock();
  if (cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Lock didn't disable interrupts\n");
    lock.unlock();
    return false;
  }
  lock.unlock();

  if (!cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Unlock didn't restore interrupts (expected enabled)\n");
    return false;
  }

  // Case 2: Interrupts disabled
  cpu_io::DisableInterrupt();
  // Ensure disabled
  if (cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Failed to disable interrupts for test\n");
    return false;
  }

  lock.lock();
  if (cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Lock enabled interrupts unexpectedly\n");
    lock.unlock();
    cpu_io::EnableInterrupt();
    return false;
  }
  lock.unlock();

  if (cpu_io::GetInterruptStatus()) {
    sk_printf("FAIL: Unlock enabled interrupts (expected disabled)\n");
    cpu_io::EnableInterrupt();
    return false;
  }

  cpu_io::EnableInterrupt();  // Cleanup
  sk_printf("test_interrupt_restore passed\n");
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

constexpr int STR_BUFFER_SIZE = 512 * 1024;
// 使用更大的缓冲区以容纳所有字符串
char shared_str_buffer[STR_BUFFER_SIZE];
int str_buffer_offset = 0;
// 单独的锁用于此测试
SpinLock str_lock("str_lock");
std::atomic<int> str_test_finished_cores = 0;
std::atomic<int> str_test_start_barrier = 0;

auto spinlock_smp_string_test() -> bool {
  size_t core_id = cpu_io::GetCurrentCoreId();
  size_t core_count = Singleton<BasicInfo>::GetInstance().core_count;

  // 需求 1: 确保核心数大于 1
  if (core_count < 2) {
    if (core_id == 0) {
      sk_printf("Skipping SMP string test: need more than 1 core.\n");
    }
    return true;
  }

  // Barrier: 等待所有核心到达此处
  // 确保它们大致同时开始写入以引起竞争
  str_test_start_barrier.fetch_add(1);
  while (str_test_start_barrier.load() < (int)core_count) {
    ;
  }

  int writes_per_core = 500;
  char local_buf[128];

  for (int i = 0; i < writes_per_core; ++i) {
    // 2. 先写入可区分的字符串到本地缓冲区
    // 增加数据长度以增加临界区持续时间
    int len = sk_snprintf(local_buf, sizeof(local_buf),
                          "[C:%d-%d|LongStringPaddingForContention]",
                          (int)core_id, i);

    str_lock.lock();
    if (str_buffer_offset + len < STR_BUFFER_SIZE - 1) {
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
    // 3. 验证
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

      // 应该找到匹配的 ']'
      int end_idx = current_idx + 1;
      bool closed = false;
      while (end_idx < str_buffer_offset) {
        if (shared_str_buffer[end_idx] == ']') {
          closed = true;
          break;
        }
        if (shared_str_buffer[end_idx] == '[') {
          // 在结束前遇到另一个开始 -> 数据损坏/交错
          break;
        }
        end_idx++;
      }

      if (!closed) {
        failed = true;
        sk_printf("FAIL: Broken token starting at %d\n", current_idx);
        break;
      }

      // 验证内容格式 C:ID-Seq
      if (shared_str_buffer[current_idx + 1] != 'C' ||
          shared_str_buffer[current_idx + 2] != ':') {
        failed = true;
        sk_printf("FAIL: Invalid content in token at %d\n", current_idx);
        break;
      }

      // 验证填充完整性
      const char *padding = "|LongStringPaddingForContention";
      int padding_len = 31;  // "|LongStringPaddingForContention" 的长度
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
    // 如果缓冲区太小可能会耗尽，但通常我们应该期望完全匹配。

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

  // 单元测试仅在核心 0 上运行，以避免日志混乱
  if (core_id == 0) {
    sk_printf("Starting spinlock_test\n");
    ret = ret && test_basic_lock();
    ret = ret && test_recursive_lock();
    ret = ret && test_lock_guard();
    ret = ret && test_interrupt_restore();
  }

  // SMP (多核) 测试在所有核心上运行
  // 使用顺序执行以确保如果前一个测试失败，屏障不会死锁
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
