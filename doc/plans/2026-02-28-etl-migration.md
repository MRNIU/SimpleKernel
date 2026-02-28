# ETL Integration Migration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate SimpleKernel's task and interrupt subsystems to ETL type-safe abstractions (etl::fsm, etl::delegate, etl::flags, etl::observer, etl::message_router), replacing raw enums, function pointers, and scattered state assignments.

**Architecture:** Six migration steps in dependency order. Steps 1-3 are independent (parallelizable). Step 4 depends on Step 1. Steps 5-6 depend on Step 4. Each step must compile on all three architectures before proceeding.

**Tech Stack:** C++23 freestanding, ETL (Embedded Template Library) — `etl::fsm`, `etl::delegate`, `etl::flags`, `etl::observer`, `etl::message_router`. Build: CMake + cross-compilation for x86_64, riscv64, aarch64.

**Design Doc:** `doc/plans/2026-02-28-etl-migration-design.md`

**ETL Reference Docs:**
- `doc/etl/etl_delegate_fsm.md` — delegate, FSM, flags migration patterns
- `doc/etl/etl_advanced.md` — FSM lifecycle, observer, message_router patterns

---

## Dependency Graph

```
Task 1: task_messages.hpp ─┐
Task 2: etl::flags          ├─ Parallel (independent)
Task 3: etl::delegate      ─┘
Task 4: etl::fsm            ← Depends on Task 1
Task 5: etl::observer       ← Depends on Task 4
Task 6: etl::message_router ← Depends on Tasks 1, 4
```

## Verification Commands

After every task, run:

```bash
# Build all three architectures
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel

# Style check
pre-commit run --all-files
```

---

## Task 1: Message & Router ID Registry

**Files:**
- Create: `src/task/include/task_messages.hpp`

**Step 1: Create the message ID header**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM message IDs and router IDs — single source of truth
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

/// @brief Task FSM message IDs
namespace task_msg_id {
inline constexpr etl::message_id_t kSchedule = 1;
inline constexpr etl::message_id_t kYield = 2;
inline constexpr etl::message_id_t kSleep = 3;
inline constexpr etl::message_id_t kBlock = 4;
inline constexpr etl::message_id_t kWakeup = 5;
inline constexpr etl::message_id_t kExit = 6;
inline constexpr etl::message_id_t kReap = 7;
}  // namespace task_msg_id

/// @brief Message router IDs
namespace router_id {
constexpr etl::message_router_id_t kTimerHandler = 0;
constexpr etl::message_router_id_t kTaskFsm = 1;
constexpr etl::message_router_id_t kVirtioBlk = 2;
constexpr etl::message_router_id_t kVirtioNet = 3;
}  // namespace router_id

/// @brief Task FSM message structs (empty payload, used as events)
struct MsgSchedule : public etl::message<task_msg_id::kSchedule> {};
struct MsgYield : public etl::message<task_msg_id::kYield> {};
struct MsgWakeup : public etl::message<task_msg_id::kWakeup> {};
struct MsgReap : public etl::message<task_msg_id::kReap> {};

/// @brief Sleep message with wake tick payload
struct MsgSleep : public etl::message<task_msg_id::kSleep> {
  uint64_t wake_tick;
  explicit MsgSleep(uint64_t tick) : wake_tick(tick) {}
};

/// @brief Block message with resource payload
/// @note Forward-declared ResourceId to avoid circular include
struct MsgBlock : public etl::message<task_msg_id::kBlock> {
  uint64_t resource_data;
  explicit MsgBlock(uint64_t data) : resource_data(data) {}
};

/// @brief Exit message with exit code and parent flag
struct MsgExit : public etl::message<task_msg_id::kExit> {
  int exit_code;
  bool has_parent;
  MsgExit(int code, bool parent) : exit_code(code), has_parent(parent) {}
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
```

**Step 2: Verify compilation**

Run: `cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel`

Expected: SUCCESS (header is not yet included anywhere, but must parse cleanly if included).

To verify the header parses correctly, temporarily add `#include "task_messages.hpp"` to `src/task/include/task_control_block.hpp`, build, then remove it.

**Step 3: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS

**Step 4: Commit**

```bash
git add src/task/include/task_messages.hpp
git commit --signoff -m "feat(task): add ETL message and router ID registry"
```

---

## Task 2: `etl::flags` for CloneFlags & CpuAffinity

**Files:**
- Modify: `src/task/include/task_control_block.hpp` — replace `enum CloneFlags` and `cpu_affinity` type
- Modify: `src/task/clone.cpp` — update flag operations
- Modify: `src/task/task_manager.cpp:103` — update `cpu_affinity` check

**Step 1: Modify `task_control_block.hpp`**

Replace the `enum CloneFlags` block (lines 23-42) with:

```cpp
#include <etl/flags.h>

/**
 * @brief Clone 标志位常量 (用于 sys_clone 系统调用)
 * @note 参考 Linux clone flags
 */
namespace clone_flag {
// 共享地址空间
inline constexpr uint64_t kVm = 0x00000100;
// 共享文件系统信息
inline constexpr uint64_t kFs = 0x00000200;
// 共享文件描述符表
inline constexpr uint64_t kFiles = 0x00000400;
// 共享信号处理器
inline constexpr uint64_t kSighand = 0x00000800;
// 保持相同父进程
inline constexpr uint64_t kParent = 0x00008000;
// 同一线程组
inline constexpr uint64_t kThread = 0x00010000;
// 全部标志掩码
inline constexpr uint64_t kAllMask =
    kVm | kFs | kFiles | kSighand | kParent | kThread;
}  // namespace clone_flag

/// @brief 类型安全的克隆标志位
using CloneFlags = etl::flags<uint64_t, clone_flag::kAllMask>;

/// @brief 类型安全的 CPU 亲和性位掩码
using CpuAffinity = etl::flags<uint64_t>;
```

In `struct TaskControlBlock`, change the member declarations:

Line 128 (`CloneFlags clone_flags = kCloneAll;`) → `CloneFlags clone_flags{};`

Line 182 (`uint64_t cpu_affinity = UINT64_MAX;`) → `CpuAffinity cpu_affinity{UINT64_MAX};`

**Step 2: Modify `clone.cpp`**

The `Clone` method takes `uint64_t flags`. We keep the parameter as `uint64_t` (it comes from syscall) but convert at assignment. Here are all the changes:

Line 24 (`if ((flags & kCloneThread) &&`):
```cpp
  if ((flags & clone_flag::kThread) &&
      (!(flags & clone_flag::kVm) || !(flags & clone_flag::kFiles) ||
       !(flags & clone_flag::kSighand))) {
```

Line 30 (`flags |= (kCloneVm | kCloneFiles | kCloneSighand);`):
```cpp
    flags |= (clone_flag::kVm | clone_flag::kFiles | clone_flag::kSighand);
```

Line 54 (`if (flags & kCloneParent)`):
```cpp
  if (flags & clone_flag::kParent) {
```

Line 62 (`if (flags & kCloneThread)`):
```cpp
  if (flags & clone_flag::kThread) {
```

Line 91 (`child->clone_flags = static_cast<CloneFlags>(flags);`):
```cpp
  child->clone_flags = CloneFlags(flags);
```

Line 95 (`if (flags & kCloneFiles)`):
```cpp
  if (flags & clone_flag::kFiles) {
```

Line 103 (`if (flags & kCloneSighand)`):
```cpp
  if (flags & clone_flag::kSighand) {
```

Line 111 (`if (flags & kCloneFs)`):
```cpp
  if (flags & clone_flag::kFs) {
```

Line 118 (`if (flags & kCloneVm)`):
```cpp
  if (flags & clone_flag::kVm) {
```

Line 150 (`if (child->page_table && !(flags & kCloneVm))`):
```cpp
    if (child->page_table && !(flags & clone_flag::kVm)) {
```

Line 196 (`const char* clone_type = (flags & kCloneThread) ? "thread" : "process";`):
```cpp
  const char* clone_type = (flags & clone_flag::kThread) ? "thread" : "process";
```

Line 197 (`const char* vm_type = (flags & kCloneVm) ? "shared" : "copied";`):
```cpp
  const char* vm_type = (flags & clone_flag::kVm) ? "shared" : "copied";
```

**Step 3: Modify `task_manager.cpp`**

Line 103 (`if (task->cpu_affinity != UINT64_MAX)`):
```cpp
  if (task->cpu_affinity.value() != UINT64_MAX) {
```

Line 106 (`if (task->cpu_affinity & (1UL << core_id))`):
```cpp
      if (task->cpu_affinity.value() & (1UL << core_id)) {
```

**Step 4: Verify compilation (all three architectures)**

Run:
```bash
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel
```
Expected: All three compile successfully.

**Step 5: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS (may need minor formatting fixes from clang-format)

**Step 6: Commit**

```bash
git add src/task/include/task_control_block.hpp src/task/clone.cpp src/task/task_manager.cpp
git commit --signoff -m "refactor(task): replace CloneFlags enum and cpu_affinity with etl::flags"
```

---

## Task 3: `etl::delegate` for InterruptFunc

**Files:**
- Modify: `src/include/interrupt_base.h` — replace typedef with delegate alias, update virtual signatures
- Modify: `src/arch/riscv64/include/interrupt.h` — update override signatures + array type
- Modify: `src/arch/aarch64/include/interrupt.h` — update override signatures + array type
- Modify: `src/arch/x86_64/include/interrupt.h` — update override signatures + array type
- Modify: `src/arch/riscv64/interrupt.cpp` — update lambda → delegate creation
- Modify: `src/arch/aarch64/interrupt.cpp` — same
- Modify: `src/arch/x86_64/interrupt.cpp` — same
- Modify: `src/arch/riscv64/plic/include/plic.h` — update PLIC's own InterruptFunc typedef
- Modify: `src/arch/riscv64/plic/plic.cpp` — update PLIC handler array type

### Important: PLIC has its own `InterruptFunc`

The PLIC class at `src/arch/riscv64/plic/include/plic.h:26` defines its own `typedef uint64_t (*InterruptFunc)(uint64_t, cpu_io::TrapContext*)`. It must be updated to match. Since PLIC stores handlers in a `std::array<InterruptFunc, ...>`, this must also become `InterruptDelegate`.

**Step 1: Modify `src/include/interrupt_base.h`**

Replace the typedef (lines 22-23) with:

```cpp
  #include <etl/delegate.h>
```

(add this include at line 12, after `#include "expected.hpp"`)

Replace:
```cpp
  typedef uint64_t (*InterruptFunc)(uint64_t cause,
                                    cpu_io::TrapContext* context);
```

With:
```cpp
  /// @brief 类型安全的中断处理委托
  using InterruptDelegate =
      etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

  /// @brief 向后兼容别名 (deprecated, use InterruptDelegate)
  using InterruptFunc = InterruptDelegate;
```

Update virtual method signatures — they already use `InterruptFunc` so no changes needed since we aliased `InterruptFunc = InterruptDelegate`.

**Step 2: Update arch `interrupt.h` headers**

For each arch's `interrupt.h`:
- The `std::array<InterruptFunc, ...>` member types remain valid because `InterruptFunc` is now an alias for `InterruptDelegate`.
- The override signatures using `InterruptFunc` remain valid.
- No changes needed in the headers if we keep the alias.

**Step 3: Update arch `interrupt.cpp` lambda assignments**

In each constructor, the default handlers are assigned as lambdas:
```cpp
i = [](uint64_t cause, cpu_io::TrapContext* context) -> uint64_t { ... };
```

`etl::delegate` cannot be directly assigned a lambda. We need to use `etl::delegate::create()` with a free function, OR we can use a wrapper approach.

**Strategy**: Create static free functions for default handlers, then use `InterruptDelegate::create<function>()`.

**For `src/arch/riscv64/interrupt.cpp`** (lines 21-40):

Replace the lambda assignments in the constructor with:

```cpp
namespace {
auto DefaultInterruptHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::Info("Default Interrupt handler [%s] 0x%X, 0x%p\n",
             cpu_io::detail::register_info::csr::ScauseInfo::kInterruptNames
                 [cause],
             cause, context);
  return 0;
}

auto DefaultExceptionHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::Err("Default Exception handler [%s] 0x%X, 0x%p\n",
            cpu_io::detail::register_info::csr::ScauseInfo::kExceptionNames
                [cause],
            cause, context);
  while (true) {
    cpu_io::Pause();
  }
}
}  // namespace

Interrupt::Interrupt() {
  for (auto& i : interrupt_handlers_) {
    i = InterruptDelegate::create<DefaultInterruptHandler>();
  }
  for (auto& i : exception_handlers_) {
    i = InterruptDelegate::create<DefaultExceptionHandler>();
  }
  klog::Info("Interrupt init.\n");
}
```

The `Do()`, `RegisterInterruptFunc()`, and `RegisterExternalInterrupt()` methods use `InterruptFunc` in their parameter types — these remain valid via the alias.

The handler invocation (`interrupt_handlers_[exception_code](exception_code, context)`) works because `etl::delegate` has `operator()`.

**For `src/arch/aarch64/interrupt.cpp`** (lines 21-26):

Replace the lambda with:

```cpp
namespace {
auto DefaultInterruptHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::Info("Default Interrupt handler 0x%X, 0x%p\n", cause, context);
  return 0;
}
}  // namespace

Interrupt::Interrupt() {
  // ... existing GIC setup code ...

  for (auto& i : interrupt_handlers_) {
    i = InterruptDelegate::create<DefaultInterruptHandler>();
  }

  // ... rest of constructor ...
}
```

**For `src/arch/x86_64/interrupt.cpp`** (lines 33-44):

Replace the lambda with:

```cpp
namespace {
auto DefaultInterruptHandler(uint64_t cause, cpu_io::TrapContext* context)
    -> uint64_t {
  klog::Info(
      "Default Interrupt handler [%s] 0x%X, 0x%p\n",
      cpu_io::detail::register_info::IdtrInfo::kInterruptNames[cause],
      cause, context);
  DumpStack();
  while (true) {
    ;
  }
}
}  // namespace

Interrupt::Interrupt() {
  for (auto& i : interrupt_handlers_) {
    i = InterruptDelegate::create<DefaultInterruptHandler>();
  }
  klog::Info("Interrupt init.\n");
}
```

**Step 4: Update PLIC**

In `src/arch/riscv64/plic/include/plic.h`:
- Remove the local `typedef uint64_t (*InterruptFunc)(...);` (line 26-27)
- Add `#include "interrupt_base.h"` and use `InterruptBase::InterruptDelegate` (or just include the base header for the type)
- OR simpler: since PLIC is only used by riscv64 Interrupt which already includes `interrupt_base.h`, we can use the type from InterruptBase.

Replace line 26-27:
```cpp
  // Remove typedef, use InterruptBase type
  using InterruptFunc = InterruptBase::InterruptDelegate;
```

And add `#include "interrupt_base.h"` to plic.h (or forward declare — but including is simpler since interrupt_base.h is lightweight).

In `src/arch/riscv64/plic/plic.cpp`, the static array initialization uses a lambda:
```cpp
alignas(4) std::array<Plic::InterruptFunc, ...> Plic::interrupt_handlers_;
```

This static array of delegates needs default-initialization. `etl::delegate` default constructs to an unset state. The PLIC's `RegisterInterruptFunc` assigns to it. The `Do()` method calls it. We need to verify `etl::delegate` doesn't crash when called unset — if it does, we need to set defaults.

Check if PLIC sets defaults in its constructor. If not, we need to initialize the static array elements. Look at `plic.cpp`:

The array is `static` and will be zero-initialized. Calling an unset `etl::delegate` is undefined. So we need to initialize it in the Plic constructor:

```cpp
// In Plic constructor
for (auto& h : interrupt_handlers_) {
  h = InterruptFunc::create<DefaultPlicHandler>();
}
```

Or add a default static handler for PLIC.

**Step 5: Verify compilation (all three architectures)**

Run:
```bash
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel
```
Expected: All three compile successfully.

**Step 6: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS

**Step 7: Commit**

```bash
git add src/include/interrupt_base.h \
        src/arch/riscv64/include/interrupt.h \
        src/arch/aarch64/include/interrupt.h \
        src/arch/x86_64/include/interrupt.h \
        src/arch/riscv64/interrupt.cpp \
        src/arch/aarch64/interrupt.cpp \
        src/arch/x86_64/interrupt.cpp \
        src/arch/riscv64/plic/include/plic.h \
        src/arch/riscv64/plic/plic.cpp
git commit --signoff -m "refactor(interrupt): replace InterruptFunc pointer with etl::delegate"
```

---

## Task 4: `etl::fsm` for TaskStatus State Machine

This is the largest task. Split into 4 sub-tasks.

### Task 4a: Create `task_fsm.hpp` — State Classes and TaskFsm

**Files:**
- Create: `src/task/include/task_fsm.hpp`

**Step 1: Create the FSM header**

This file defines 7 state classes and the TaskFsm class. Each state inherits `etl::fsm_state<>` and overrides `on_enter_state()` where appropriate.

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM — etl::fsm-based state machine for task lifecycle
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_

#include <etl/fsm.h>

#include "task_messages.hpp"

// Forward declaration
struct TaskControlBlock;

/// @brief Task status IDs — used as etl::fsm state IDs
/// @note These MUST match the old TaskStatus enum values for compatibility
enum TaskStatusId : uint8_t {
  kUnInit = 0,
  kReady = 1,
  kRunning = 2,
  kSleeping = 3,
  kBlocked = 4,
  kExited = 5,
  kZombie = 6,
};

// Forward-declare all states so they can reference each other in transition maps
class StateUnInit;
class StateReady;
class StateRunning;
class StateSleeping;
class StateBlocked;
class StateExited;
class StateZombie;

// ─── State: UnInit ───────────────────────────────────────────────────
class StateUnInit : public etl::fsm_state<StateUnInit, TaskStatusId::kUnInit,
                                          MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Ready ────────────────────────────────────────────────────
class StateReady
    : public etl::fsm_state<StateReady, TaskStatusId::kReady, MsgSchedule> {
 public:
  auto on_event(const MsgSchedule& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Running ──────────────────────────────────────────────────
class StateRunning
    : public etl::fsm_state<StateRunning, TaskStatusId::kRunning, MsgYield,
                            MsgSleep, MsgBlock, MsgExit> {
 public:
  auto on_event(const MsgYield& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgSleep& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgBlock& msg) -> etl::fsm_state_id_t;
  auto on_event(const MsgExit& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Sleeping ─────────────────────────────────────────────────
class StateSleeping : public etl::fsm_state<StateSleeping,
                                            TaskStatusId::kSleeping,
                                            MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Blocked ──────────────────────────────────────────────────
class StateBlocked
    : public etl::fsm_state<StateBlocked, TaskStatusId::kBlocked, MsgWakeup> {
 public:
  auto on_event(const MsgWakeup& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Exited ───────────────────────────────────────────────────
class StateExited
    : public etl::fsm_state<StateExited, TaskStatusId::kExited, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── State: Zombie ───────────────────────────────────────────────────
class StateZombie
    : public etl::fsm_state<StateZombie, TaskStatusId::kZombie, MsgReap> {
 public:
  auto on_event(const MsgReap& msg) -> etl::fsm_state_id_t;
  auto on_event_unknown(const etl::imessage& msg) -> etl::fsm_state_id_t;
};

// ─── TaskFsm ─────────────────────────────────────────────────────────

/// @brief Per-task FSM instance. Each TaskControlBlock owns one.
class TaskFsm {
 public:
  TaskFsm();

  /// @brief Start the FSM (call after TCB is fully constructed)
  void Start();

  /// @brief Send a message to the FSM
  void Receive(const etl::imessage& msg);

  /// @brief Get the current state ID
  auto GetStateId() const -> etl::fsm_state_id_t;

 private:
  StateUnInit state_uninit_;
  StateReady state_ready_;
  StateRunning state_running_;
  StateSleeping state_sleeping_;
  StateBlocked state_blocked_;
  StateExited state_exited_;
  StateZombie state_zombie_;

  etl::fsm fsm_;
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_FSM_HPP_
```

**Step 2: Verify header parses cleanly**

Temporarily include it from `task_control_block.hpp`, build, then remove.

**Step 3: Commit**

```bash
git add src/task/include/task_fsm.hpp
git commit --signoff -m "feat(task): add TaskFsm header with ETL FSM state classes"
```

### Task 4b: Implement `TaskFsm` — FSM state transitions

**Files:**
- Create: `src/task/task_fsm.cpp`
- Modify: `src/task/CMakeLists.txt` — add `task_fsm.cpp` to INTERFACE sources

**Step 1: Implement state transitions**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Task FSM implementation
 */

#include "task_fsm.hpp"

#include "kernel_log.hpp"

// ─── StateUnInit ────────────────────────────────────────────────────
auto StateUnInit::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateUnInit::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in UnInit state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateReady ─────────────────────────────────────────────────────
auto StateReady::on_event(const MsgSchedule& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kRunning;
}

auto StateReady::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Ready state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateRunning ───────────────────────────────────────────────────
auto StateRunning::on_event(const MsgYield& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateRunning::on_event(const MsgSleep& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kSleeping;
}

auto StateRunning::on_event(const MsgBlock& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kBlocked;
}

auto StateRunning::on_event(const MsgExit& msg) -> etl::fsm_state_id_t {
  if (msg.has_parent) {
    return TaskStatusId::kZombie;
  }
  return TaskStatusId::kExited;
}

auto StateRunning::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Running state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateSleeping ──────────────────────────────────────────────────
auto StateSleeping::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateSleeping::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Sleeping state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateBlocked ───────────────────────────────────────────────────
auto StateBlocked::on_event(const MsgWakeup& /*msg*/) -> etl::fsm_state_id_t {
  return TaskStatusId::kReady;
}

auto StateBlocked::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Blocked state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateExited ────────────────────────────────────────────────────
auto StateExited::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  // Terminal state — stay here (or could transition to a "Reaped" state)
  return STATE_ID;
}

auto StateExited::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Exited state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── StateZombie ────────────────────────────────────────────────────
auto StateZombie::on_event(const MsgReap& /*msg*/) -> etl::fsm_state_id_t {
  // Terminal state — stay here
  return STATE_ID;
}

auto StateZombie::on_event_unknown(const etl::imessage& msg)
    -> etl::fsm_state_id_t {
  klog::Warn("TaskFsm: unexpected event %d in Zombie state\n",
             msg.get_message_id());
  return STATE_ID;
}

// ─── TaskFsm ────────────────────────────────────────────────────────
TaskFsm::TaskFsm()
    : fsm_(router_id::kTaskFsm) {
  // Set the state list for the FSM
  etl::ifsm_state* state_list[] = {
      &state_uninit_, &state_ready_,    &state_running_, &state_sleeping_,
      &state_blocked_, &state_exited_, &state_zombie_,
  };
  fsm_.set_states(state_list, TaskStatusId::kUnInit);
}

void TaskFsm::Start() {
  fsm_.start();
}

void TaskFsm::Receive(const etl::imessage& msg) {
  fsm_.receive(msg);
}

auto TaskFsm::GetStateId() const -> etl::fsm_state_id_t {
  return fsm_.get_state_id();
}
```

**Step 2: Add to CMakeLists.txt**

Add `task_fsm.cpp` to the INTERFACE sources in `src/task/CMakeLists.txt`:

```cmake
TARGET_SOURCES (
    task
    INTERFACE task_control_block.cpp
              task_fsm.cpp
              schedule.cpp
              ...
```

**Step 3: Verify compilation**

Run: `cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel`
Expected: May need to adjust ETL FSM API usage based on actual ETL version. Common issues:
- `etl::fsm` constructor signature
- `set_states()` vs constructor-based state registration
- `etl::fsm_state` template parameter ordering

Consult `3rd/etl/include/etl/fsm.h` for the exact API if build fails.

**Step 4: Commit**

```bash
git add src/task/task_fsm.cpp src/task/CMakeLists.txt
git commit --signoff -m "feat(task): implement TaskFsm state transition logic"
```

### Task 4c: Embed FSM in TCB and add `GetStatus()` helper

**Files:**
- Modify: `src/task/include/task_control_block.hpp` — add `TaskFsm` member, `GetStatus()`, keep `TaskStatus` for backward compat
- Modify: `src/task/task_control_block.cpp` — initialize FSM in constructors

**Step 1: Modify `task_control_block.hpp`**

Add include:
```cpp
#include "task_fsm.hpp"
```

Replace the `TaskStatus` enum (lines 47-62) with:

```cpp
/// @brief 任务状态枚举 (retained as state IDs for FSM and backward compatibility)
using TaskStatus = TaskStatusId;
```

In `struct TaskControlBlock`, replace:

Line 120 (`TaskStatus status = TaskStatus::kUnInit;`):
```cpp
  /// 任务 FSM (manages state transitions)
  TaskFsm fsm;

  /// @brief 获取当前任务状态
  auto GetStatus() const -> TaskStatus {
    return static_cast<TaskStatus>(fsm.GetStateId());
  }
```

Remove the `status` field entirely — it's replaced by `fsm`.

**Step 2: Modify `task_control_block.cpp`**

In both constructors, after all fields are initialized, call `fsm.Start()`:

```cpp
TaskControlBlock::TaskControlBlock(const char* name, int priority,
                                   ThreadEntry entry, void* arg)
    : name(name) {
  // ... existing initialization ...
  fsm.Start();
}

TaskControlBlock::TaskControlBlock(const char* name, int priority,
                                   uint8_t* elf, int argc, char** argv)
    : name(name) {
  // ... existing initialization ...
  fsm.Start();
}
```

**Step 3: This will cause compilation failures across all .cpp files that reference `task->status`**

This is expected — the next sub-task (4d) fixes them all.

**Step 4: Commit (WIP — may not compile yet)**

```bash
git add src/task/include/task_control_block.hpp src/task/include/task_fsm.hpp src/task/task_control_block.cpp
git commit --signoff -m "refactor(task): embed TaskFsm in TCB, add GetStatus() helper [WIP]"
```

### Task 4d: Replace all `task->status` assignments with FSM messages

**Files:**
- Modify: `src/task/task_manager.cpp` — AddTask, InitCurrentCore, ReapTask
- Modify: `src/task/schedule.cpp` — Schedule
- Modify: `src/task/sleep.cpp` — Sleep
- Modify: `src/task/tick_update.cpp` — TickUpdate
- Modify: `src/task/block.cpp` — Block
- Modify: `src/task/wakeup.cpp` — Wakeup
- Modify: `src/task/exit.cpp` — Exit
- Modify: `src/task/wait.cpp` — Wait

**Replacement Map (every change in every file):**

#### `task_manager.cpp`

Line 59 (`idle_task->status = TaskStatus::kRunning;`):
```cpp
  // Transition idle task: kUnInit → kReady → kRunning
  idle_task->fsm.Receive(MsgSchedule{});  // UnInit → Ready
  idle_task->fsm.Receive(MsgSchedule{});  // Ready → Running
```
Add include: `#include "task_messages.hpp"`

Line 73-74 (assert `task->status == TaskStatus::kUnInit`):
```cpp
  assert(task->GetStatus() == TaskStatus::kUnInit &&
         "AddTask: task status must be kUnInit");
```

Line 97 (`task->status = TaskStatus::kReady;`):
```cpp
  task->fsm.Receive(MsgSchedule{});  // UnInit → Ready
```

Line 157-158 (ReapTask status checks):
```cpp
  if (task->GetStatus() != TaskStatus::kZombie &&
      task->GetStatus() != TaskStatus::kExited) {
```

#### `schedule.cpp`

Line 39 (`if (current->status == TaskStatus::kRunning)`):
```cpp
  if (current->GetStatus() == TaskStatus::kRunning) {
```

Line 41 (`current->status = TaskStatus::kReady;`):
```cpp
    current->fsm.Receive(MsgYield{});
```
Add include: `#include "task_messages.hpp"`

Line 68 (`if (current->status == TaskStatus::kReady)`):
```cpp
    if (current->GetStatus() == TaskStatus::kReady) {
```

Line 86-88 (assert `next->status == TaskStatus::kReady`):
```cpp
  assert((next->GetStatus() == TaskStatus::kReady ||
          next->policy == SchedPolicy::kIdle) &&
         "Schedule: next task must be kReady or kIdle policy");
```

Line 90 (`next->status = TaskStatus::kRunning;`):
```cpp
  next->fsm.Receive(MsgSchedule{});  // Ready → Running
```

#### `sleep.cpp`

Line 20-21 (assert `current->status == TaskStatus::kRunning`):
```cpp
  assert(current->GetStatus() == TaskStatus::kRunning &&
         "Sleep: current task status must be kRunning");
```

Line 37 (`current->status = TaskStatus::kSleeping;`):
```cpp
    current->fsm.Receive(MsgSleep{current->sched_info.wake_tick});
```
Add include: `#include "task_messages.hpp"`

Line 43 (`current->status = TaskStatus::kRunning;` — rollback):

**IMPORTANT**: The rollback pattern needs special handling. The FSM is now in `kSleeping` state. We cannot simply set it back to `kRunning`. Options:
1. Check capacity BEFORE sending MsgSleep
2. Add a MsgWakeup to transition back to kReady, then MsgSchedule to kRunning

Option 1 is cleaner:

```cpp
    // Check capacity BEFORE transitioning to sleep
    if (cpu_sched.sleeping_tasks.full()) {
      klog::Err("Sleep: sleeping_tasks full, cannot sleep task %zu\n",
                current->pid);
      // Don't transition FSM — task stays in Running state
      return;
    }

    // Compute wake tick
    current->sched_info.wake_tick = cpu_sched.local_tick + sleep_ticks;

    // Transition to sleeping
    current->fsm.Receive(MsgSleep{current->sched_info.wake_tick});

    // Add to sleeping queue
    cpu_sched.sleeping_tasks.push(current);
```

#### `block.cpp`

Line 16-17 (assert):
```cpp
  assert(current->GetStatus() == TaskStatus::kRunning &&
         "Block: current task status must be kRunning");
```

Line 23 (`current->status = TaskStatus::kBlocked;`):

Same rollback pattern issue. Check capacity first:

```cpp
    // Check capacity BEFORE transitioning
    auto& list = cpu_sched.blocked_tasks[resource_id];
    if (list.full()) {
      klog::Err("Block: blocked_tasks list full, cannot block task %zu\n",
                current->pid);
      return;
    }

    // Transition to blocked
    current->fsm.Receive(MsgBlock{resource_id.GetData()});
    current->blocked_on = resource_id;
    list.push_back(current);
```
Add include: `#include "task_messages.hpp"`

Lines 35-36 (rollback `current->status = TaskStatus::kRunning;`): Remove — covered by checking capacity first.

#### `wakeup.cpp`

Line 34-35 (assert):
```cpp
    assert(task->GetStatus() == TaskStatus::kBlocked &&
           "Wakeup: task status must be kBlocked");
```

Line 40 (`task->status = TaskStatus::kReady;`):
```cpp
    task->fsm.Receive(MsgWakeup{});
```
Add include: `#include "task_messages.hpp"`

#### `tick_update.cpp`

Line 33 (`task->status = TaskStatus::kReady;`):
```cpp
      task->fsm.Receive(MsgWakeup{});  // Sleeping → Ready
```
Add include: `#include "task_messages.hpp"`

Line 43 (`if (current && current->status == TaskStatus::kRunning)`):
```cpp
    if (current && current->GetStatus() == TaskStatus::kRunning) {
```

#### `exit.cpp`

Line 15-16 (assert):
```cpp
  assert(current->GetStatus() == TaskStatus::kRunning &&
         "Exit: current task status must be kRunning");
```

Lines 46-47 (`current->status = TaskStatus::kZombie;`):
```cpp
      current->fsm.Receive(MsgExit{exit_code, true});  // → kZombie
```
Add include: `#include "task_messages.hpp"`

Line 60 (`current->status = TaskStatus::kExited;`):
```cpp
      current->fsm.Receive(MsgExit{exit_code, false});  // → kExited
```

#### `wait.cpp`

Line 16-17 (assert):
```cpp
  assert(current->GetStatus() == TaskStatus::kRunning &&
         "Wait: current task status must be kRunning");
```

Line 51-52 (`task->status == TaskStatus::kZombie || task->status == TaskStatus::kExited`):
```cpp
        if (task->GetStatus() == TaskStatus::kZombie ||
            task->GetStatus() == TaskStatus::kExited) {
```

Line 58 (`task->status == TaskStatus::kBlocked`):
```cpp
        if (untraced && task->GetStatus() == TaskStatus::kBlocked) {
```

Lines 70-72 (assert):
```cpp
      assert((target->GetStatus() == TaskStatus::kZombie ||
              target->GetStatus() == TaskStatus::kExited) &&
             "Wait: target task must be kZombie or kExited");
```

**Step 2: Verify compilation (all three architectures)**

Run:
```bash
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel
```
Expected: All three compile successfully.

**Step 3: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS

**Step 4: Commit**

```bash
git add src/task/task_manager.cpp src/task/schedule.cpp src/task/sleep.cpp \
        src/task/tick_update.cpp src/task/block.cpp src/task/wakeup.cpp \
        src/task/exit.cpp src/task/wait.cpp
git commit --signoff -m "refactor(task): replace all status assignments with TaskFsm messages"
```

---

## Task 5: `etl::observer` for Tick & Panic Broadcast

**Files:**
- Create: `src/include/tick_observer.hpp`
- Create: `src/include/panic_observer.hpp`
- Modify: `src/include/kernel_config.hpp` — add observer capacity constants

**Step 1: Add constants to `kernel_config.hpp`**

Add before the closing `}  // namespace kernel::config`:

```cpp
/// 最大 tick 观察者数
static constexpr size_t kTickObservers = 8;
/// 最大 panic 观察者数
static constexpr size_t kPanicObservers = 4;
```

**Step 2: Create `tick_observer.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Tick event observer interface (etl::observer pattern)
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_

#include <etl/observer.h>

#include <cstdint>

#include "kernel_config.hpp"

/// @brief Tick event payload
struct TickEvent {
  uint64_t jiffies;
};

/// @brief Observer interface for tick events
using ITickObserver = etl::observer<TickEvent>;

/// @brief Observable base for tick event publishers
using TickObservable =
    etl::observable<ITickObserver, kernel::config::kTickObservers>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_TICK_OBSERVER_HPP_
```

**Step 3: Create `panic_observer.hpp`**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Panic event observer interface (etl::observer pattern)
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_

#include <etl/observer.h>

#include <cstdint>

#include "kernel_config.hpp"

/// @brief Panic event payload
struct PanicEvent {
  const char* reason;
  uint64_t pc;
};

/// @brief Observer interface for panic events
using IPanicObserver = etl::observer<PanicEvent>;

/// @brief Observable base for panic event publishers
using PanicObservable =
    etl::observable<IPanicObserver, kernel::config::kPanicObservers>;

#endif  // SIMPLEKERNEL_SRC_INCLUDE_PANIC_OBSERVER_HPP_
```

**Step 4: Verify compilation**

Run: `cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel`
Expected: SUCCESS (headers not yet included by anyone)

**Step 5: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS

**Step 6: Commit**

```bash
git add src/include/kernel_config.hpp \
        src/include/tick_observer.hpp \
        src/include/panic_observer.hpp
git commit --signoff -m "feat(kernel): add tick and panic observer interfaces using etl::observer"
```

**Note:** Wiring up the observer pattern (making timer handlers inherit TickObservable, making schedulers implement ITickObserver, etc.) is a follow-up task that modifies arch interrupt handlers and scheduler classes. This task only creates the interfaces and types.

---

## Task 6: `etl::message_router` for Bottom-Half Routing

**Files:**
- Create: `src/task/include/lifecycle_messages.hpp`
- Create: `src/device/include/virtio_messages.hpp`

**Step 1: Create lifecycle message types**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Thread lifecycle messages for etl::message_router
 */

#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_LIFECYCLE_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_LIFECYCLE_MESSAGES_HPP_

#include <etl/message.h>

#include <cstddef>

/// @brief Lifecycle message IDs (start at 100 to avoid collision with task FSM)
namespace lifecycle_msg_id {
inline constexpr etl::message_id_t kThreadCreate = 100;
inline constexpr etl::message_id_t kThreadExit = 101;
}  // namespace lifecycle_msg_id

/// @brief Thread creation event message
struct ThreadCreateMsg
    : public etl::message<lifecycle_msg_id::kThreadCreate> {
  size_t pid;
  explicit ThreadCreateMsg(size_t p) : pid(p) {}
};

/// @brief Thread exit event message
struct ThreadExitMsg : public etl::message<lifecycle_msg_id::kThreadExit> {
  size_t pid;
  int exit_code;
  ThreadExitMsg(size_t p, int code) : pid(p), exit_code(code) {}
};

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_LIFECYCLE_MESSAGES_HPP_
```

**Step 2: Create virtio message types**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Virtio bottom-half messages for etl::message_router
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_

#include <etl/message.h>

#include <cstdint>

/// @brief Virtio message IDs (start at 200 to avoid collision)
namespace virtio_msg_id {
inline constexpr etl::message_id_t kQueueReady = 200;
}  // namespace virtio_msg_id

/// @brief Virtio queue ready notification (sent by top-half ISR)
struct VirtioQueueReadyMsg
    : public etl::message<virtio_msg_id::kQueueReady> {
  uint32_t queue_index;
  explicit VirtioQueueReadyMsg(uint32_t idx) : queue_index(idx) {}
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_VIRTIO_MESSAGES_HPP_
```

**Step 3: Verify compilation**

Run: `cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel`
Expected: SUCCESS

**Step 4: Run style check**

Run: `pre-commit run --all-files`
Expected: PASS

**Step 5: Commit**

```bash
git add src/task/include/lifecycle_messages.hpp \
        src/device/include/virtio_messages.hpp
git commit --signoff -m "feat(kernel): add lifecycle and virtio message types for etl::message_router"
```

**Note:** Wiring up actual message routers (creating `etl::message_router` subclasses that handle these messages, connecting top-half ISRs to bottom-half workers) is a follow-up task. This task only creates the message type definitions.

---

## Post-Migration Checklist

After all 6 tasks complete:

- [ ] All three architectures compile: `cmake --preset build_{riscv64,aarch64,x86_64} && make SimpleKernel`
- [ ] `pre-commit run --all-files` passes
- [ ] No `TaskStatus status` field remains in `TaskControlBlock`
- [ ] No raw `task->status = ...` assignments remain in any `.cpp` file
- [ ] No `typedef uint64_t (*InterruptFunc)(...)` remains in `interrupt_base.h`
- [ ] No `enum CloneFlags : uint64_t` remains in `task_control_block.hpp`
- [ ] `MsgSchedule`, `MsgYield`, `MsgSleep`, `MsgBlock`, `MsgWakeup`, `MsgExit`, `MsgReap` are the only ways to change task state
- [ ] FSM rejects invalid transitions with `on_event_unknown()` warning log
- [ ] All ETL headers used: `etl/fsm.h`, `etl/message.h`, `etl/delegate.h`, `etl/flags.h`, `etl/observer.h`

## Known Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `etl::delegate` cannot store lambdas | Use static free functions with `delegate::create<fn>()` |
| Sleep/Block rollback pattern | Check capacity BEFORE FSM transition (not after) |
| PLIC has own `InterruptFunc` typedef | Replace with `InterruptBase::InterruptDelegate` alias |
| `etl::fsm` API may differ from docs | Consult `3rd/etl/include/etl/fsm.h` directly for constructor/method signatures |
| `etl::flags` may not support all bitwise ops | Test with clone.cpp patterns first; may need `.value()` calls |
| Multiple TCBs need independent FSM instances | Each `TaskFsm` owns its 7 state instances (no static sharing) |
