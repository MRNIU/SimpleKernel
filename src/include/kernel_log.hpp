/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <cpu_io.h>
#include <etl/format.h>

#include <MPMCQueue.hpp>
#include <atomic>
#include <cstdint>
#include <tuple>
#include <type_traits>

#include "kstd_cstdio"

namespace klog {
namespace detail {

/// @brief Map types unsupported by ETL's basic_format_arg to supported ones.
/// ETL format lacks constructors for `long` and `unsigned long` (LP64 issue).
/// This maps them to `long long` / `unsigned long long` respectively.
/// All other types pass through unchanged.
/// @{
template <typename T>
struct EtlArgMap {
  using type = std::decay_t<T>;
};
template <>
struct EtlArgMap<long> {
  using type = long long;
};
template <>
struct EtlArgMap<unsigned long> {
  using type = unsigned long long;
};
/// @}

using WrapperIt = etl::private_format::limit_iterator<char*>;

/// @brief Non-template bridge to etl::vformat_to.
/// Defined in klog_format.cpp which is compiled without -mgeneral-regs-only
/// so that ETL's floating-point formatting templates can be instantiated.
__always_inline auto VFormatToN(
    char* buf, size_t n, etl::string_view fmt,
    etl::format_args<etl::private_format::limit_iterator<char*>> args)
    -> char* {
  return etl::vformat_to(WrapperIt(buf, n), fmt, args).get();
}

/// @brief Safe wrapper around etl::format_to_n that casts args to
/// ETL-compatible types before formatting.  Works around the missing
/// `long` / `unsigned long` constructors in ETL's basic_format_arg.
/// The actual vformat_to call is in a separate TU (klog_format.cpp)
/// to avoid floating-point template instantiation under -mgeneral-regs-only.
template <typename... Args>
__always_inline auto FormatToN(char* buf, size_t n,
                               etl::format_string<Args...> fmt, Args&&... args)
    -> char* {
  using WrapperIt = etl::private_format::limit_iterator<char*>;
  // Materialize cast values as named locals so make_format_args gets lvalue
  // refs. EtlArgMap converts long -> long long, unsigned long -> unsigned long
  // long.
  auto cast_tuple =
      std::tuple<typename EtlArgMap<std::remove_cvref_t<Args>>::type...>(
          static_cast<typename EtlArgMap<std::remove_cvref_t<Args>>::type>(
              args)...);
  return std::apply(
      [&](auto&... cast_args) -> char* {
        auto the_args = etl::make_format_args<WrapperIt>(cast_args...);
        return VFormatToN(buf, n, fmt.get(),
                          etl::format_args<WrapperIt>(the_args));
      },
      cast_tuple);
}

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
    auto* end = FormatToN(drop_buf, sizeof(drop_buf) - 1,
                          "\033[31m[LOG] dropped {} entries\033[0m\n",
                          static_cast<uint64_t>(dropped));
    *end = '\0';
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
/// @tparam Args variadic format argument types
template <Level Lvl, typename... Args>
__always_inline void Log(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (Lvl < kMinLevel) {
    return;
  }

  LogEntry entry{};
  entry.seq = log_seq.fetch_add(1, std::memory_order_relaxed);
  entry.core_id = cpu_io::GetCurrentCoreId();
  entry.level = Lvl;
  auto* end = FormatToN(entry.msg, sizeof(entry.msg) - 1, fmt,
                        static_cast<Args&&>(args)...);
  *end = '\0';

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
template <typename... Args>
inline void Debug(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kDebug < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kDebug>(fmt, static_cast<Args&&>(args)...);
}

/// @brief Log at INFO level
template <typename... Args>
inline void Info(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kInfo < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kInfo>(fmt, static_cast<Args&&>(args)...);
}

/// @brief Log at WARN level
template <typename... Args>
inline void Warn(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kWarn < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kWarn>(fmt, static_cast<Args&&>(args)...);
}

/// @brief Log at ERROR level
template <typename... Args>
inline void Err(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kErr < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kErr>(fmt, static_cast<Args&&>(args)...);
}

/// @brief Force-drain all queued log entries to serial output
__always_inline void Flush() { detail::TryDrain(); }

/// @brief Direct serial output bypassing queue (for panic paths)
__always_inline void RawPut(const char* msg) { detail::PutStr(msg); }

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
