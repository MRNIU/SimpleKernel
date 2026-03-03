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
