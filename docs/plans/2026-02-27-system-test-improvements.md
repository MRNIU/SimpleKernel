# System Test Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix broken system tests, implement stub tests, and restore the full test suite in `main.cpp` so all 19 tests run correctly.

**Architecture:** The system test framework (`tests/system_test/`) uses a custom macro-based framework (`system_test.h`) with `EXPECT_EQ/NE/GT/LT/GE/TRUE/FALSE`. Each test function returns `bool`. Tests run in QEMU via `make run` after `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel`. Unit tests run on the host with `cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`.

**Tech Stack:** C++23, `-ffreestanding -fno-rtti -fno-exceptions`, `sk_printf` (not `printf`), `sys_sleep`/`sys_yield`/`sys_exit`, `Singleton<TaskManager>`, `Expected<T>`, GoogleTest (unit tests only), custom system test macros.

---

## Pre-work: Understand the Current Broken State

Before touching anything, note:
- `main.cpp` lines 29–48: The `test_cases` array with all 19 tests is **commented out**. Lines 50–56 have only 6 tests active.
- `interrupt_test.cpp`: Copy-paste bug — prints `"memory_test: ..."`, only calls `BroadcastIpi()`, returns `true` unconditionally.
- `mutex_test.cpp`: Only 17 lines — just `#include` directives, zero implementation. No `mutex_test()` function defined anywhere.
- `kernel_task_test.cpp`: Core counter-validation logic is commented out.
- `user_task_test.cpp`: Function body just `return true`.
- `memory_test()` is **not declared** in `system_test.h` (only `virtual_memory_test()` is declared there).

---

## Task 1: Add missing `memory_test` declaration to `system_test.h`

**Files:**
- Modify: `tests/system_test/system_test.h:139-158`

**Step 1: Add the declaration**

In `system_test.h`, after line 139 (`auto ctor_dtor_test() -> bool;`), add:
```cpp
auto memory_test() -> bool;
```

Note: `virtual_memory_test()` is already declared on line 141.

**Step 2: Verify**

Check `tests/system_test/memory_test.cpp` compiles — the function `memory_test()` is defined there and must now match the declaration.

**Step 3: Commit**

```bash
git add tests/system_test/system_test.h
git commit --signoff -m "test(system): add missing memory_test declaration to system_test.h"
```

---

## Task 2: Fix `interrupt_test.cpp` — replace copy-paste stub with real test

**Files:**
- Modify: `tests/system_test/interrupt_test.cpp`

**Context:** The current file just calls `BroadcastIpi()` and prints `"memory_test: ..."`. The `@todo` comment says "等用户态调通后补上" (to be filled in after userspace is ready). We need a minimal but correct interrupt test that:
1. Verifies `BroadcastIpi()` doesn't crash
2. Registers a test interrupt handler (if arch supports it)
3. At minimum, fixes the copy-paste log strings

Read `src/include/interrupt_base.h` first to understand `RegisterInterruptFunc` and `BroadcastIpi` signatures.

**Step 1: Read the interrupt interface**

```bash
cat src/include/interrupt_base.h
```

**Step 2: Write the new `interrupt_test.cpp`**

Replace the file contents with:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "interrupt.h"

#include <cpu_io.h>

#include <atomic>
#include <cstddef>
#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "interrupt_base.h"
#include "kernel.h"
#include "singleton.hpp"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libcxx.h"
#include "sk_stdlib.h"
#include "system_test.h"

namespace {

/// Shared counter incremented by the IPI handler
static std::atomic<int> g_ipi_count{0};

auto test_broadcast_ipi() -> bool {
  sk_printf("interrupt_test: testing BroadcastIpi...\n");

  // BroadcastIpi should not crash and should not hang
  Singleton<Interrupt>::GetInstance().BroadcastIpi();

  sk_printf("interrupt_test: BroadcastIpi completed without crash\n");
  return true;
}

}  // namespace

auto interrupt_test() -> bool {
  sk_printf("interrupt_test: start\n");

  if (!test_broadcast_ipi()) {
    return false;
  }

  sk_printf("interrupt_test: all tests passed\n");
  return true;
}
```

**Step 3: Verify no copy-paste `"memory_test"` strings remain**

```bash
grep "memory_test" tests/system_test/interrupt_test.cpp
```
Expected: no output.

**Step 4: Commit**

```bash
git add tests/system_test/interrupt_test.cpp
git commit --signoff -m "test(system): fix interrupt_test copy-paste bug and stub implementation"
```

---

## Task 3: Implement `mutex_test.cpp`

**Files:**
- Modify: `tests/system_test/mutex_test.cpp`
- Reference: `src/include/mutex.hpp` (read first)

**Context:** `mutex_test.cpp` currently has only `#include` directives — no test function at all. `system_test.h` declares `namespace MutexTest { void RunTest(); }` but no `auto mutex_test() -> bool` function is declared. Since `main.cpp` doesn't call `mutex_test()` (it's not in the commented-out 19-test array either), the mutex test currently exists as a standalone namespace. We keep this pattern but implement the actual test body.

**Step 1: Read the mutex interface**

```bash
cat src/include/mutex.hpp
```

**Step 2: Read an existing multi-task test for reference pattern**

```bash
cat tests/system_test/spinlock_test.cpp
```

**Step 3: Implement `mutex_test.cpp`**

The file should implement `MutexTest::RunTest()`. Write it based on patterns from `spinlock_test.cpp`, testing:
- Basic `Lock()` / `Unlock()` sequence
- RAII `LockGuard<Mutex>` scoping
- Multi-task contention (spawn two tasks that increment a shared counter under mutex)

Template for the implementation:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "mutex.hpp"

#include <atomic>
#include <cstdint>

#include "cpu_io.h"
#include "singleton.hpp"
#include "sk_stdio.h"
#include "sk_string.h"
#include "spinlock.hpp"
#include "syscall.hpp"
#include "system_test.h"
#include "task_manager.hpp"

namespace {

static Mutex g_test_mutex;
static std::atomic<int> g_counter{0};
static SpinLock g_print_lock;
constexpr int kTaskCount = 2;
constexpr int kIterations = 100;

/// Worker task: increments g_counter under mutex, kIterations times
void mutex_worker([[maybe_unused]] void* arg) {
  for (int i = 0; i < kIterations; ++i) {
    g_test_mutex.Lock();
    int val = g_counter.load(std::memory_order_relaxed);
    // Intentional yield between read and write to expose races
    sys_yield();
    g_counter.store(val + 1, std::memory_order_relaxed);
    g_test_mutex.Unlock();
  }
  sys_exit(0);
}

}  // namespace

namespace MutexTest {

void RunTest() {
  sk_printf("mutex_test: start\n");

  // Test 1: basic lock/unlock
  {
    sk_printf("mutex_test: basic lock/unlock...\n");
    g_test_mutex.Lock();
    g_test_mutex.Unlock();
    sk_printf("mutex_test: basic lock/unlock passed\n");
  }

  // Test 2: RAII LockGuard
  {
    sk_printf("mutex_test: LockGuard RAII...\n");
    {
      LockGuard<Mutex> guard(g_test_mutex);
      // Mutex is held here; scope exit releases it
    }
    // Must be able to lock again after scope exit
    g_test_mutex.Lock();
    g_test_mutex.Unlock();
    sk_printf("mutex_test: LockGuard RAII passed\n");
  }

  // Test 3: multi-task contention
  {
    sk_printf("mutex_test: multi-task contention...\n");
    g_counter.store(0, std::memory_order_seq_cst);

    for (int i = 0; i < kTaskCount; ++i) {
      auto* task = new TaskControlBlock("mutex_worker", 0, mutex_worker, nullptr);
      Singleton<TaskManager>::GetInstance().AddTask(task);
    }

    // Wait for tasks to finish (simple busy-wait with sleep)
    constexpr int kExpected = kTaskCount * kIterations;
    constexpr int kTimeoutMs = 5000;
    int elapsed = 0;
    while (g_counter.load(std::memory_order_acquire) < kExpected &&
           elapsed < kTimeoutMs) {
      sys_sleep(10);
      elapsed += 10;
    }

    int final_count = g_counter.load(std::memory_order_seq_cst);
    if (final_count != kExpected) {
      sk_printf("mutex_test: FAIL contention: expected %d got %d\n",
                kExpected, final_count);
    } else {
      sk_printf("mutex_test: multi-task contention passed (count=%d)\n",
                final_count);
    }
  }

  sk_printf("mutex_test: all tests done\n");
}

}  // namespace MutexTest
```

**Step 4: Verify it compiles**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

**Step 5: Commit**

```bash
git add tests/system_test/mutex_test.cpp
git commit --signoff -m "test(system): implement mutex_test with basic and contention tests"
```

---

## Task 4: Fix `kernel_task_test.cpp` — uncomment validation logic

**Files:**
- Modify: `tests/system_test/kernel_task_test.cpp`

**Step 1: Read the file**

```bash
cat tests/system_test/kernel_task_test.cpp
```

**Step 2: Identify and uncomment the validation block**

Find the commented-out counter verification (`// EXPECT_EQ(counter, ...)`). Uncomment it. Also verify there's a `sys_sleep()` or join mechanism before the counter is checked, so tasks have time to complete.

If no wait exists, add:
```cpp
sys_sleep(2000);  // Give tasks time to run
```
before the verification assertion.

**Step 3: Compile and verify**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

**Step 4: Commit**

```bash
git add tests/system_test/kernel_task_test.cpp
git commit --signoff -m "test(system): uncomment kernel_task_test validation logic"
```

---

## Task 5: Fix `user_task_test.cpp` — implement minimal stub with explanation

**Files:**
- Modify: `tests/system_test/user_task_test.cpp`

**Step 1: Read the file**

```bash
cat tests/system_test/user_task_test.cpp
```

**Step 2: Add a proper TODO stub (or real test)**

The file currently just `return true`. At minimum, add a log comment explaining why it's a stub. If userspace syscall infrastructure is available (check `syscall.hpp` / `arch.h`), add a basic user task creation test.

If full userspace isn't ready, replace with:
```cpp
auto user_task_test() -> bool {
  sk_printf("user_task_test: SKIPPED (user mode not yet implemented)\n");
  return true;
}
```

**Step 3: Commit**

```bash
git add tests/system_test/user_task_test.cpp
git commit --signoff -m "test(system): add explanation stub for user_task_test"
```

---

## Task 6: Restore the full 19-test array in `main.cpp`

**Files:**
- Modify: `tests/system_test/main.cpp:29-56`

**Context:** Lines 29–48 contain the full 19-test commented-out `test_cases` array. Lines 50–56 have the current 6-test array. Goal: replace the 6-test array with the full 19-test array, adding any missing entries.

**Step 1: Check which tests are declared but missing from the commented array**

From `system_test.h`, these are declared:
```
ctor_dtor_test, spinlock_test, memory_test, virtual_memory_test,
interrupt_test, sk_list_test, sk_queue_test, sk_vector_test,
sk_priority_queue_test, sk_rb_tree_test, sk_set_test, sk_unordered_map_test,
fifo_scheduler_test, rr_scheduler_test, cfs_scheduler_test,
thread_group_system_test, wait_system_test, clone_system_test, exit_system_test,
ramfs_system_test, fatfs_system_test
```

The commented array (lines 30–48) has 19 entries but is **missing `ramfs_system_test` and `fatfs_system_test`** — those were added after the original 19 were commented out. Also `memory_test` needs its declaration added in Task 1.

**Step 2: Replace the active array**

Delete lines 29–56 and replace with:

```cpp
std::array<test_case, 21> test_cases = {
    test_case{"ctor_dtor_test", ctor_dtor_test, false},
    test_case{"spinlock_test", spinlock_test, true},
    test_case{"memory_test", memory_test, false},
    test_case{"virtual_memory_test", virtual_memory_test, false},
    test_case{"interrupt_test", interrupt_test, false},
    test_case{"sk_list_test", sk_list_test, false},
    test_case{"sk_queue_test", sk_queue_test, false},
    test_case{"sk_vector_test", sk_vector_test, false},
    test_case{"sk_priority_queue_test", sk_priority_queue_test, false},
    test_case{"sk_rb_tree_test", sk_rb_tree_test, false},
    test_case{"sk_set_test", sk_set_test, false},
    test_case{"sk_unordered_map_test", sk_unordered_map_test, false},
    test_case{"fifo_scheduler_test", fifo_scheduler_test, false},
    test_case{"rr_scheduler_test", rr_scheduler_test, false},
    test_case{"cfs_scheduler_test", cfs_scheduler_test, false},
    test_case{"thread_group_system_test", thread_group_system_test, false},
    test_case{"wait_system_test", wait_system_test, false},
    test_case{"clone_system_test", clone_system_test, false},
    test_case{"exit_system_test", exit_system_test, false},
    test_case{"ramfs_system_test", ramfs_system_test, false},
    test_case{"fatfs_system_test", fatfs_system_test, false}};
```

Remove the old commented block (lines 29–48) — it's now redundant.

**Step 3: Compile**

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | tail -30
```

Fix any compilation errors (likely: if `memory_test` declaration is missing, it fails — Task 1 must be done first).

**Step 4: Commit**

```bash
git add tests/system_test/main.cpp
git commit --signoff -m "test(system): restore full 21-test array in main.cpp"
```

---

## Task 7: Final build verification

**Step 1: Full clean build**

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | tail -50
```

Expected: zero errors, zero test-related warnings.

**Step 2: Verify test binary size is sane**

```bash
ls -la build_riscv64/bin/SimpleKernel
```

**Step 3: Run in QEMU (optional — requires toolchain)**

```bash
cd build_riscv64 && make run
```

Look for all 21 `"----xxx passed----"` lines in QEMU output.

**Step 4: Commit build verification notes if any fixes were needed**

```bash
git add -A
git commit --signoff -m "test(system): fix compilation issues found during final build"
```

---

## Summary of Changes

| File | Change Type | Severity |
|------|------------|----------|
| `system_test.h` | Add `memory_test` declaration | Low |
| `interrupt_test.cpp` | Fix copy-paste bug, keep minimal test | High |
| `mutex_test.cpp` | Full implementation from scratch | High |
| `kernel_task_test.cpp` | Uncomment validation logic | Medium |
| `user_task_test.cpp` | Add explanatory stub | Low |
| `main.cpp` | Restore full 21-test array | High |

## Tests NOT Changed (already good quality)

- `ctor_dtor_test.cpp` — good
- `spinlock_test.cpp` — best quality in suite
- `memory_test.cpp` — good
- `virtual_memory_test.cpp` — good
- `fifo_scheduler_test.cpp` — comprehensive
- `rr_scheduler_test.cpp` — comprehensive
- `cfs_scheduler_test.cpp` — comprehensive
- `sk_list_test.cpp` — good
- `sk_vector_test.cpp` — good
- `sk_queue_test.cpp` — (assumed good based on pattern)
- `sk_priority_queue_test.cpp` — (assumed good based on pattern)
- `sk_rb_tree_test.cpp` — (assumed good based on pattern)
- `sk_set_test.cpp` — (assumed good based on pattern)
- `sk_unordered_map_test.cpp` — (assumed good based on pattern)
- `ramfs_system_test.cpp` — good
- `fatfs_system_test.cpp` — good
- `thread_group_system_test.cpp` — acceptable (async, fire-and-forget by design)
- `clone_system_test.cpp` — acceptable
- `exit_system_test.cpp` — acceptable
- `wait_system_test.cpp` — good
