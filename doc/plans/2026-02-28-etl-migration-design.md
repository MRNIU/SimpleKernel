# ETL Integration Migration Design

## Goal

Migrate SimpleKernel's task subsystem and interrupt subsystem to use ETL's type-safe abstractions (`etl::fsm`, `etl::delegate`, `etl::flags`, `etl::observer`, `etl::message_router`), replacing raw enums, function pointers, and scattered state assignments with formalized, compile-time-checked mechanisms.

## Background

SimpleKernel already integrates ETL as a submodule (`3rd/etl/`) and uses `etl::singleton`, `etl::unordered_map`, `etl::list`, `etl::vector`, and `etl::priority_queue` throughout the codebase. This migration extends ETL usage to control flow and event handling.

### Current Pain Points

1. **TaskStatus** is a raw `enum TaskStatus : uint8_t` with state transitions scattered across 8 `.cpp` files via direct assignment (`task->status = kReady`). No enforcement of valid transitions.
2. **InterruptFunc** is a raw function pointer `typedef uint64_t (*InterruptFunc)(...)` that cannot bind instance methods without global state.
3. **CloneFlags** and **cpu_affinity** are raw `uint64_t` values with no type distinction — they can be accidentally swapped.
4. **Tick distribution** is hardcoded: the timer interrupt handler directly calls `TaskManager::TickUpdate`, rather than broadcasting to subscribers.

## Architecture

Six migration steps in dependency order. Steps 1-3 are independent and can be parallelized. Step 4 depends on Step 1. Steps 5-6 depend on Step 4.

```
Step 1: task_messages.hpp ─┐
Step 2: etl::flags          ├─ Parallel (independent)
Step 3: etl::delegate      ─┘
Step 4: etl::fsm            ← Depends on Step 1
Step 5: etl::observer       ← Depends on Step 4
Step 6: etl::message_router ← Depends on Steps 1, 4
```

## Constraints

- C++23 freestanding. No exceptions, no RTTI, no heap allocation before memory init.
- Google C++ Style Guide enforced by `.clang-format`.
- `kernel_config.hpp` constants are NOT to be modified — observer capacity constants will be added to it.
- ETL config is via CMake (`ETL_CPP23_SUPPORTED`, `ETL_VERBOSE_ERRORS`). No `etl_profile.h` changes.
- All three architectures (x86_64, riscv64, aarch64) must compile after each step.

---

## Step 1: Message & Router ID Registry (`task_messages.hpp`)

### What

Create `src/task/include/task_messages.hpp` — the single source of truth for all `etl::message_id_t` and `etl::message_router_id_t` values used by the task FSM and message routing subsystems.

### Why

ETL's `etl::message<ID>` requires globally unique integer IDs. Scattering these across files guarantees collisions and maintenance burden.

### Design

```cpp
// src/task/include/task_messages.hpp
#ifndef SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
#define SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_

#include <etl/message.h>

namespace task_msg_id {
  inline constexpr etl::message_id_t kSchedule = 1;
  inline constexpr etl::message_id_t kYield    = 2;
  inline constexpr etl::message_id_t kSleep    = 3;
  inline constexpr etl::message_id_t kBlock    = 4;
  inline constexpr etl::message_id_t kWakeup   = 5;
  inline constexpr etl::message_id_t kExit     = 6;
  inline constexpr etl::message_id_t kReap     = 7;
}  // namespace task_msg_id

namespace router_id {
  constexpr etl::message_router_id_t kTimerHandler = 0;
  constexpr etl::message_router_id_t kTaskFsm      = 1;
  constexpr etl::message_router_id_t kVirtioBlk    = 2;
  constexpr etl::message_router_id_t kVirtioNet    = 3;
}  // namespace router_id

#endif  // SIMPLEKERNEL_SRC_TASK_INCLUDE_TASK_MESSAGES_HPP_
```

### Files

- **Create**: `src/task/include/task_messages.hpp`
- **Modify**: `src/task/CMakeLists.txt` (add header to sources if needed)

---

## Step 2: `etl::flags` for CloneFlags & CpuAffinity

### What

Replace the raw `enum CloneFlags : uint64_t` and `uint64_t cpu_affinity` in `TaskControlBlock` with type-safe `etl::flags<uint64_t, mask>` wrappers.

### Why

`CloneFlags` and `cpu_affinity` are both `uint64_t` and can be accidentally swapped at call sites. `etl::flags` provides distinct types with masked bit operations.

### Design

```cpp
// In task_control_block.hpp — replace enum CloneFlags
#include <etl/flags.h>

namespace clone_flag {
  inline constexpr uint64_t kVm      = 0x00000100;
  inline constexpr uint64_t kFs      = 0x00000200;
  inline constexpr uint64_t kFiles   = 0x00000400;
  inline constexpr uint64_t kSighand = 0x00000800;
  inline constexpr uint64_t kParent  = 0x00008000;
  inline constexpr uint64_t kThread  = 0x00010000;
  inline constexpr uint64_t kAllMask = kVm | kFs | kFiles | kSighand | kParent | kThread;
}  // namespace clone_flag

using CloneFlags = etl::flags<uint64_t, clone_flag::kAllMask>;
using CpuAffinity = etl::flags<uint64_t>;
```

TCB changes:
- `CloneFlags clone_flags = kCloneAll;` → `CloneFlags clone_flags{};` (default = 0 = all shared)
- `uint64_t cpu_affinity = UINT64_MAX;` → `CpuAffinity cpu_affinity{UINT64_MAX};`

### Files

- **Modify**: `src/task/include/task_control_block.hpp` — replace enum and field types
- **Modify**: `src/task/clone.cpp` — update flag checks from `(flags & kCloneVm)` to `flags.test(clone_flag::kVm)`
- **Verify**: All arch builds compile

---

## Step 3: `etl::delegate` for InterruptFunc

### What

Replace the raw function pointer `typedef uint64_t (*InterruptFunc)(...)` in `interrupt_base.h` with `etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>`.

### Why

Raw function pointers cannot bind instance methods. Drivers currently use static adapter functions with global `this` pointers — fragile and breaks encapsulation. `etl::delegate` supports member function binding with zero heap allocation.

### Design

```cpp
// In interrupt_base.h
#include <etl/delegate.h>

class InterruptBase {
 public:
  using InterruptDelegate = etl::delegate<uint64_t(uint64_t, cpu_io::TrapContext*)>;

  virtual void RegisterInterruptFunc(uint64_t cause, InterruptDelegate delegate) = 0;
  virtual auto RegisterExternalInterrupt(uint32_t irq, uint32_t cpu_id,
                                         uint32_t priority,
                                         InterruptDelegate handler)
      -> Expected<void> = 0;
  // ... rest unchanged
};
```

### Migration Strategy

For existing free-function handlers (which are the current pattern in all three arch interrupt.cpp files), `etl::delegate::create<FreeFunction>()` is a drop-in replacement. No behavioral change — just type-safe wrapping.

### Files

- **Modify**: `src/include/interrupt_base.h` — replace typedef + update virtual signatures
- **Modify**: `src/arch/aarch64/interrupt.cpp` — update RegisterInterruptFunc call sites
- **Modify**: `src/arch/riscv64/interrupt.cpp` — same
- **Modify**: `src/arch/x86_64/interrupt.cpp` — same
- **Verify**: All arch builds compile

---

## Step 4: `etl::fsm` for TaskStatus

### What

Replace the `enum TaskStatus` + scattered `task->status = kXxx` assignments with a formal `etl::fsm` state machine embedded in each `TaskControlBlock`.

### Why

State transitions are currently spread across 8 `.cpp` files with no validation. The FSM centralizes all valid transitions, provides `on_enter_state()`/`on_exit_state()` hooks, and rejects illegal transitions via `on_event_unknown()`.

### Design

**7 states**: `StateUnInit`, `StateReady`, `StateRunning`, `StateSleeping`, `StateBlocked`, `StateZombie`, `StateExited`

**7 messages**: `MsgSchedule`, `MsgYield`, `MsgSleep{wake_tick}`, `MsgBlock{resource}`, `MsgWakeup`, `MsgExit{exit_code, has_parent}`, `MsgReap`

**State transition map** (enforced by FSM):

| From | Message | To |
|------|---------|-----|
| kUnInit | MsgSchedule | kReady |
| kReady | MsgSchedule | kRunning |
| kRunning | MsgYield | kReady |
| kRunning | MsgSleep | kSleeping |
| kRunning | MsgBlock | kBlocked |
| kRunning | MsgExit(has_parent=true) | kZombie |
| kRunning | MsgExit(has_parent=false) | kExited |
| kSleeping | MsgWakeup | kReady |
| kBlocked | MsgWakeup | kReady |
| kZombie | MsgReap | (terminal) |
| kExited | MsgReap | (terminal) |

**TCB embedding**: `TaskFsm` is a member of `TaskControlBlock`. Constructor creates it; `Init()` called after TCB is fully constructed. Each `TaskFsm` owns its 7 state instances (no static sharing — supports multiple TCBs).

**Status query**: `TaskStatus` enum is retained as state IDs. `tcb.fsm.get_state_id()` replaces `tcb.status` reads. A helper `tcb.GetStatus()` method wraps this.

### Replacement Map

| File | Current Code | Replacement |
|------|-------------|-------------|
| `task_manager.cpp:AddTask` | `task->status = kReady` | `task->fsm.receive(MsgSchedule{})` |
| `task_manager.cpp:InitCurrentCore` | idle `status = kRunning` | `idle->fsm.receive(MsgSchedule{})` twice (UnInit→Ready→Running) |
| `schedule.cpp` | `current->status = kReady; next->status = kRunning` | `current->fsm.receive(MsgYield{}); next->fsm.receive(MsgSchedule{})` |
| `sleep.cpp` | `task->status = kSleeping` | `task->fsm.receive(MsgSleep{wake_tick})` |
| `tick_update.cpp` | `task->status = kReady` | `task->fsm.receive(MsgWakeup{})` |
| `block.cpp` | `task->status = kBlocked` | `task->fsm.receive(MsgBlock{resource})` |
| `wakeup.cpp` | `task->status = kReady` | `task->fsm.receive(MsgWakeup{})` |
| `exit.cpp` | `task->status = kZombie/kExited` | `task->fsm.receive(MsgExit{code, has_parent})` |
| `wait.cpp` | reads `kZombie`/`kExited` | reads `task->GetStatus()` |

### Files

- **Create**: `src/task/include/task_fsm.hpp` — state classes, message structs, TaskFsm class
- **Modify**: `src/task/include/task_messages.hpp` — add message struct definitions (depends on Step 1)
- **Modify**: `src/task/include/task_control_block.hpp` — add `TaskFsm fsm` member, `GetStatus()` helper, remove `TaskStatus status` field
- **Modify**: `src/task/task_manager.cpp`, `schedule.cpp`, `sleep.cpp`, `tick_update.cpp`, `block.cpp`, `wakeup.cpp`, `exit.cpp`, `wait.cpp` — replace all status assignments/reads
- **Modify**: `src/task/task_control_block.cpp` — initialize FSM in constructor
- **Verify**: All arch builds compile

---

## Step 5: `etl::observer` for Tick & Panic Broadcast

### What

Implement observer pattern for timer tick distribution and kernel panic broadcast, decoupling the timer interrupt handler from specific subscribers.

### Why

Currently `TimerInterrupt → TaskManager::TickUpdate` is a hardcoded call. Adding new tick consumers (watchdog, profiler, power management) requires modifying the interrupt handler. Observer pattern allows subscribe/unsubscribe without touching the publisher.

### Design

**Tick Observer**:
- `struct TickEvent { uint64_t jiffies; }`
- `using ITickObserver = etl::observer<TickEvent>`
- Timer interrupt handler inherits `etl::observable<ITickObserver, kTickObservers>`
- Scheduler, sleep queue subscribe as observers

**Panic Observer**:
- `struct PanicEvent { const char* reason; uint64_t pc; }`
- `using IPanicObserver = etl::observer<PanicEvent>`
- Kernel panic function inherits `etl::observable<IPanicObserver, kPanicObservers>`
- Filesystem, watchdog subscribe as observers

### Files

- **Modify**: `src/include/kernel_config.hpp` — add `kTickObservers`, `kPanicObservers` constants
- **Create**: `src/include/tick_observer.hpp` — TickEvent, ITickObserver types
- **Create**: `src/include/panic_observer.hpp` — PanicEvent, IPanicObserver types
- **Modify**: arch interrupt handlers — inherit observable, call `notify_observers()`
- **Modify**: scheduler classes — inherit ITickObserver
- **Verify**: All arch builds compile

---

## Step 6: `etl::message_router` for Bottom-Half Routing

### What

Implement message routing for interrupt bottom-half processing (Virtio devices) and thread lifecycle events, decoupling top-half interrupt handlers from device-specific processing.

### Why

Virtio interrupt handlers currently must inline all device processing. With message routing, the top-half constructs a lightweight message and dispatches it to a worker thread's router, keeping interrupt latency minimal.

### Design

**Virtio Bottom-Half**:
- Top half: `static VirtioQueueReadyMsg msg; etl::send_message(router, msg);`
- Bottom half router: `VirtioBottomHalf::on_receive(VirtioQueueReadyMsg)` dispatches to `VirtioBlkDriver::ProcessQueue()` or `VirtioNetDriver::ProcessQueue()`

**Thread Lifecycle Bus**:
- `ThreadExitMsg` broadcast to memory manager (reclaim stacks) and mutex manager (release orphaned locks)

### Files

- **Create**: `src/task/include/lifecycle_messages.hpp` — ThreadExitMsg, ThreadCreateMsg
- **Create**: `src/device/include/virtio_messages.hpp` — VirtioQueueReadyMsg
- **Modify**: relevant device/task files to use message routing
- **Verify**: All arch builds compile

---

## Verification Strategy

After each step:
1. `cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel` — kernel compiles
2. `lsp_diagnostics` on all modified files — no new errors
3. `pre-commit run --all-files` — style check passes
4. If unit tests exist for modified code, run `make unit-test`

## Reference Documents

- `doc/etl/etl_delegate_fsm.md` — detailed delegate, FSM, flags migration with code examples
- `doc/etl/etl_advanced.md` — FSM lifecycle, observer, message_router patterns and caveats
- `doc/etl/etl_containers.md` — ETL container usage patterns (for reference)
