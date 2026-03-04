# MPMC Lock-Free Log Refactor — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the SpinLock-based stream logging system with a lock-free MPMC queue design, then migrate all 59 call-site files to the new printf-style API.

**Architecture:** Lock-free `MPMCQueue<LogEntry, 256>` as the core transport. Producers format with `stbsp_vsnprintf` and push entries. A `TryDrain()` function (guarded by `atomic_flag`) pops and outputs entries via `PutStr()`. Since no `InInterruptContext()` mechanism exists yet, `TryDrain()` always attempts drain (the atomic_flag try-lock is non-blocking, so interrupt handlers won't spin).

**Tech Stack:** C++23 freestanding, `MPMCQueue.hpp` (Vyukov lock-free), `stb_sprintf.h` (`stbsp_vsnprintf`), `etl_putchar` (arch serial primitive)

**Reference:**
- Design doc: `docs/plans/2026-03-03-mpmc-log-refactor-design.md`
- Current logging: `src/include/kernel_log.hpp` (475 lines)
- MPMC queue: `3rd/MPMCQueue/include/MPMCQueue.hpp`
- Printf impl: `src/libc/include/sk_stdio.h` (provides `stbsp_vsnprintf` via `stb_sprintf.h`)
- Build config: `cmake/compile_config.cmake` (already links `MPMCQueue`, defines `SIMPLEKERNEL_MIN_LOG_LEVEL`)

---

### Task 1: Rewrite `kernel_log.hpp` — New MPMC-based logging system

**Files:**
- Rewrite: `src/include/kernel_log.hpp`

**Step 1: Replace the entire content of `kernel_log.hpp`**

Write the following complete replacement:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <MPMCQueue.hpp>

#include <atomic>
#include <cstdarg>
#include <cstdint>

#include "kstd_cstdio"

namespace klog {
namespace detail {

/// ANSI escape codes
inline constexpr auto kReset = "\033[0m";
inline constexpr auto kRed = "\033[31m";
inline constexpr auto kGreen = "\033[32m";
inline constexpr auto kYellow = "\033[33m";
inline constexpr auto kCyan = "\033[36m";

/// Log levels
enum Level : uint8_t {
  kDebug = 0,
  kInfo = 1,
  kWarn = 2,
  kErr = 3,
};

/// Compile-time minimum log level
inline constexpr auto kMinLevel =
    static_cast<Level>(SIMPLEKERNEL_MIN_LOG_LEVEL);

/// Level labels (fixed-width for aligned output)
inline constexpr const char* kLevelLabel[] = {
    "[DEBUG] ",
    "[INFO ] ",
    "[WARN ] ",
    "[ERROR] ",
};

/// Level colors (indexed by Level enum)
inline constexpr const char* kLevelColor[] = {
    kGreen,   // debug
    kCyan,    // info
    kYellow,  // warn
    kRed,     // err
};

/// Log entry stored in the MPMC queue
struct LogEntry {
  uint64_t seq;
  uint64_t core_id;
  Level level;
  char msg[239];
};
static_assert(sizeof(LogEntry) == 256, "LogEntry must be 256 bytes");

/// Global queue (64 KB static, constexpr-constructed)
inline mpmc_queue::MPMCQueue<LogEntry, 256> log_queue;

/// Monotonic sequence counter for cross-core ordering
inline std::atomic<uint64_t> log_seq{0};

/// Single-consumer drain guard (non-blocking try-lock)
inline std::atomic_flag drain_flag = ATOMIC_FLAG_INIT;

/// Count of dropped entries (queue-full)
inline std::atomic<uint64_t> dropped_count{0};

/// Low-level string output via etl_putchar (NULL-safe)
__always_inline void PutStr(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar(static_cast<unsigned char>(*s));
    ++s;
  }
}

/// Drain all queued entries to serial output.
/// Uses atomic_flag as non-blocking try-lock — only one core drains at a time.
/// Other cores skip silently (their entries will be drained next time).
inline void TryDrain() {
  // Non-blocking try-lock: if another core is draining, return immediately
  if (drain_flag.test_and_set(std::memory_order_acquire)) {
    return;
  }

  // Report dropped entries if any
  auto dropped = dropped_count.exchange(0, std::memory_order_relaxed);
  if (dropped > 0) {
    char drop_buf[64];
    stbsp_snprintf(drop_buf, static_cast<int>(sizeof(drop_buf)),
                   "\033[31m[LOG] dropped %llu entries\033[0m\n",
                   static_cast<unsigned long long>(dropped));
    PutStr(drop_buf);
  }

  // Drain loop
  LogEntry entry{};
  while (log_queue.pop(entry)) {
    PutStr(kLevelColor[entry.level]);
    PutStr(kLevelLabel[entry.level]);
    PutStr(entry.msg);
    PutStr(kReset);
    PutStr("\n");
  }

  drain_flag.clear(std::memory_order_release);
}

/// Core implementation: format message, push to queue, try drain.
/// @tparam Lvl compile-time log level for filtering
template <Level Lvl>
__always_inline void Log(const char* fmt, va_list args) {
  if constexpr (Lvl < kMinLevel) {
    return;
  }

  LogEntry entry{};
  entry.seq = log_seq.fetch_add(1, std::memory_order_relaxed);
  entry.core_id = cpu_io::GetCurrentCoreId();
  entry.level = Lvl;
  stbsp_vsnprintf(entry.msg, static_cast<int>(sizeof(entry.msg)), fmt, args);

  if (!log_queue.push(entry)) {
    // Queue full: try drain, then retry once
    TryDrain();
    if (!log_queue.push(entry)) {
      dropped_count.fetch_add(1, std::memory_order_relaxed);
      return;
    }
  }

  TryDrain();
}

}  // namespace detail

/// @brief Log at DEBUG level (compiled out when SIMPLEKERNEL_MIN_LOG_LEVEL > 0)
__always_inline void Debug(const char* fmt, ...) {
  if constexpr (detail::Level::kDebug < detail::kMinLevel) {
    return;
  }
  va_list args;
  va_start(args, fmt);
  detail::Log<detail::Level::kDebug>(fmt, args);
  va_end(args);
}

/// @brief Log at INFO level
__always_inline void Info(const char* fmt, ...) {
  if constexpr (detail::Level::kInfo < detail::kMinLevel) {
    return;
  }
  va_list args;
  va_start(args, fmt);
  detail::Log<detail::Level::kInfo>(fmt, args);
  va_end(args);
}

/// @brief Log at WARN level
__always_inline void Warn(const char* fmt, ...) {
  if constexpr (detail::Level::kWarn < detail::kMinLevel) {
    return;
  }
  va_list args;
  va_start(args, fmt);
  detail::Log<detail::Level::kWarn>(fmt, args);
  va_end(args);
}

/// @brief Log at ERROR level
__always_inline void Err(const char* fmt, ...) {
  if constexpr (detail::Level::kErr < detail::kMinLevel) {
    return;
  }
  va_list args;
  va_start(args, fmt);
  detail::Log<detail::Level::kErr>(fmt, args);
  va_end(args);
}

/// @brief Force-drain all queued log entries to serial output
__always_inline void Flush() { detail::TryDrain(); }

/// @brief Direct serial output bypassing queue (for panic paths)
__always_inline void RawPut(const char* msg) { detail::PutStr(msg); }

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
```

**Step 2: Verify the rewrite compiles**

This file is header-only and included by 72 files, so a build will test it immediately in Task 2.

**Step 3: Commit**

```bash
git add src/include/kernel_log.hpp
git commit --signoff -m "refactor(log): rewrite kernel_log.hpp with MPMC lock-free queue

Replace SpinLock-based stream logging with Vyukov lock-free MPMC queue.
New API: klog::Info/Warn/Err/Debug (printf-style), klog::Flush, klog::RawPut.
Remove: LogLine, LogLineRaw, LogStream, LogStreamRaw, DebugBlob, klog::hex/HEX."
```

---

### Task 2: Migrate call sites — `src/main.cpp`

**Files:**
- Modify: `src/main.cpp`

**Step 1: Migrate all klog call sites**

The old includes can stay (`#include "kernel_log.hpp"` is unchanged).
Remove any `#include <etl/format_spec.h>` or `#include <etl/string_stream.h>` if they were only needed for logging.

Conversion rules for ALL migration tasks (Tasks 2–15):

| Old Pattern | New Pattern |
|---|---|
| `klog::info << "text" << val;` | `klog::Info("text %s", val_as_str);` or `klog::Info("text %d", val);` |
| `klog::err << "text" << val;` | `klog::Err("text %s", val);` |
| `klog::warn << "text" << val;` | `klog::Warn("text %s", val);` |
| `klog::debug() << "text" << val;` | `klog::Debug("text %s", val);` |
| `klog::raw_info << "text";` | `klog::Info("text");` |
| `klog::raw_err << "text" << val;` | `klog::Err("text %s", val);` |
| `klog::raw_debug() << "text";` | `klog::Debug("text");` |
| `klog::hex << intval` | `stbsp_snprintf` with `%llx` or directly `"0x%llx"` in the format string |
| `klog::HEX << intval` | Same, use `%llX` for uppercase |
| `klog::DebugBlob(ptr, size)` | Remove (no call sites exist) |
| Multi-part `<< a << b << c` | Single `klog::Info("a %s c", b)` combining all parts |
| `<< (val ? "true" : "false")` | `"%s", val ? "true" : "false"` |
| `<< reinterpret_cast<void*>(x)` | `"0x%llx", static_cast<unsigned long long>(x)` |

**Format specifiers reference:**
- `const char*` strings → `%s`
- `int` / `uint32_t` → `%d` / `%u`
- `int64_t` / `uint64_t` → `%lld` / `%llu`
- Pointers / addresses as hex → `0x%llx` with `static_cast<unsigned long long>(val)`
- `bool` → use ternary `val ? "true" : "false"` with `%s`

Example migration for `src/main.cpp:36`:
```cpp
// OLD:
klog::info << "Task1: arg = " << klog::hex
           << reinterpret_cast<uintptr_t>(arg);
// NEW:
klog::Info("Task1: arg = 0x%llx",
           static_cast<unsigned long long>(reinterpret_cast<uintptr_t>(arg)));
```

Example migration for `src/main.cpp:39`:
```cpp
// OLD:
klog::info << "Task1: iteration " << i + 1 << "/5";
// NEW:
klog::Info("Task1: iteration %d/5", i + 1);
```

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

If build errors occur, fix them before proceeding.

**Step 3: Commit**

```bash
git add src/main.cpp
git commit --signoff -m "refactor(log): migrate src/main.cpp to new klog API"
```

---

### Task 3: Migrate call sites — `src/arch/x86_64/` (all files)

**Files:**
- Modify: `src/arch/x86_64/arch_main.cpp`
- Modify: `src/arch/x86_64/backtrace.cpp`
- Modify: `src/arch/x86_64/interrupt.cpp`
- Modify: `src/arch/x86_64/interrupt_main.cpp`
- Modify: `src/arch/x86_64/syscall.cpp`
- Modify: `src/arch/x86_64/apic/apic.cpp`
- Modify: `src/arch/x86_64/apic/io_apic.cpp`
- Modify: `src/arch/x86_64/apic/local_apic.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Special attention: `backtrace.cpp` uses `klog::raw_err` — convert to `klog::Err(...)`.
Special attention: APIC files use many `klog::hex` — convert all to `"0x%llx"` format specifier with `static_cast<unsigned long long>(val)`.

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/arch/x86_64/
git commit --signoff -m "refactor(log): migrate src/arch/x86_64/ to new klog API"
```

---

### Task 4: Migrate call sites — `src/arch/riscv64/` (all files)

**Files:**
- Modify: `src/arch/riscv64/arch_main.cpp`
- Modify: `src/arch/riscv64/backtrace.cpp`
- Modify: `src/arch/riscv64/interrupt.cpp`
- Modify: `src/arch/riscv64/interrupt_main.cpp`
- Modify: `src/arch/riscv64/syscall.cpp`
- Modify: `src/arch/riscv64/plic/plic.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Special attention: `backtrace.cpp` uses `klog::raw_err` — convert to `klog::Err(...)`.
Special attention: `interrupt_main.cpp` uses `klog::raw_err` with `klog::hex` — convert to `klog::Err("...", 0x%llx)`.

**Step 2: Build to verify**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/arch/riscv64/
git commit --signoff -m "refactor(log): migrate src/arch/riscv64/ to new klog API"
```

---

### Task 5: Migrate call sites — `src/arch/aarch64/` (all files)

**Files:**
- Modify: `src/arch/aarch64/arch_main.cpp`
- Modify: `src/arch/aarch64/backtrace.cpp`
- Modify: `src/arch/aarch64/interrupt.cpp`
- Modify: `src/arch/aarch64/interrupt_main.cpp`
- Modify: `src/arch/aarch64/syscall.cpp`
- Modify: `src/arch/aarch64/gic/gic.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Special attention: `backtrace.cpp` uses `klog::raw_err` — convert to `klog::Err(...)`.
Special attention: `gic/gic.cpp` may use hex formatting.

**Step 2: Build to verify**

```bash
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/arch/aarch64/
git commit --signoff -m "refactor(log): migrate src/arch/aarch64/ to new klog API"
```

---

### Task 6: Migrate call sites — `src/memory/` (all files)

**Files:**
- Modify: `src/memory/memory.cpp`
- Modify: `src/memory/virtual_memory.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Heavy `klog::hex` / `klog::HEX` usage in `virtual_memory.cpp` — convert all to hex format specifiers.

**Step 2: Build to verify (use any architecture preset)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/memory/
git commit --signoff -m "refactor(log): migrate src/memory/ to new klog API"
```

---

### Task 7: Migrate call sites — `src/device/` (all files)

**Files:**
- Modify: `src/device/device.cpp`
- Modify: `src/device/device_manager.cpp`
- Modify: `src/device/include/device_manager.hpp`
- Modify: `src/device/include/platform_bus.hpp`
- Modify: `src/device/include/driver_registry.hpp` (includes kernel_log but may not have call sites — check)
- Modify: `src/device/ns16550a/ns16550a_driver.hpp`
- Modify: `src/device/pl011/pl011_driver.hpp`
- Modify: `src/device/virtio/virtio_driver.cpp`
- Modify: `src/device/virtio/virtio_driver.hpp` (includes kernel_log — check for call sites)
- Modify: `src/device/virtio/transport/mmio.hpp` (includes kernel_log — check for call sites)
- Modify: `src/device/virtio/device/device_initializer.hpp` (includes kernel_log — check for call sites)
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Some files only `#include "kernel_log.hpp"` but may not have active call sites — that's fine, leave the include.

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/device/
git commit --signoff -m "refactor(log): migrate src/device/ to new klog API"
```

---

### Task 8: Migrate call sites — `src/task/` (all files)

**Files:**
- Modify: `src/task/sleep.cpp`
- Modify: `src/task/clone.cpp`
- Modify: `src/task/task_control_block.cpp`
- Modify: `src/task/schedule.cpp`
- Modify: `src/task/exit.cpp`
- Modify: `src/task/block.cpp`
- Modify: `src/task/wakeup.cpp`
- Modify: `src/task/wait.cpp`
- Modify: `src/task/mutex.cpp`
- Modify: `src/task/tick_update.cpp`
- Modify: `src/task/task_manager.cpp`
- Modify: `src/task/include/rr_scheduler.hpp`
- Modify: `src/task/include/fifo_scheduler.hpp`
- Modify: `src/task/include/cfs_scheduler.hpp`
- Modify: `src/task/include/task_fsm.hpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

`task_fsm.hpp` has many `klog::warn << "TaskFsm:..."` — convert to `klog::Warn("TaskFsm: ...")`.

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/task/
git commit --signoff -m "refactor(log): migrate src/task/ to new klog API"
```

---

### Task 9: Migrate call sites — `src/filesystem/` (all files)

**Files:**
- Modify: `src/filesystem/filesystem.cpp`
- Modify: `src/filesystem/file_descriptor.cpp`
- Modify: `src/filesystem/ramfs/ramfs.cpp`
- Modify: `src/filesystem/fatfs/fatfs.cpp`
- Modify: `src/filesystem/fatfs/diskio.cpp`
- Modify: `src/filesystem/vfs/vfs.cpp`
- Modify: `src/filesystem/vfs/mount.cpp`
- Modify: `src/filesystem/vfs/mkdir.cpp`
- Modify: `src/filesystem/vfs/rmdir.cpp`
- Modify: `src/filesystem/vfs/open.cpp`
- Modify: `src/filesystem/vfs/close.cpp`
- Modify: `src/filesystem/vfs/read.cpp`
- Modify: `src/filesystem/vfs/write.cpp`
- Modify: `src/filesystem/vfs/unlink.cpp`
- Modify: `src/filesystem/vfs/lookup.cpp`
- Modify: `src/filesystem/vfs/readdir.cpp`
- Modify: `src/filesystem/vfs/seek.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

Many VFS files include `kernel_log.hpp` but may have zero or few call sites — check each file.

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/filesystem/
git commit --signoff -m "refactor(log): migrate src/filesystem/ to new klog API"
```

---

### Task 10: Migrate call sites — `src/include/` headers and remaining files

**Files:**
- Modify: `src/include/kernel_fdt.hpp`
- Modify: `src/include/kernel_elf.hpp`
- Modify: `src/include/basic_info.hpp` (includes kernel_log — check for call sites)
- Modify: `src/include/spinlock.hpp` (commented-out klog usage — remove or leave as comments)
- Modify: `src/syscall.cpp`
- Modify: `src/io_buffer.cpp` (includes kernel_log — check for call sites)
- Modify: `src/libcxx/kstd_libcxx.cpp`

**Step 1: Migrate all klog call sites using the conversion rules from Task 2**

`kernel_fdt.hpp` has heavy `klog::debug()` and `klog::HEX` usage — convert carefully.
`spinlock.hpp` has only commented-out klog usage — update the comments to reflect new API or remove them.

**Step 2: Build to verify**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

**Step 3: Commit**

```bash
git add src/include/ src/syscall.cpp src/io_buffer.cpp src/libcxx/
git commit --signoff -m "refactor(log): migrate src/include/ and remaining files to new klog API"
```

---

### Task 11: Clean up unused includes

**Files:**
- All 72 files that include `kernel_log.hpp`

**Step 1: Search for now-unused includes**

After the migration, these includes are no longer needed by any file (they were only used by the old logging system):
- `#include <etl/enum_type.h>` — only if not used elsewhere in the file
- `#include <etl/format_spec.h>` — only if not used elsewhere in the file
- `#include <etl/string.h>` — only if not used elsewhere in the file
- `#include <etl/string_stream.h>` — only if not used elsewhere in the file
- `#include <source_location>` — only if not used elsewhere in the file
- `#include "spinlock.hpp"` — only if not used elsewhere in the file

These were pulled in transitively through `kernel_log.hpp`. The new `kernel_log.hpp` no longer includes them, so files that relied on them transitively may need to add direct includes — or more likely, they already have their own includes.

**Step 2: Build all three architectures to verify**

```bash
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
```

**Step 3: Commit**

```bash
git add -A
git commit --signoff -m "refactor(log): clean up unused includes after klog migration"
```

---

### Task 12: Run pre-commit format check

**Step 1: Run format check**

```bash
pre-commit run --all-files
```

**Step 2: Fix any formatting issues**

If `clang-format` modifies files, stage and commit:

```bash
git add -A
git commit --signoff -m "style: apply clang-format after klog refactor"
```

---

### Task 13: Final verification — build all architectures

**Step 1: Clean build all three architectures**

```bash
cmake --preset build_x86_64 && make -C build_x86_64 SimpleKernel
cmake --preset build_riscv64 && make -C build_riscv64 SimpleKernel
cmake --preset build_aarch64 && make -C build_aarch64 SimpleKernel
```

All three must succeed with exit code 0.

**Step 2: Run unit tests (if on matching host architecture)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make unit-test
```

**Step 3: Verify no regressions**

Grep for any remaining old API usage:

```bash
grep -rn 'klog::\(info\|warn\|err\|raw_info\|raw_warn\|raw_err\|raw_debug\|hex\|HEX\|DebugBlob\) ' src/
```

Expected: zero matches (except possibly in comments).
