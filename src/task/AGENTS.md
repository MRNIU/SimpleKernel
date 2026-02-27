# AGENTS.md — src/task/

## OVERVIEW
Task management subsystem: schedulers (CFS/FIFO/RR/Idle), TaskControlBlock, TaskManager singleton, sync primitives (mutex via spinlock), syscall-level task operations (clone, exit, sleep, wait, wakeup, block).

## STRUCTURE
```
include/
  scheduler_base.hpp       # SchedulerBase ABC — Pick, AddTask, RemoveTask, TickUpdate
  cfs_scheduler.hpp        # CFS (Completely Fair Scheduler) — vruntime-based
  fifo_scheduler.hpp       # FIFO — first-in first-out, no preemption
  rr_scheduler.hpp         # Round-Robin — time-slice based preemption
  idle_scheduler.hpp       # Idle — runs when no other tasks ready
  task_control_block.hpp   # TCB — task state, context, priority, stack
  task_manager.hpp         # Singleton<TaskManager> — owns schedulers, dispatches
  resource_id.hpp          # Typed resource IDs (TaskId, etc.)
schedule.cpp               # Schedule() — main scheduling loop, context switch trigger
task_control_block.cpp     # TCB construction, state transitions
task_manager.cpp           # TaskManager — AddTask, RemoveTask, scheduler selection
tick_update.cpp            # Timer tick handler — calls scheduler TickUpdate
clone.cpp                  # sys_clone — task creation
exit.cpp                   # sys_exit — task termination, cleanup
sleep.cpp                  # sys_sleep — timed task suspension
wait.cpp                   # sys_wait — wait for child task
wakeup.cpp                 # Wakeup — move task from blocked to ready
block.cpp                  # Block — move task from ready to blocked
mutex.cpp                  # Mutex implementation (uses SpinLock internally)
```

## WHERE TO LOOK
- **Adding a scheduler** → Subclass `SchedulerBase` (see `cfs_scheduler.hpp`), implement `Pick()`, `AddTask()`, `RemoveTask()`, `TickUpdate()`
- **Task lifecycle** → clone.cpp (create) → schedule.cpp (run) → exit.cpp (destroy)
- **Context switch** → `schedule.cpp` calls arch-specific `switch.S` via function pointer
- **Sync primitives** → `mutex.cpp` uses `SpinLock` + task blocking; `spinlock.hpp` in `src/include/`

## CONVENTIONS
- One syscall operation per .cpp file (clone, exit, sleep, wait, wakeup, block)
- Schedulers are stateless policy objects — TaskManager owns the task queues
- `Singleton<TaskManager>::GetInstance()` is the global entry point
- TCB contains arch-specific context pointer — populated by `switch.S`
- TODO in `task_manager.cpp`: task stealing across cores (not yet implemented)

## ANTI-PATTERNS
- **DO NOT** call Schedule() before TaskManager initialization in boot sequence
- **DO NOT** hold SpinLock across context switch boundaries — deadlock
- **DO NOT** access TCB fields without proper locking (SpinLock or LockGuard)
- **DO NOT** add scheduler implementations without corresponding test in tests/
