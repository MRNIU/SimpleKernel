/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <etl/enum_type.h>
#include <etl/format_spec.h>
#include <etl/string.h>
#include <etl/string_stream.h>

#include <cstddef>
#include <cstdint>
#include <source_location>

#include "kstd_cstdio"
#include "spinlock.hpp"

namespace klog {
namespace detail {

// 日志专用的自旋锁实例
inline SpinLock log_lock("kernel_log");

/// ANSI 转义码，在支持 ANSI 转义码的终端中可以显示颜色
inline constexpr auto kReset = "\033[0m";
inline constexpr auto kRed = "\033[31m";
inline constexpr auto kGreen = "\033[32m";
inline constexpr auto kYellow = "\033[33m";
inline constexpr auto kBlue = "\033[34m";
inline constexpr auto kMagenta = "\033[35m";
inline constexpr auto kCyan = "\033[36m";
inline constexpr auto kWhite = "\033[37m";

/**
 * @brief 类型安全的日志级别枚举
 * @note 使用 etl::enum_type 提供 c_str() 字符串转换
 */
struct LogLevel {
  enum enum_type : uint8_t {
    kDebug = 0,
    kInfo = 1,
    kWarn = 2,
    kErr = 3,
    kMax = 4,
  };

  ETL_DECLARE_ENUM_TYPE(LogLevel, uint8_t)
  ETL_ENUM_TYPE(kDebug, "DEBUG")
  ETL_ENUM_TYPE(kInfo, "INFO")
  ETL_ENUM_TYPE(kWarn, "WARN")
  ETL_ENUM_TYPE(kErr, "ERR")
  ETL_END_ENUM_TYPE
};

/// 编译期最低日志级别，低于此级别的日志在编译时被消除
inline constexpr auto kMinLogLevel =
    static_cast<LogLevel::enum_type>(SIMPLEKERNEL_MIN_LOG_LEVEL);

/// 每种日志级别对应的 ANSI 颜色 (indexed by enum_type value)
inline constexpr const char* kLogColors[LogLevel::kMax] = {
    // kDebug
    kMagenta,
    // kInfo
    kCyan,
    // kWarn
    kYellow,
    // kErr
    kRed,
};

// ── Header prefix emission ────────────────────────────────────────────────

/**
 * @brief 仅供 DebugBlob 使用：直接向终端输出日志行前缀（ANSI 颜色码 +
 *        [core_id]），不经过 LogLine 缓冲区。
 *        LogLine 自行在构造函数中以流式方式写入缓冲区前缀。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
__always_inline void EmitHeader() {
  sk_print_str(kLogColors[Level]);
  etl_putchar('[');
  sk_print_int(static_cast<long long>(cpu_io::GetCurrentCoreId()));
  etl_putchar(']');
}

// ── LogLine: RAII stream-style log line ──────────────────────────────────

/// @brief No-op tag for disabled log levels
struct NoOpTag {};

/**
 * @brief RAII 流式日志行（基于 etl::string 实例缓冲）
 *
 * 构造时获取自旋锁并向 etl::string<512> 写入颜色前缀+核心ID，
 * 析构时通过 sk_print_str 原子输出完整日志行并释放锁。
 *
 * @tparam Level 日志级别
 * @note Only one LogLine may be live at a time (protected by log_lock).
 * @note Buffer capacity is 512 bytes; content exceeding this limit is
 *       silently truncated by etl::string_stream.
 */
template <LogLevel::enum_type Level>
class LogLine {
 public:
  /// @brief Construct a no-op LogLine (for disabled log levels)
  explicit LogLine(NoOpTag) : released_(true) {}

  /**
   * @brief Construct an active LogLine, acquire lock and emit prefix
   * @param loc source location (captured at call site for kDebug)
   */
  explicit LogLine(
      const std::source_location& loc = std::source_location::current()) {
    AcquireLock();
    s_buf_.clear();
    stream_ << kLogColors[Level] << "[" << cpu_io::GetCurrentCoreId() << "]";
    if constexpr (Level == LogLevel::kDebug) {
      stream_ << "[" << loc.file_name() << ":" << loc.line() << " "
              << loc.function_name() << "] ";
    }
  }

  LogLine(LogLine&& other) noexcept
      : s_buf_(other.s_buf_), stream_(s_buf_), released_(other.released_) {
    other.released_ = true;
    other.s_buf_.clear();
  }

  LogLine(const LogLine&) = delete;
  auto operator=(const LogLine&) -> LogLine& = delete;
  auto operator=(LogLine&&) -> LogLine& = delete;

  ~LogLine() {
    if (!released_) {
      stream_ << "\n" << kReset;
      sk_print_str(s_buf_.c_str());
      ReleaseLock();
    }
  }

  /// @brief Stream any value supported by etl::string_stream
  template <typename T>
  auto operator<<(T&& val) -> LogLine& {
    if (!released_) {
      stream_ << static_cast<T&&>(val);
    }
    return *this;
  }

  /// @brief Stream bool as "true"/"false"
  auto operator<<(bool val) -> LogLine& {
    if (!released_) {
      stream_ << (val ? "true" : "false");
    }
    return *this;
  }

  /// @brief Stream a single char directly (bypasses ETL integral formatting)
  auto operator<<(char c) -> LogLine& {
    if (!released_) {
      s_buf_.push_back(c);
    }
    return *this;
  }

  /// @brief Stream pointer as "0x<hex>"
  auto operator<<(const void* val) -> LogLine& {
    if (!released_) {
      stream_ << "0x";
      etl::format_spec spec{};
      spec.hex();
      stream_ << spec << reinterpret_cast<uintptr_t>(val);
    }
    return *this;
  }

  auto operator<<(void* val) -> LogLine& {
    return operator<<(static_cast<const void*>(val));
  }

 private:
  /// @brief Acquire log_lock; spin-halt on failure (e.g. recursive lock).
  static void AcquireLock() {
    log_lock.Lock().or_else([](auto&& err) -> Expected<void> {
      sk_print_str("LogLine: Failed to acquire lock: ");
      sk_print_str(err.message());
      etl_putchar('\n');
      while (true) {
        cpu_io::Pause();
      }
      __builtin_unreachable();
    });
  }

  /// @brief Release log_lock; spin-halt on failure (e.g. not owned).
  static void ReleaseLock() {
    log_lock.UnLock().or_else([](auto&& err) -> Expected<void> {
      sk_print_str("LogLine: Failed to release lock: ");
      sk_print_str(err.message());
      etl_putchar('\n');
      while (true) {
        cpu_io::Pause();
      }
      __builtin_unreachable();
    });
  }

  etl::string<512> s_buf_{};
  etl::string_stream stream_{s_buf_};
  bool released_ = false;
};

/**
 * @brief 惰性流式日志代理（LogStream）
 *
 * 全局实例（如 klog::info），第一次 operator<< 创建 LogLine 临时对象，
 * 后续 << 链式调用在同一 LogLine 上进行，
 * 语句结束时 LogLine 析构自动换行并释放锁。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
struct LogStream {
  /// @brief 通过 << 创建 LogLine 并输出第一个值
  /// @param val  第一个要输出的值
  /// @note source_location 仅在 klog::debug() 函数调用时捕获；
  ///       info/warn/err 的流式 << 接口不支持捕获调用方位置，
  ///       因为 C++ 不允许在成员函数模板的 operator<< 上使用
  ///       std::source_location 默认参数。
  template <typename T>
  auto operator<<(T&& val) -> LogLine<Level> {
    if constexpr (Level < kMinLogLevel) {
      return LogLine<Level>{NoOpTag{}};
    }
    LogLine<Level> line{};
    line << static_cast<T&&>(val);
    return line;
  }
};

}  // namespace detail

/**
 * @brief 以十六进制输出内存块内容
 * @param data 数据指针
 * @param size 数据大小（字节）
 */
__always_inline void DebugBlob([[maybe_unused]] const void* data,
                               [[maybe_unused]] size_t size) {
  if constexpr (detail::LogLevel::kDebug >= detail::kMinLogLevel) {
    LockGuard<SpinLock> lock_guard(klog::detail::log_lock);
    detail::EmitHeader<detail::LogLevel::kDebug>();
    etl_putchar(' ');
    for (size_t i = 0; i < size; i++) {
      etl_putchar('0');
      etl_putchar('x');
      sk_print_hex(static_cast<unsigned long long>(
                       reinterpret_cast<const uint8_t*>(data)[i]),
                   /*width=*/2, /*upper=*/1);
      etl_putchar(' ');
    }
    sk_print_str(detail::kReset);
    etl_putchar('\n');
  }
}

/// @brief Hex format spec for stream-style integer formatting (lowercase 0x
/// prefix)
inline const etl::format_spec hex = etl::format_spec{}.hex().show_base(true);

/// @brief Hex format spec for stream-style integer formatting (uppercase 0X
/// prefix)
inline const etl::format_spec HEX =
    etl::format_spec{}.hex().upper_case(true).show_base(true);

/// @brief 流式日志实例（使用 inline 避免跨 TU 重复定义）
/// @{
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kInfo> info;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kWarn> warn;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kErr> err;
/// @}

/**
 * @brief Create a Debug-level LogLine with source location captured at call
 * site
 *
 * Usage: `klog::debug() << "msg " << val;`
 * @param loc automatically captured source location
 * @note [[nodiscard]] 修饰确保返回的 LogLine 不被意外丢弃——若丢弃，析构函数
 *       将立即执行，日志内容在未写入任何消息的情况下输出（仅含前缀），
 *       且持有的 log_lock 会被立即释放。
 */
[[nodiscard]] inline auto debug(
    std::source_location loc = std::source_location::current())
    -> detail::LogLine<detail::LogLevel::kDebug> {
  if constexpr (detail::LogLevel::kDebug < detail::kMinLogLevel) {
    return detail::LogLine<detail::LogLevel::kDebug>{detail::NoOpTag{}};
  }
  return detail::LogLine<detail::LogLevel::kDebug>{loc};
}

/// @todo 可插拔输出 sink：使用 etl::delegate 将输出重定向到串口/内存缓冲区等
/// 需要引入缓冲机制后实现

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
