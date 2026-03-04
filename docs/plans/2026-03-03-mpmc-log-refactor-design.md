# MPMC Lock-Free Log Refactor — Design Document

**Date:** 2026-03-03
**Status:** Approved
**Supersedes:** `2026-03-03-easylogger-integration-design.md`

## Goal

Replace the SpinLock-based stream logging system (`kernel_log.hpp`) with a lock-free MPMC queue design. The new system must be interrupt-safe, depend on zero subsystems, and guarantee cross-core message ordering.

## Constraints

1. Usable inside interrupt handlers (no blocking)
2. No dependencies on task scheduler, memory manager, or any other subsystem
3. Guaranteed multi-core/async ordering via monotonic sequence number
4. Minimal code, simplest possible API
5. Reuse existing `3rd/MPMCQueue` (Vyukov lock-free MPMC)

## Architecture

```
Producer (any core/context)          Consumer (same or other core)
┌──────────────────────┐             ┌──────────────────────┐
│ klog::Info(fmt, ...) │             │      TryDrain()      │
│   stbsp_vsnprintf    │             │  atomic_flag guard   │
│   atomic seq++       │──push()──>  │  pop() loop          │
│   queue.push(entry)  │             │  PutStr(formatted)   │
│   TryDrain()         │             └──────────────────────┘
└──────────────────────┘
        MPMCQueue<LogEntry, 256>
           (64 KB static, constexpr)
```

- **Push** is lock-free (CAS-based), safe in any context including interrupts
- **Drain** uses `atomic_flag` try-lock — only one core drains at a time, non-blocking for others
- `TryDrain()` called after every push; skipped in interrupt context (`InInterruptContext()`)
- Output goes through `PutStr()` → `etl_putchar()` → arch serial

## Data Structures

```cpp
struct LogEntry {
  uint64_t seq;                    // monotonic sequence number
  uint64_t core_id;                // producing core
  LogLevel::enum_type level;       // log level
  char msg[240];                   // formatted message (stbsp_vsnprintf)
};  // 256 bytes total

inline mpmc_queue::MPMCQueue<LogEntry, 256> log_queue;   // 64 KB static
inline std::atomic<uint64_t> log_seq{0};
inline std::atomic_flag drain_flag = ATOMIC_FLAG_INIT;
inline std::atomic<uint64_t> dropped_count{0};
```

- `LogEntry` is 256 bytes, power-of-2 aligned for cache efficiency
- Queue capacity 256 entries = 64 KB total — fits in BSS, no heap needed
- `log_seq` provides global ordering across cores
- `drain_flag` ensures single-consumer drain (try-lock, not spin)

## API

```cpp
namespace klog {

void Info(const char* fmt, ...);
void Warn(const char* fmt, ...);
void Err(const char* fmt, ...);
void Debug(const char* fmt, ...);
void Flush();                      // force-drain all queued entries
void RawPut(const char* msg);     // direct bypass for panic paths

}  // namespace klog
```

- Printf-style variadic — simplest possible call sites
- `Debug` compiled out when `SIMPLEKERNEL_MIN_LOG_LEVEL` > debug
- `RawPut` bypasses queue entirely — direct `PutStr()` for panic/early boot
- No `source_location` (incompatible with C varargs)

## Drain Mechanism

1. After every `push()`, producer calls `TryDrain()`
2. `TryDrain()` checks `InInterruptContext()` — if true, return immediately
3. `TryDrain()` attempts `drain_flag.test_and_set()` — if already set, return (another core is draining)
4. Drain loop: `pop()` entries, format with color/prefix, output via `PutStr()`
5. After loop: if `dropped_count > 0`, emit drop warning, reset counter
6. Clear `drain_flag`

### Queue-Full Strategy

1. Try `TryDrain()` (inline drain attempt)
2. Retry `push()` once
3. If still full: `dropped_count.fetch_add(1)`, discard entry
4. Next successful drain reports dropped count

## Log Level Filtering

```cpp
#ifndef SIMPLEKERNEL_MIN_LOG_LEVEL
#define SIMPLEKERNEL_MIN_LOG_LEVEL 0  // debug
#endif
```

Compile-time filtering via `if constexpr` or preprocessor — entries below threshold never enter queue.

## Output Format

```
[INFO ] msg\n          — cyan
[WARN ] msg\n          — yellow
[ERROR] msg\n          — red
[DEBUG] msg\n          — green (compile-time gated)
```

ANSI color codes via `PutStr()`. Reset after each line.

## File Layout

- **Rewrite**: `src/include/kernel_log.hpp` (entire file, ~475 → ~200 lines)
- **Migrate**: 59 files, 321 call sites (stream API → printf API)
- **Remove**: `LogLine`, `LogLineRaw`, `LogStream`, `LogStreamRaw`, `DebugBlob`, `klog::hex`, `klog::HEX`, all SpinLock usage in logging

## Dependencies

- `3rd/MPMCQueue/include/MPMCQueue.hpp` — already linked in CMake
- `stbsp_vsnprintf` from `stb_sprintf.h` — already available via `sk_stdio.h`
- `etl_putchar` — arch serial primitive, declared in `sk_stdio.h`
- `<atomic>`, `<cstdint>`, `<cstdarg>` — freestanding headers

No new dependencies introduced.
