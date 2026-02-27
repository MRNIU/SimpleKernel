# ETL delegate / fsm / flags Migration Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace bare function pointers (`InterruptFunc`), raw enum-based state transitions (`TaskStatus`), and bit-twiddling (`CloneFlags`, `cpu_affinity`) with type-safe ETL primitives (`etl::delegate`, `etl::fsm`, `etl::flags`).

**Architecture:** Three independent migration tracks that can be executed sequentially. Track 1 (delegate) touches interrupt subsystem. Track 2 (flags) touches task/clone/affinity subsystem. Track 3 (fsm) adds FSM on top of `TaskStatus`. Each track is independently committable.

**Tech Stack:** ETL (`3rd/etl/`), C++23, freestanding kernel environment, GoogleTest for host-only unit tests (`cmake --preset build_x86_64 && cd build_x86_64 && make unit-test`).

---

## Prerequisites

### P0: Understanding the lambda lifetime constraint

`etl::delegate` holds a **raw pointer** to the callable object. Lambdas stored as local variables in constructors will be destroyed after the constructor returns — the delegate then holds a dangling pointer.

**Solution pattern used throughout this plan:** Store lambda objects (and delegate objects) as `static` member variables in the class, or as class members (not on the stack). For `Interrupt` classes, the arrays of lambdas that are currently direct-assigned will become `static` arrays of stored lambdas, then wrapped into delegates.

### P1: Key file locations summary

| File | What changes |
|------|-------------|
| `src/include/etl_profile.h` | Remove `ETL_NO_CHECKS`, add `ETL_USE_ASSERT_FUNCTION` |
| `src/main.cpp` | Register `KernelEtlAssertHandler` after `ArchInit` |
| `src/include/interrupt_base.h` | Add `InterruptDelegate` type alias, update virtual method signatures |
| `src/arch/riscv64/interrupt.cpp` | Change `InterruptFunc` → `InterruptDelegate`, store lambdas statically |
| `src/arch/riscv64/interrupt.h` | Update `interrupt_handlers_` / `exception_handlers_` array types |
| `src/arch/aarch64/interrupt.cpp` + `.h` | Same treatment as riscv64 |
| `src/arch/x86_64/interrupt.cpp` + `.h` | Same treatment |
| `src/arch/riscv64/plic/include/plic.h` | `InterruptFunc` typedef → `InterruptDelegate`, update `interrupt_handlers_` |
| `src/arch/riscv64/plic/plic.cpp` | Update implementations |
| `src/task/include/task_control_block.hpp` | `CloneFlags` enum → `etl::flags`, `cpu_affinity` `uint64_t` → `etl::flags` |
| `src/task/clone.cpp` | Update `flags & kCloneXxx` → `.test()`, `static_cast<CloneFlags>` → direct ctor |
| `src/task/task_manager.cpp` | Update `cpu_affinity` bitwise → `.test()` / value comparisons |
| `src/main.cpp` | Update `cpu_affinity = (1UL << core_id)` → `CpuAffinity{1UL << core_id}` |
| `src/syscall.cpp` | `cpu_affinity` read/write: keep raw `uint64_t` conversion at syscall boundary |
| `src/task/include/task_messages.hpp` | **NEW** — FSM message type IDs |
| `src/task/include/task_fsm.hpp` | **NEW** — FSM state classes + `TaskFsm` host |
| `src/task/include/task_control_block.hpp` | Add `TaskFsm fsm` field |
| `src/task/schedule.cpp`, `exit.cpp`, `sleep.cpp`, `block.cpp`, `wakeup.cpp`, `tick_update.cpp` | `task->status = kXxx` → `task->fsm.receive(MsgXxx{})` |

---

## Track 1: etl::delegate — Type-safe Interrupt Callbacks

### Task 1.1: Update ETL profile and register assert handler

**Files:**
- Modify: `src/include/etl_profile.h`
- Modify: `src/main.cpp`

**Step 1: Update `etl_profile.h`**

Current content:
```cpp
#define ETL_CPP23_SUPPORTED 1
#define ETL_NO_CHECKS 1
```

Replace with:
```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ETL configuration for SimpleKernel freestanding environment.
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
#define SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_

// Use generic C++23 profile as base
#define ETL_CPP23_SUPPORTED 1

// Enable custom assert function instead of disabling checks entirely.
// This allows ETL runtime checks to call KernelEtlAssertHandler.
// Register the handler via etl::set_assert_function() in main.cpp after ArchInit.
#define ETL_USE_ASSERT_FUNCTION 1

#endif  // SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
```

**Step 2: Register ETL assert handler in `src/main.cpp`**

Find the `ArchInit(argc, argv);` call (around line 40) and add the handler registration directly after:

```cpp
#include <etl/error_handler.h>
// ... existing includes ...

// After ArchInit(), before anything else:
static void KernelEtlAssertHandler(const etl::exception& e) {
  klog::Err("ETL assert: file=%s line=%d what=%s\n",
             e.file_name(), static_cast<int>(e.line_number()), e.what());
  while (true) {
    cpu_io::Pause();
  }
}

// In kernel_main():
  ArchInit(argc, argv);
  etl::set_assert_function(KernelEtlAssertHandler);  // ADD THIS LINE
  // rest of init...
```

**Step 3: Run unit tests to verify no regression**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/include/etl_profile.h src/main.cpp
git commit --signoff -m "feat(etl): enable ETL_USE_ASSERT_FUNCTION, register kernel assert handler"
```

---

### Task 1.2: Add `InterruptDelegate` type alias to `interrupt_base.h`

**Files:**
- Modify: `src/include/interrupt_base.h`

**IMPORTANT:** Per `AGENTS.md`, **do NOT add implementation to interface headers**. We only add:
1. An `#include <etl/delegate.h>`
2. A `using InterruptDelegate = ...` type alias in the class
3. Updated pure-virtual method signatures

**Step 1: Read the current file** (already done — see line listing above)

**Step 2: Modify `interrupt_base.h`**

After `#include "expected.hpp"`, add:
```cpp
#include <etl/delegate.h>
```

Inside `class InterruptBase`, replace the `typedef` and the two virtual method signatures:

```cpp
// DELETE:
//   typedef uint64_t (*InterruptFunc)(uint64_t cause,
//                                     cpu_io::TrapContext* context);
//   virtual void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) = 0;
//   virtual auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
//                                          uint32_t priority,
//                                          InterruptFunc handler)
//       -> Expected<void> = 0;

// ADD:
  /**
   * @brief Type-safe interrupt/exception handler delegate.
   *        Zero heap allocation; holds raw pointer to callable object.
   * @warning The callable object MUST outlive this delegate.
   */
  using InterruptDelegate =
      etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

  /**
   * @brief Register an interrupt/exception handler delegate.
   * @param cause Interrupt/exception number (platform-specific).
   * @param func  Delegate wrapping the handler (object must outlive delegate).
   */
  virtual void RegisterInterruptFunc(uint64_t cause,
                                     InterruptDelegate func) = 0;

  /**
   * @brief Register an external interrupt handler delegate.
   * @param irq      External IRQ number (PLIC source_id / GIC INTID / APIC IRQ).
   * @param cpu_id   Target CPU core ID.
   * @param priority Interrupt priority.
   * @param handler  Delegate wrapping the handler (object must outlive delegate).
   * @return Expected<void>
   */
  virtual auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                         uint32_t priority,
                                         InterruptDelegate handler)
      -> Expected<void> = 0;
```

**Step 3: Check compilation (diagnostics only — full build requires cross-compiler)**

There will be compilation errors in all `.cpp` files that implemented these virtuals — that is expected and will be fixed in Tasks 1.3–1.5.

**Step 4: Commit the interface change alone**

```bash
git add src/include/interrupt_base.h
git commit --signoff -m "feat(interrupt): replace InterruptFunc bare pointer with etl::delegate"
```

---

### Task 1.3: Migrate RISC-V 64 `Interrupt` implementation

**Files:**
- Modify: `src/arch/riscv64/interrupt.h` (add `InterruptDelegate` type, update array type)
- Modify: `src/arch/riscv64/interrupt.cpp` (update arrays, lambda storage, `RegisterInterruptFunc`)

**Step 1: Read `src/arch/riscv64/interrupt.h`**

```bash
# Locate the header
grep -n "InterruptFunc\|interrupt_handlers_\|exception_handlers_" \
  src/arch/riscv64/interrupt.h
```

**Step 2: Update `interrupt.h`**

In `interrupt.h`, replace:
```cpp
// DELETE any local typedef of InterruptFunc (it now comes from InterruptBase)
// CHANGE array member types:

// Old:
static std::array<InterruptFunc, ...::kInterruptMaxCount> interrupt_handlers_;
static std::array<InterruptFunc, ...::kExceptionMaxCount> exception_handlers_;

// New (InterruptDelegate comes from InterruptBase):
static std::array<InterruptDelegate, ...::kInterruptMaxCount> interrupt_handlers_;
static std::array<InterruptDelegate, ...::kExceptionMaxCount> exception_handlers_;
```

Also update method signature:
```cpp
// Old:
void RegisterInterruptFunc(uint64_t cause, InterruptFunc func) override;
// New:
void RegisterInterruptFunc(uint64_t cause, InterruptDelegate func) override;
```

**Step 3: Update `interrupt.cpp`**

The constructor currently assigns lambdas directly:
```cpp
// OLD (problematic — lambda is a temporary rvalue):
for (auto& i : interrupt_handlers_) {
  i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t { ... };
}
```

`etl::delegate` forbids binding rvalues (to prevent dangling). The fix is to store default handler lambdas in `static` variables:

```cpp
// NEW — default handlers stored with static lifetime:
static auto kDefaultInterruptHandler =
    [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
  klog::Info("Default Interrupt handler [%s] 0x%X, 0x%p\n",
             cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames[cause],
             cause, context);
  return 0;
};

static auto kDefaultExceptionHandler =
    [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t {
  klog::Err("Default Exception handler [%s] 0x%X, 0x%p\n",
            cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames[cause],
            cause, context);
  while (true) { cpu_io::Pause(); }
};

Interrupt::Interrupt() {
  for (auto& i : interrupt_handlers_) {
    i = InterruptDelegate::create(kDefaultInterruptHandler);
  }
  for (auto& i : exception_handlers_) {
    i = InterruptDelegate::create(kDefaultExceptionHandler);
  }
  klog::Info("Interrupt init.\n");
}
```

Update `Do()` — the call syntax is unchanged (`interrupt_handlers_[idx](cause, ctx)` still works since `etl::delegate` has `operator()`).

Update `RegisterInterruptFunc()`:
```cpp
void Interrupt::RegisterInterruptFunc(uint64_t cause, InterruptDelegate func) {
  // ... existing bound checks ...
  if (interrupt) {
    interrupt_handlers_[exception_code] = func;
    klog::Info("RegisterInterruptFunc [%s] 0x%X\n", ...);
  } else {
    exception_handlers_[exception_code] = func;
    klog::Info("RegisterInterruptFunc [%s] 0x%X\n", ...);
  }
}
```

Update `RegisterExternalInterrupt()` — just update the `InterruptFunc handler` parameter to `InterruptDelegate handler`.

**Step 4: Check there are no calls passing raw function pointers to `RegisterInterruptFunc` in `src/arch/riscv64/interrupt_main.cpp`**

```bash
grep -n "RegisterInterruptFunc\|RegisterExternalInterrupt" \
  src/arch/riscv64/interrupt_main.cpp
```

In `interrupt_main.cpp`, lambdas are passed directly. Since `etl::delegate::create(lambda)` requires an lvalue, any lambda passed inline must become a named `static` local:

```cpp
// OLD:
interrupt.RegisterInterruptFunc(cause, [](uint64_t c, cpu_io::TrapContext* ctx) {
  ...
});

// NEW:
static auto kSomeHandler = [](uint64_t c, cpu_io::TrapContext* ctx) -> uint64_t {
  ...
};
interrupt.RegisterInterruptFunc(cause,
    InterruptBase::InterruptDelegate::create(kSomeHandler));
```

Apply this pattern to **every** lambda passed to `RegisterInterruptFunc` / `RegisterExternalInterrupt` in `interrupt_main.cpp`.

**Step 5: Commit**

```bash
git add src/arch/riscv64/interrupt.h src/arch/riscv64/interrupt.cpp \
        src/arch/riscv64/interrupt_main.cpp
git commit --signoff -m "feat(riscv64/interrupt): migrate to etl::delegate, store default handlers statically"
```

---

### Task 1.4: Migrate AArch64 `Interrupt` implementation

**Files:**
- Modify: `src/arch/aarch64/include/interrupt.h`
- Modify: `src/arch/aarch64/interrupt.cpp`

Follow the exact same pattern as Task 1.3:

1. **Read** the current `interrupt.h` and `interrupt.cpp` for aarch64.
2. In `interrupt.h`: change `interrupt_handlers_` array element type from `InterruptFunc` to `InterruptDelegate`. Update `RegisterInterruptFunc` signature.
3. In `interrupt.cpp`: convert inline lambdas in the constructor to `static` lambdas, create delegates with `InterruptDelegate::create(static_lambda)`.
4. Check `interrupt_main.cpp` for aarch64 (if it exists) and apply static-lambda pattern to all `RegisterInterruptFunc` calls.

**Step 5: Commit**

```bash
git add src/arch/aarch64/include/interrupt.h src/arch/aarch64/interrupt.cpp
git commit --signoff -m "feat(aarch64/interrupt): migrate to etl::delegate"
```

---

### Task 1.5: Migrate x86_64 `Interrupt` implementation

**Files:**
- Modify: `src/arch/x86_64/include/interrupt.h` (or equivalent)
- Modify: `src/arch/x86_64/interrupt.cpp`

Follow the exact same pattern as Task 1.3. Note: x86_64 uses template-heavy ISR registration — read the file carefully before changing.

**Step 5: Commit**

```bash
git add src/arch/x86_64/include/interrupt.h src/arch/x86_64/interrupt.cpp
git commit --signoff -m "feat(x86_64/interrupt): migrate to etl::delegate"
```

---

### Task 1.6: Migrate PLIC `InterruptFunc` to `InterruptDelegate`

**Files:**
- Modify: `src/arch/riscv64/plic/include/plic.h`
- Modify: `src/arch/riscv64/plic/plic.cpp`

**Step 1: Update `plic.h`**

Add `#include <etl/delegate.h>` at top (after the existing includes).

Replace the `typedef`:
```cpp
// DELETE:
//   typedef uint64_t (*InterruptFunc)(uint64_t cause,
//                                     cpu_io::TrapContext* context);

// ADD — reuse InterruptBase's type alias:
#include "interrupt_base.h"
using InterruptDelegate = InterruptBase::InterruptDelegate;
```

Update the `interrupt_handlers_` array member type:
```cpp
// OLD:
static std::array<InterruptFunc, kInterruptMaxCount> interrupt_handlers_;

// NEW:
static std::array<InterruptDelegate, kInterruptMaxCount> interrupt_handlers_;
```

Update the `RegisterInterruptFunc` method signature:
```cpp
// OLD:
void RegisterInterruptFunc(uint8_t cause, InterruptFunc func);
// NEW:
void RegisterInterruptFunc(uint8_t cause, InterruptDelegate func);
```

**Step 2: Update `plic.cpp`**

In the constructor (if any), convert default handler lambdas to `static` lambdas:
```cpp
static auto kDefaultPlicHandler =
    [](uint64_t cause, cpu_io::TrapContext* ctx) -> uint64_t {
  klog::Warn("PLIC: unhandled IRQ %lu\n", cause);
  return 0;
};

// In constructor or Init():
for (auto& h : interrupt_handlers_) {
  h = InterruptDelegate::create(kDefaultPlicHandler);
}
```

Update `RegisterInterruptFunc`:
```cpp
void Plic::RegisterInterruptFunc(uint8_t cause, InterruptDelegate func) {
  interrupt_handlers_[cause] = func;
}
```

In `Interrupt::RegisterExternalInterrupt()` in `interrupt.cpp` (riscv64), the call:
```cpp
Singleton<Plic>::GetInstance().RegisterInterruptFunc(
    static_cast<uint8_t>(irq), handler);
```
already receives an `InterruptDelegate` (since the riscv64 `interrupt.cpp` was updated in Task 1.3), so no change needed here.

**Step 3: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/arch/riscv64/plic/include/plic.h src/arch/riscv64/plic/plic.cpp
git commit --signoff -m "feat(riscv64/plic): migrate InterruptFunc to etl::delegate"
```

---

## Track 2: etl::flags — Type-safe Bit Flags

### Task 2.1: Replace `CloneFlags` enum with `etl::flags`

**Files:**
- Modify: `src/task/include/task_control_block.hpp`
- Modify: `src/task/clone.cpp`

**Step 1: Update `task_control_block.hpp`**

Add `#include <etl/flags.h>` near the top (after existing includes).

Replace the `CloneFlags` enum with a namespace + type alias:

```cpp
// DELETE:
// enum CloneFlags : uint64_t {
//   kCloneAll     = 0,
//   kCloneVm      = 0x00000100,
//   kCloneFs      = 0x00000200,
//   kCloneFiles   = 0x00000400,
//   kCloneSighand = 0x00000800,
//   kCloneParent  = 0x00008000,
//   kCloneThread  = 0x00010000,
// };

// ADD:
/**
 * @brief Clone flag bit values (used with sys_clone).
 * @note Matches Linux clone flag values for compatibility.
 */
namespace clone_flag {
  inline constexpr uint64_t kVm      = 0x00000100;  ///< Share address space
  inline constexpr uint64_t kFs      = 0x00000200;  ///< Share filesystem info
  inline constexpr uint64_t kFiles   = 0x00000400;  ///< Share file descriptor table
  inline constexpr uint64_t kSighand = 0x00000800;  ///< Share signal handlers
  inline constexpr uint64_t kParent  = 0x00008000;  ///< Keep same parent process
  inline constexpr uint64_t kThread  = 0x00010000;  ///< Same thread group
  /// Mask of all valid clone flags
  inline constexpr uint64_t kAllMask =
      kVm | kFs | kFiles | kSighand | kParent | kThread;
}  // namespace clone_flag

/**
 * @brief Type-safe clone flags, restricted to kAllMask.
 *        Use clone_flag::kXxx constants as bit values.
 */
using CloneFlags = etl::flags<uint64_t, clone_flag::kAllMask>;
```

In `struct TaskControlBlock`, update the default initialization:
```cpp
// OLD:
CloneFlags clone_flags = kCloneAll;

// NEW (default-constructed etl::flags has all bits 0, equivalent to kCloneAll=0):
CloneFlags clone_flags{};
```

**Step 2: Update `clone.cpp`**

Replace all `flags & kCloneXxx` patterns with `etl::flags::test()`:

```cpp
// The raw uint64_t `flags` parameter stays as-is (syscall boundary).
// Only the stored clone_flags field and checks within Clone() change.

// OLD:
child->clone_flags = static_cast<CloneFlags>(flags);

// NEW:
child->clone_flags = CloneFlags{flags};

// OLD:
if ((flags & kCloneThread) && (!(flags & kCloneVm) || ...))
  flags |= (kCloneVm | kCloneFiles | kCloneSighand);

// NEW: Keep working with raw `flags` uint64_t in the parameter checks
// (it's a syscall boundary — treat it as raw bits throughout this function).
// Only when storing to TCB use CloneFlags{}.
// The bitwise checks on the uint64_t parameter `flags` are fine to keep as-is.
```

> **Note:** The `flags` parameter in `Clone(uint64_t flags, ...)` is a raw `uint64_t` from the syscall interface. Bitwise operations on `flags` (the local `uint64_t`) throughout this function remain unchanged. Only `child->clone_flags = static_cast<CloneFlags>(flags)` changes to `child->clone_flags = CloneFlags{flags}`.

**Step 3: Update other callers of `clone_flags`**

Search for all uses of `child->clone_flags` or `task->clone_flags`:
```bash
grep -rn "clone_flags" src/
```

Update any `.clone_flags & kCloneXxx` reads to `.clone_flags.test(clone_flag::kXxx)`.

**Step 4: Write a unit test**

In `tests/unit_test/` (check existing test structure), add a test:

```cpp
// tests/unit_test/test_clone_flags.cpp
#include <gtest/gtest.h>
#include "task_control_block.hpp"

TEST(CloneFlagsTest, DefaultIsZero) {
  CloneFlags f{};
  EXPECT_FALSE(f.test(clone_flag::kVm));
  EXPECT_FALSE(f.test(clone_flag::kThread));
}

TEST(CloneFlagsTest, SetAndTest) {
  CloneFlags f{clone_flag::kVm | clone_flag::kThread};
  EXPECT_TRUE(f.test(clone_flag::kVm));
  EXPECT_TRUE(f.test(clone_flag::kThread));
  EXPECT_FALSE(f.test(clone_flag::kFs));
}

TEST(CloneFlagsTest, FromRawFlags) {
  uint64_t raw = clone_flag::kVm | clone_flag::kThread;
  CloneFlags f{raw};
  EXPECT_TRUE(f.test(clone_flag::kVm));
  EXPECT_TRUE(f.test(clone_flag::kThread));
}
```

**Step 5: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass including the new CloneFlags tests.

**Step 6: Commit**

```bash
git add src/task/include/task_control_block.hpp src/task/clone.cpp \
        tests/unit_test/test_clone_flags.cpp
git commit --signoff -m "feat(task): replace CloneFlags enum with etl::flags<uint64_t>"
```

---

### Task 2.2: Replace `cpu_affinity uint64_t` with `etl::flags`

**Files:**
- Modify: `src/task/include/task_control_block.hpp`
- Modify: `src/main.cpp`
- Modify: `src/task/task_manager.cpp`
- Modify: `src/syscall.cpp`

**Step 1: Add `CpuAffinity` type alias in `task_control_block.hpp`**

```cpp
// After CloneFlags definition:

/**
 * @brief Type-safe CPU affinity bitmask (one bit per logical CPU).
 *        Bit N = 1 means this task may run on CPU N.
 *        Default UINT64_MAX = any CPU.
 */
using CpuAffinity = etl::flags<uint64_t>;

// In struct TaskControlBlock, change:
// OLD:
uint64_t cpu_affinity = UINT64_MAX;

// NEW:
CpuAffinity cpu_affinity{UINT64_MAX};
```

**Step 2: Update `src/main.cpp`**

```cpp
// OLD:
task1->cpu_affinity = (1UL << core_id);
task2->cpu_affinity = (1UL << core_id);
task3->cpu_affinity = (1UL << core_id);
task4->cpu_affinity = (1UL << core_id);

// NEW:
task1->cpu_affinity = CpuAffinity{1UL << core_id};
task2->cpu_affinity = CpuAffinity{1UL << core_id};
task3->cpu_affinity = CpuAffinity{1UL << core_id};
task4->cpu_affinity = CpuAffinity{1UL << core_id};
```

**Step 3: Update `src/task/task_manager.cpp`**

```cpp
// OLD (around line 98-101):
if (task->cpu_affinity != UINT64_MAX) {
  for (size_t core_id = 0; core_id < SIMPLEKERNEL_MAX_CORE_COUNT; ++core_id) {
    if (task->cpu_affinity & (1UL << core_id)) {
      target_core = core_id;
      break;
    }
  }
}

// NEW:
// etl::flags<uint64_t> with all bits set == UINT64_MAX affinity (any CPU).
// Check: if not all bits set, find first allowed core.
if (task->cpu_affinity.value() != UINT64_MAX) {
  for (size_t core_id = 0; core_id < SIMPLEKERNEL_MAX_CORE_COUNT; ++core_id) {
    if (task->cpu_affinity.test(1UL << core_id)) {
      target_core = core_id;
      break;
    }
  }
}
```

**Step 4: Update `src/syscall.cpp`**

The `sched_getaffinity` / `sched_setaffinity` syscall handlers at lines ~250 and ~278.

At the syscall boundary, the ABI uses raw `uint64_t`:
```cpp
// OLD:
*mask = target->cpu_affinity;       // line ~250
target->cpu_affinity = *mask;       // line ~278

// NEW (convert at boundary):
*mask = target->cpu_affinity.value();           // read: get raw uint64_t
target->cpu_affinity = CpuAffinity{*mask};      // write: wrap in type
```

**Step 5: Write a unit test**

```cpp
// tests/unit_test/test_cpu_affinity.cpp
#include <gtest/gtest.h>
#include "task_control_block.hpp"

TEST(CpuAffinityTest, DefaultIsAllCpus) {
  CpuAffinity aff{UINT64_MAX};
  EXPECT_EQ(aff.value(), UINT64_MAX);
}

TEST(CpuAffinityTest, SingleCoreBinding) {
  CpuAffinity aff{1UL << 2};  // CPU 2
  EXPECT_TRUE(aff.test(1UL << 2));
  EXPECT_FALSE(aff.test(1UL << 0));
  EXPECT_FALSE(aff.test(1UL << 1));
}

TEST(CpuAffinityTest, TypeSafetyFromCloneFlags) {
  // CloneFlags and CpuAffinity are different types — cannot mix accidentally.
  CloneFlags cf{clone_flag::kVm};
  CpuAffinity ca{1UL << 0};
  // The following would fail to compile:
  // ca = cf;  // compile error
  static_assert(!std::is_same_v<CloneFlags, CpuAffinity>,
                "CloneFlags and CpuAffinity must be distinct types");
}
```

**Step 6: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 7: Commit**

```bash
git add src/task/include/task_control_block.hpp src/main.cpp \
        src/task/task_manager.cpp src/syscall.cpp \
        tests/unit_test/test_cpu_affinity.cpp
git commit --signoff -m "feat(task): replace cpu_affinity uint64_t with etl::flags<uint64_t>"
```

---

## Track 3: etl::fsm — Formalize TaskStatus State Machine

> **Scope note:** This track adds a `TaskFsm` field to `TaskControlBlock` and migrates all scattered `task->status = kXxx` assignments to `task->fsm.receive(MsgXxx{})`. The `TaskStatus` enum is **kept** as state IDs for the FSM (etl::fsm requires integer state IDs — we reuse the enum values).

### Task 3.1: Create `task_messages.hpp` — centralized message type registry

**Files:**
- Create: `src/task/include/task_messages.hpp`

**Step 1: Create the file**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ETL FSM message type IDs for TaskFsm.
 *        ALL message IDs must be assigned here — never use raw integers elsewhere.
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

#include <cstdint>

#include "resource_id.hpp"

// All TaskFsm message IDs — allocated sequentially, must not repeat.
namespace task_msg_id {
/// Scheduler picks this task to run (kReady → kRunning).
inline constexpr etl::message_id_t kSchedule = 1;
/// Task voluntarily yields CPU (kRunning → kReady).
inline constexpr etl::message_id_t kYield = 2;
/// Task requests timed sleep (kRunning → kSleeping).
inline constexpr etl::message_id_t kSleep = 3;
/// Task blocks on a resource (kRunning → kBlocked).
inline constexpr etl::message_id_t kBlock = 4;
/// Task is woken from blocked/sleeping state (kBlocked/kSleeping → kReady).
inline constexpr etl::message_id_t kWakeup = 5;
/// Task calls sys_exit (kRunning → kZombie or kExited).
inline constexpr etl::message_id_t kExit = 6;
/// Parent reaps zombie child (kZombie → kExited).
inline constexpr etl::message_id_t kReap = 7;
}  // namespace task_msg_id

// ---- Message definitions ----

/// Scheduler selects this task to run.
struct MsgSchedule : etl::message<task_msg_id::kSchedule> {};

/// Task voluntarily yields the CPU.
struct MsgYield : etl::message<task_msg_id::kYield> {};

/// Task requests timed sleep; carries the target wake tick.
struct MsgSleep : etl::message<task_msg_id::kSleep> {
  uint64_t wake_tick;
};

/// Task blocks on a resource.
struct MsgBlock : etl::message<task_msg_id::kBlock> {
  ResourceId resource;
};

/// Task wakeup (from blocked or sleeping state).
struct MsgWakeup : etl::message<task_msg_id::kWakeup> {};

/// Task exits; carries the exit code.
struct MsgExit : etl::message<task_msg_id::kExit> {
  int exit_code;
};

/// Parent reaps a zombie child.
struct MsgReap : etl::message<task_msg_id::kReap> {};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
```

**Step 2: Run unit tests (no functional change yet)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/task/include/task_messages.hpp
git commit --signoff -m "feat(task): add task_messages.hpp — centralized FSM message type IDs"
```

---

### Task 3.2: Create `task_fsm.hpp` — FSM state classes and host

**Files:**
- Create: `src/task/include/task_fsm.hpp`

**IMPORTANT architecture note:** `etl::fsm` uses `static` state objects shared across all FSM instances. The context (per-task data) is accessed via `get_fsm_context()` which returns the host's context reference. This means:
- State class methods access the *current task's* TCB via `get_fsm_context()`.
- Multiple tasks can be in the same state type concurrently — that is fine, the state object is stateless; all task data lives in the TCB (context).

**Step 1: Create `task_fsm.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ETL FSM host and state classes for TaskControlBlock status transitions.
 *
 * Usage:
 *   TaskControlBlock tcb;
 *   // tcb.fsm is default-constructed (kUnInit state).
 *   tcb.fsm.Init(tcb);   // bind context and start FSM
 *   tcb.fsm.receive(MsgSchedule{});   // → kRunning
 *   tcb.fsm.receive(MsgYield{});      // → kReady
 *
 * State transition table:
 *   kUnInit   --[MsgSchedule]--> kRunning   (first run from init)
 *   kReady    --[MsgSchedule]--> kRunning
 *   kRunning  --[MsgYield]------> kReady
 *   kRunning  --[MsgSleep]------> kSleeping
 *   kRunning  --[MsgBlock]------> kBlocked
 *   kRunning  --[MsgExit]-------> kZombie (has parent) or kExited
 *   kSleeping --[MsgWakeup]-----> kReady
 *   kBlocked  --[MsgWakeup]-----> kReady
 *   kZombie   --[MsgReap]-------> kExited
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_

#include <etl/fsm.h>

#include "task_control_block.hpp"
#include "task_messages.hpp"

// Forward declaration
class TaskFsm;

// ============================================================================
// State: kUnInit — task not yet initialized
// ============================================================================
class StateUnInit
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kUnInit,
                            MsgSchedule> {
 public:
  /// Initial scheduling: kUnInit → kRunning (first dispatch).
  auto on_event(const MsgSchedule&) -> etl::fsm_state_id_t {
    return TaskStatus::kRunning;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;  // ignore unknown messages in uninit
  }
};

// ============================================================================
// State: kReady — task is queued, waiting to run
// ============================================================================
class StateReady
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kReady,
                            MsgSchedule> {
 public:
  /// Scheduler picks task: kReady → kRunning.
  auto on_event(const MsgSchedule&) -> etl::fsm_state_id_t {
    return TaskStatus::kRunning;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;
  }
};

// ============================================================================
// State: kRunning — task is currently executing on a CPU
// ============================================================================
class StateRunning
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kRunning,
                            MsgYield, MsgSleep, MsgBlock, MsgExit> {
 public:
  /// Task yields: kRunning → kReady.
  auto on_event(const MsgYield&) -> etl::fsm_state_id_t {
    return TaskStatus::kReady;
  }
  /// Task sleeps: set wake_tick on TCB, kRunning → kSleeping.
  auto on_event(const MsgSleep& m) -> etl::fsm_state_id_t {
    get_fsm_context().sched_info.wake_tick = m.wake_tick;
    return TaskStatus::kSleeping;
  }
  /// Task blocks on resource: record resource_id, kRunning → kBlocked.
  auto on_event(const MsgBlock& m) -> etl::fsm_state_id_t {
    get_fsm_context().blocked_on = m.resource;
    return TaskStatus::kBlocked;
  }
  /// Task exits: set exit_code, transition based on whether parent exists.
  auto on_event(const MsgExit& m) -> etl::fsm_state_id_t {
    get_fsm_context().exit_code = m.exit_code;
    // Transition: zombie if has parent, exited if orphan.
    // The caller (Exit()) handles ReparentChildren and Wakeup of parent.
    if (get_fsm_context().parent_pid != 0) {
      return TaskStatus::kZombie;
    }
    return TaskStatus::kExited;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;
  }
};

// ============================================================================
// State: kSleeping — task is waiting for a timer tick
// ============================================================================
class StateSleeping
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kSleeping,
                            MsgWakeup> {
 public:
  /// Timer fires: kSleeping → kReady.
  auto on_event(const MsgWakeup&) -> etl::fsm_state_id_t {
    return TaskStatus::kReady;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;
  }
};

// ============================================================================
// State: kBlocked — task is waiting for a resource
// ============================================================================
class StateBlocked
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kBlocked,
                            MsgWakeup> {
 public:
  /// Resource available: kBlocked → kReady.
  auto on_event(const MsgWakeup&) -> etl::fsm_state_id_t {
    get_fsm_context().blocked_on = ResourceId{};
    return TaskStatus::kReady;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;
  }
};

// ============================================================================
// State: kZombie — task exited, waiting for parent to reap
// ============================================================================
class StateZombie
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kZombie,
                            MsgReap> {
 public:
  /// Parent reaps: kZombie → kExited.
  auto on_event(const MsgReap&) -> etl::fsm_state_id_t {
    return TaskStatus::kExited;
  }
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;
  }
};

// ============================================================================
// State: kExited — task fully terminated (resources freed)
// ============================================================================
class StateExited
    : public etl::fsm_state<TaskFsm, TaskControlBlock, TaskStatus::kExited> {
 public:
  auto on_event_unknown(const etl::imessage&) -> etl::fsm_state_id_t {
    return STATE_ID;  // terminal state, ignore all messages
  }
};

// ============================================================================
// FSM Host: TaskFsm
// ============================================================================
/**
 * @brief ETL FSM host for a single TaskControlBlock.
 *
 * @pre  Call Init(tcb) before any receive() call.
 * @post After Init(), the FSM is in kUnInit state.
 *       Transition to kRunning via receive(MsgSchedule{}).
 */
class TaskFsm : public etl::fsm {
 public:
  TaskFsm() : etl::fsm(etl::ifsm_state_id_t(TaskStatus::kUnInit)) {}

  /**
   * @brief Bind this FSM to a TaskControlBlock and start it.
   * @param tcb The task control block that owns this FSM.
   * @note  Must be called before any receive(). Safe to call multiple times
   *        (idempotent once started).
   */
  void Init(TaskControlBlock& tcb) {
    // State objects are static — shared across all TaskFsm instances.
    // Each TaskFsm binds its own TCB as context.
    static StateUnInit  s_uninit;
    static StateReady   s_ready;
    static StateRunning s_running;
    static StateSleeping s_sleeping;
    static StateBlocked s_blocked;
    static StateZombie  s_zombie;
    static StateExited  s_exited;

    static etl::ifsm_state* kStateList[] = {
      &s_uninit, &s_ready, &s_running,
      &s_sleeping, &s_blocked, &s_zombie, &s_exited,
    };

    set_states(kStateList, ETL_OR_STD17::size(kStateList));
    set_context(tcb);
    start();
  }

  /**
   * @brief Convenience: read current status from the FSM state ID.
   *        Equivalent to the old task->status field read.
   */
  [[nodiscard]] auto GetStatus() const -> TaskStatus {
    return static_cast<TaskStatus>(get_state_id());
  }
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
```

**Step 2: Run unit tests (compilation check only at this stage)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: Compilation succeeds, tests pass (no new test yet — just verifying the headers compile clean).

**Step 3: Commit**

```bash
git add src/task/include/task_messages.hpp src/task/include/task_fsm.hpp
git commit --signoff -m "feat(task): add task_fsm.hpp — ETL FSM for TaskStatus state transitions"
```

---

### Task 3.3: Integrate `TaskFsm` into `TaskControlBlock`

**Files:**
- Modify: `src/task/include/task_control_block.hpp`
- Modify: `src/task/task_control_block.cpp` (add `fsm.Init(*this)` in constructors)

**Step 1: Add `fsm` field to `task_control_block.hpp`**

Add `#include "task_fsm.hpp"` after the other task includes.

In `struct TaskControlBlock`, add the FSM field immediately after `TaskStatus status`:

```cpp
/// Thread state
TaskStatus status = TaskStatus::kUnInit;

/// ETL FSM — manages state transitions for this task.
/// @note Call fsm.Init(*this) after all TCB fields are initialized.
TaskFsm fsm{};
```

**Step 2: Initialize FSM in `task_control_block.cpp` constructors**

In each constructor that sets `status = kReady` (or similar), call `fsm.Init(*this)` at the end:

```cpp
// Kernel thread constructor (end):
fsm.Init(*this);

// User thread constructor (end):
fsm.Init(*this);
```

**Step 3: Write a unit test**

```cpp
// tests/unit_test/test_task_fsm.cpp
#include <gtest/gtest.h>
#include "task_fsm.hpp"
#include "task_control_block.hpp"
#include "task_messages.hpp"

TEST(TaskFsmTest, InitialStateIsUnInit) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kUnInit);
  EXPECT_EQ(tcb.status, TaskStatus::kUnInit);  // status field still synced
}

TEST(TaskFsmTest, ScheduleTransitionToRunning) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  tcb.fsm.receive(MsgSchedule{});
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kRunning);
}

TEST(TaskFsmTest, YieldTransitionToReady) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  // kUnInit → kRunning
  tcb.fsm.receive(MsgSchedule{});
  // kRunning → kReady
  tcb.fsm.receive(MsgYield{});
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kReady);
}

TEST(TaskFsmTest, SleepSetsWakeTick) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  tcb.fsm.receive(MsgSchedule{});
  tcb.fsm.receive(MsgSleep{.wake_tick = 12345});
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kSleeping);
  EXPECT_EQ(tcb.sched_info.wake_tick, 12345UL);
}

TEST(TaskFsmTest, BlockSetsResourceId) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  tcb.fsm.receive(MsgSchedule{});
  ResourceId rid{ResourceType::kMutex, 42};
  tcb.fsm.receive(MsgBlock{.resource = rid});
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kBlocked);
  EXPECT_EQ(tcb.blocked_on, rid);
}

TEST(TaskFsmTest, WakeupFromSleeping) {
  TaskControlBlock tcb;
  tcb.fsm.Init(tcb);
  tcb.fsm.receive(MsgSchedule{});
  tcb.fsm.receive(MsgSleep{.wake_tick = 100});
  tcb.fsm.receive(MsgWakeup{});
  EXPECT_EQ(tcb.fsm.GetStatus(), TaskStatus::kReady);
}
```

**Step 4: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass including new FSM tests.

**Step 5: Commit**

```bash
git add src/task/include/task_control_block.hpp src/task/task_control_block.cpp \
        tests/unit_test/test_task_fsm.cpp
git commit --signoff -m "feat(task): integrate TaskFsm into TaskControlBlock"
```

---

### Task 3.4: Migrate `status` assignments — schedule, tick_update, wakeup

**Files:**
- Modify: `src/task/schedule.cpp`
- Modify: `src/task/tick_update.cpp`
- Modify: `src/task/wakeup.cpp`

**Strategy:** Keep `task->status` field for now as a **read-only** consistency check. The FSM's `GetStatus()` is authoritative. After sending a message, sync `task->status` from the FSM:

```cpp
// After every fsm.receive(), sync the status field:
task->fsm.receive(MsgXxx{...});
task->status = task->fsm.GetStatus();
```

This allows existing code that reads `task->status` to continue working during the migration.

**Step 1: Update `schedule.cpp`**

```cpp
// OLD (line 43):
current->status = TaskStatus::kReady;

// NEW:
current->fsm.receive(MsgYield{});
current->status = current->fsm.GetStatus();  // sync

// OLD (line 92):
next->status = TaskStatus::kRunning;

// NEW:
next->fsm.receive(MsgSchedule{});
next->status = next->fsm.GetStatus();  // sync
```

**Step 2: Update `tick_update.cpp`**

```cpp
// OLD (line 33):
task->status = TaskStatus::kReady;

// NEW:
task->fsm.receive(MsgWakeup{});
task->status = task->fsm.GetStatus();  // sync
```

**Step 3: Update `wakeup.cpp`**

```cpp
// OLD (line 39):
task->status = TaskStatus::kReady;

// NEW:
task->fsm.receive(MsgWakeup{});
task->status = task->fsm.GetStatus();  // sync
```

**Step 4: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/task/schedule.cpp src/task/tick_update.cpp src/task/wakeup.cpp
git commit --signoff -m "feat(task): migrate schedule/tick_update/wakeup to fsm.receive()"
```

---

### Task 3.5: Migrate `status` assignments — exit, sleep, block

**Files:**
- Modify: `src/task/exit.cpp`
- Modify: `src/task/sleep.cpp`
- Modify: `src/task/block.cpp`

**Step 1: Update `exit.cpp`**

```cpp
// OLD (line 45):
current->status = TaskStatus::kZombie;
// OLD (line 59):
current->status = TaskStatus::kExited;

// NEW — send MsgExit, FSM decides kZombie vs kExited based on parent_pid:
current->fsm.receive(MsgExit{.exit_code = exit_code});
current->status = current->fsm.GetStatus();  // sync
// Remove the separate `current->exit_code = exit_code;` line —
// StateRunning::on_event(MsgExit) already sets it via get_fsm_context().
```

**Step 2: Update `sleep.cpp`**

```cpp
// OLD (line 36):
current->status = TaskStatus::kSleeping;
// (line 33): current->sched_info.wake_tick = cpu_sched.local_tick + sleep_ticks;

// NEW — pass wake_tick via message, StateRunning sets it on TCB:
current->fsm.receive(MsgSleep{.wake_tick = cpu_sched.local_tick + sleep_ticks});
current->status = current->fsm.GetStatus();  // sync
// Remove the separate wake_tick assignment line (now done inside StateRunning::on_event(MsgSleep)).
```

**Step 3: Update `block.cpp`**

```cpp
// OLD (line 22):
current->status = TaskStatus::kBlocked;
// OLD (line 25): current->blocked_on = resource_id;

// NEW — pass resource via message, StateRunning sets blocked_on on TCB:
current->fsm.receive(MsgBlock{.resource = resource_id});
current->status = current->fsm.GetStatus();  // sync
// Remove the separate blocked_on assignment (done inside StateRunning::on_event(MsgBlock)).
```

**Step 4: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/task/exit.cpp src/task/sleep.cpp src/task/block.cpp
git commit --signoff -m "feat(task): migrate exit/sleep/block to fsm.receive()"
```

---

### Task 3.6: Remove `task->status` sync boilerplate (optional cleanup)

Once all direct assignments are gone, the `task->status` field can either be:

**Option A (recommended):** Keep `task->status` as a convenience field and have `TaskFsm::on_enter_state()` in each state class automatically update it:

```cpp
// In each state class, add on_enter_state():
void on_enter_state() override {
  get_fsm_context().status = static_cast<TaskStatus>(STATE_ID);
}
```

This eliminates all the `task->status = task->fsm.GetStatus()` sync lines.

**Option B:** Remove `task->status` entirely and replace all reads with `task->fsm.GetStatus()`. More invasive but cleaner.

**Recommendation:** Implement Option A.

**Step 1: Add `on_enter_state()` to each state class in `task_fsm.hpp`**

In each of `StateReady`, `StateRunning`, `StateSleeping`, `StateBlocked`, `StateZombie`, `StateExited`, `StateUnInit`:
```cpp
void on_enter_state() override {
  get_fsm_context().status = static_cast<TaskStatus>(STATE_ID);
}
```

**Step 2: Remove all explicit sync lines** from `schedule.cpp`, `tick_update.cpp`, `wakeup.cpp`, `exit.cpp`, `sleep.cpp`, `block.cpp`:
```cpp
// DELETE these lines:
// task->status = task->fsm.GetStatus();
```

**Step 3: Run unit tests**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

Expected: All tests pass and `task->status` is now always auto-synced by FSM entry hooks.

**Step 4: Commit**

```bash
git add src/task/include/task_fsm.hpp \
        src/task/schedule.cpp src/task/tick_update.cpp src/task/wakeup.cpp \
        src/task/exit.cpp src/task/sleep.cpp src/task/block.cpp
git commit --signoff -m "refactor(task): auto-sync status via FSM on_enter_state(), remove manual sync"
```

---

## Final Verification

After all tasks are complete:

```bash
# Build all architectures
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel
cmake --preset build_x86_64  && cd build_x86_64  && make SimpleKernel

# Run all tests
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test

# Check format
pre-commit run --all-files
```

---

## Implementation Notes & Gotchas

### etl::delegate lambda lifetime
`etl::delegate::create(callable)` accepts an **lvalue reference** only. Temporary lambdas (rvalues) are deleted at the call site. Pattern: declare lambda as `static auto kHandler = [...]` before creating the delegate.

### etl::fsm static state objects
State objects are `static` inside `TaskFsm::Init()` — they are shared across ALL TaskFsm instances. This is intentional: the state objects are stateless; all per-task data lives in the `TaskControlBlock` context, accessed via `get_fsm_context()`.

### CloneFlags: raw uint64_t at syscall boundary
The `Clone(uint64_t flags, ...)` parameter remains `uint64_t`. Bitwise operations on this raw parameter throughout `clone.cpp` are fine. Only `child->clone_flags = CloneFlags{flags}` is the conversion point.

### CpuAffinity: value() for legacy comparisons
`etl::flags<uint64_t>` wraps the value. Use `.value()` to extract the raw `uint64_t` when comparing against `UINT64_MAX` or passing to `syscall` ABI.

### TaskStatus enum kept as FSM state IDs
`etl::fsm_state<..., STATE_ID>` requires an integer state ID. We reuse `TaskStatus` enum values directly (e.g. `TaskStatus::kReady` = 1, `TaskStatus::kRunning` = 2, etc.) — no renaming needed.
