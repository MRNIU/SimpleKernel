/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <etl/enum_type.h>

#include <concepts>
#include <cstddef>
#include <cstdint>
#include <source_location>
#include <type_traits>

#include "kstd_cstdio"
#include "spinlock.hpp"

namespace klog {
namespace detail {

// 日志专用的自旋锁实例
inline SpinLock log_lock("kernel_log");

/// ANSI 转义码，在支持 ANSI 转义码的终端中可以显示颜色
static constexpr auto kReset = "\033[0m";
static constexpr auto kRed = "\033[31m";
static constexpr auto kGreen = "\033[32m";
static constexpr auto kYellow = "\033[33m";
static constexpr auto kBlue = "\033[34m";
static constexpr auto kMagenta = "\033[35m";
static constexpr auto kCyan = "\033[36m";
static constexpr auto kWhite = "\033[37m";

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
static constexpr const char* kLogColors[LogLevel::kMax] = {
    kMagenta,  // kDebug
    kCyan,     // kInfo
    kYellow,   // kWarn
    kRed,      // kErr
};

// ── Freestanding-safe format string checker ───────────────────────────────

namespace fmt_detail {

/// 编译期计算格式字符串中 {} 占位符的数量（支持 {:...} 变体）
consteval auto CountFormatArgs(const char* s) -> size_t {
  size_t count = 0;
  for (size_t i = 0; s[i] != '\0'; ++i) {
    if (s[i] == '{') {
      if (s[i + 1] == '}') {
        ++count;
        ++i;  // skip '}'
      } else if (s[i + 1] != '\0' && s[i + 1] != '{') {
        // {:spec} or {:#x} etc. — scan to matching '}'
        size_t j = i + 1;
        while (s[j] != '\0' && s[j] != '}') {
          ++j;
        }
        if (s[j] == '}') {
          ++count;
          i = j;
        }
      }
      // '{{' is an escaped brace, not a placeholder — skip
    }
  }
  return count;
}

/// 格式占位符解析结果
struct FmtSpec {
  const char* next;        ///< 跳过占位符后的字符串指针
  const char* spec_begin;  ///< 说明符起始（'{' 之后），nullptr 表示无说明符
  const char* spec_end;    ///< 说明符末尾（'}' 之前）
};

/// 解析说明符中是否含十六进制/其他格式标志
__always_inline void ParseHexSpec(const char* sb, const char* se, bool& is_hex,
                                  bool& upper, bool& alt) {
  is_hex = false;
  upper = false;
  alt = false;
  if (sb == nullptr) return;
  for (const char* p = sb; p < se; ++p) {
    if (*p == 'x') {
      is_hex = true;
    } else if (*p == 'X') {
      is_hex = true;
      upper = true;
    } else if (*p == '#') {
      alt = true;
    }
  }
}

}  // namespace fmt_detail

// Called only from consteval context when format arg count mismatches.
// Being non-constexpr makes calling it a hard compile-time error in consteval.
// The function name appears in the diagnostic, acting as the error message.
void klog_format_arg_count_mismatch_error();

/**
 * @brief 编译期类型安全格式字符串（内部实现）
 *
 * 使用 {} 语法，编译期校验占位符数量与参数数量一致。
 * 仅依赖 freestanding 安全头文件——不引入任何 ETL string/format/print。
 *
 * @tparam Args 格式化参数类型包
 */
template <typename... Args>
struct KernelFmtStr {
  const char* str;

  // Templated consteval constructor that accepts any string-literal-like type.
  // Using `const _Str&` (templated) rather than `const char*` allows CTAD to
  // work correctly when used via the KernelFormatString alias below.
  // This mirrors the pattern used by std::basic_format_string.
  template <typename _Str>
  // NOLINTNEXTLINE(google-explicit-constructor)
  consteval KernelFmtStr(const _Str& s) : str(s) {  // NOLINT
    if (fmt_detail::CountFormatArgs(s) != sizeof...(Args)) {
      // Calling a non-constexpr function from consteval is a compile-time
      // error — no exceptions needed (freestanding / -fno-exceptions safe).
      klog_format_arg_count_mismatch_error();
    }
  }
};

/**
 * @brief 编译期类型安全格式字符串（对外接口）
 *
 * 使用 type_identity_t 包裹 Args，阻止编译器从格式字符串参数推导 Args，
 * 强制 Args 仅由值参数推导——与 std::format_string 的做法完全相同。
 *
 * @tparam Args 格式化参数类型包
 */
template <typename... Args>
using KernelFormatString = KernelFmtStr<std::type_identity_t<Args>...>;

// ── Header prefix emission ────────────────────────────────────────────────

/**
 * @brief 输出日志行前缀：ANSI 颜色码 + [core_id]
 *
 * 统一 LogLine 构造函数和 LogEmit 中重复的前缀发射逻辑。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
__always_inline void EmitHeader() {
  sk_emit_str(kLogColors[Level]);
  etl_putchar('[');
  sk_emit_sint(static_cast<long long>(cpu_io::GetCurrentCoreId()));
  etl_putchar(']');
}

// ── Single-argument freestanding-safe printer ────────────────────────────

/// 输出有符号整数（支持 {:x}/{:#x}/{:X}/{:#X} 说明符）
__always_inline void FormatArg(long long val, const char* sb, const char* se) {
  bool is_hex, upper, alt;
  fmt_detail::ParseHexSpec(sb, se, is_hex, upper, alt);
  if (is_hex) {
    if (alt) {
      etl_putchar('0');
      etl_putchar(upper ? 'X' : 'x');
    }
    sk_emit_hex(static_cast<unsigned long long>(val), 0, upper ? 1 : 0);
  } else {
    sk_emit_sint(val);
  }
}
__always_inline void FormatArg(long val, const char* sb, const char* se) {
  FormatArg(static_cast<long long>(val), sb, se);
}
__always_inline void FormatArg(int val, const char* sb, const char* se) {
  FormatArg(static_cast<long long>(val), sb, se);
}

/// 输出无符号整数（支持 {:x}/{:#x}/{:X}/{:#X} 说明符）
__always_inline void FormatArg(unsigned long long val, const char* sb,
                               const char* se) {
  bool is_hex, upper, alt;
  fmt_detail::ParseHexSpec(sb, se, is_hex, upper, alt);
  if (is_hex) {
    if (alt) {
      etl_putchar('0');
      etl_putchar(upper ? 'X' : 'x');
    }
    sk_emit_hex(val, 0, upper ? 1 : 0);
  } else {
    sk_emit_uint(val);
  }
}
__always_inline void FormatArg(unsigned long val, const char* sb,
                               const char* se) {
  FormatArg(static_cast<unsigned long long>(val), sb, se);
}
__always_inline void FormatArg(unsigned int val, const char* sb,
                               const char* se) {
  FormatArg(static_cast<unsigned long long>(val), sb, se);
}

/// 输出布尔值（说明符忽略）
__always_inline void FormatArg(bool val, const char* /*sb*/,
                               const char* /*se*/) {
  sk_emit_str(val ? "true" : "false");
}

/// 输出字符（说明符忽略）
__always_inline void FormatArg(char val, const char* /*sb*/,
                               const char* /*se*/) {
  etl_putchar(val);
}

/// 输出 C 字符串（说明符忽略）
__always_inline void FormatArg(const char* val, const char* /*sb*/,
                               const char* /*se*/) {
  sk_emit_str(val);
}

/// 输出指针地址（始终以 0x 前缀十六进制输出，说明符忽略）
__always_inline void FormatArg(const void* val, const char* /*sb*/,
                               const char* /*se*/) {
  etl_putchar('0');
  etl_putchar('x');
  sk_emit_hex(static_cast<unsigned long long>(
                  static_cast<uintptr_t>(reinterpret_cast<uintptr_t>(val))),
              static_cast<int>(sizeof(void*) * 2), /*upper=*/0);
}
__always_inline void FormatArg(void* val, const char* sb, const char* se) {
  FormatArg(static_cast<const void*>(val), sb, se);
}

// ── Format string walker ──────────────────────────────────────────────────

/// 输出格式字符串中直到下一个占位符之前的字面量字符，
/// 返回 FmtSpec（占位符后的指针 + 说明符范围）
__always_inline auto EmitUntilNextArg(const char* s) -> fmt_detail::FmtSpec {
  while (*s != '\0') {
    if (s[0] == '{' && s[1] == '{') {
      etl_putchar('{');
      s += 2;
      continue;
    }
    if (s[0] == '}' && s[1] == '}') {
      etl_putchar('}');
      s += 2;
      continue;
    }
    if (s[0] == '{') {
      ++s;  // skip '{'
      const char* spec_begin = s;
      while (*s != '\0' && *s != '}') {
        ++s;
      }
      const char* spec_end = s;
      if (*s == '}') {
        ++s;  // skip '}'
      }
      return {s, spec_begin, spec_end};
    }
    etl_putchar(*s);
    ++s;
  }
  return {s, nullptr, nullptr};
}

/// Base case: no remaining args — flush any trailing literal characters
__always_inline void LogFormat(const char* s) {
  while (*s != '\0') {
    if (s[0] == '{' && s[1] == '{') {
      etl_putchar('{');
      s += 2;
    } else if (s[0] == '}' && s[1] == '}') {
      etl_putchar('}');
      s += 2;
    } else {
      etl_putchar(*s);
      ++s;
    }
  }
}

/// Recursive case: emit up to next {}, format current arg, recurse for rest
template <typename T, typename... Rest>
__always_inline void LogFormat(const char* s, T&& val, Rest&&... rest) {
  fmt_detail::FmtSpec spec = EmitUntilNextArg(s);
  FormatArg(static_cast<std::decay_t<T>>(val), spec.spec_begin, spec.spec_end);
  LogFormat(spec.next, static_cast<Rest&&>(rest)...);
}

// ── LogLine: RAII stream-style log line ──────────────────────────────────

/**
 * @brief RAII 流式日志行
 *
 * 构造时获取自旋锁并输出颜色前缀+核心ID，
 * 析构时输出换行、重置码并释放锁。
 * 整条日志行在锁的保护下原子输出，不会被其他核心打断。
 *
 * @tparam Level 日志级别
 * @note 仅通过移动语义传递所有权（移动后原对象不再持有锁）
 */
template <LogLevel::enum_type Level>
class LogLine {
 public:
  LogLine() {
    log_lock.Lock().or_else([](auto&& err) {
      sk_emit_str("LogLine: Failed to acquire lock: ");
      sk_emit_str(err.message());
      etl_putchar('\n');
      while (true) {
        cpu_io::Pause();
      }
      return Expected<void>{};
    });
    EmitHeader<Level>();
  }

  LogLine(LogLine&& other) noexcept : released_(other.released_) {
    other.released_ = true;
  }

  LogLine(const LogLine&) = delete;
  auto operator=(const LogLine&) -> LogLine& = delete;
  auto operator=(LogLine&&) -> LogLine& = delete;

  ~LogLine() {
    if (!released_) {
      etl_putchar('\n');
      sk_emit_str(kReset);
      log_lock.UnLock().or_else([](auto&& err) {
        sk_emit_str("LogLine: Failed to release lock: ");
        sk_emit_str(err.message());
        etl_putchar('\n');
        while (true) {
          cpu_io::Pause();
        }
        return Expected<void>{};
      });
    }
  }

  // ── Value overloads ──────────────────────────────────────────────────

  /// 输出有符号整数类型
  template <std::signed_integral T>
  auto operator<<(T val) -> LogLine& {
    FormatArg(static_cast<long long>(val), nullptr, nullptr);
    return *this;
  }

  /// 输出无符号整数类型
  template <std::unsigned_integral T>
    requires(!std::same_as<T, bool> && !std::same_as<T, char>)
  auto operator<<(T val) -> LogLine& {
    FormatArg(static_cast<unsigned long long>(val), nullptr, nullptr);
    return *this;
  }

  /// 输出 C 字符串
  auto operator<<(const char* val) -> LogLine& {
    FormatArg(val, nullptr, nullptr);
    return *this;
  }

  /// 输出单个字符
  auto operator<<(char val) -> LogLine& {
    FormatArg(val, nullptr, nullptr);
    return *this;
  }

  /// 输出布尔值
  auto operator<<(bool val) -> LogLine& {
    FormatArg(val, nullptr, nullptr);
    return *this;
  }

  /// 输出指针地址
  auto operator<<(const void* val) -> LogLine& {
    FormatArg(val, nullptr, nullptr);
    return *this;
  }

 private:
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
  template <typename T>
  auto operator<<(T&& val) -> LogLine<Level> {
    LogLine<Level> line;
    line << static_cast<T&&>(val);
    return line;
  }
};

/**
 * @brief printf 风格日志输出（类型安全版本）
 *
 * 使用 KernelFormatString<Args...> 进行编译期格式字符串校验。
 * 格式语法：{} 用于默认类型，{:x}/{:#x} 等用于整数十六进制，
 * {{ 和 }} 为转义括号。
 * 不依赖任何 ETL string/format/print 头文件——完全 freestanding 安全。
 *
 * @tparam Level 日志级别
 * @tparam Args 格式化参数类型
 * @param location 调用位置（自动捕获）
 * @param fmt 编译期格式字符串
 * @param args 格式化参数
 */
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    [[maybe_unused]] const std::source_location& location,
    KernelFormatString<Args...> fmt, Args&&... args) {
  if constexpr (Level < kMinLogLevel) {
    return;
  }
  LockGuard<SpinLock> lock_guard(log_lock);
  EmitHeader<Level>();
  if constexpr (Level == LogLevel::kDebug) {
    etl_putchar('[');
    sk_emit_str(location.file_name());
    etl_putchar(':');
    sk_emit_uint(static_cast<unsigned long long>(location.line()));
    etl_putchar(' ');
    sk_emit_str(location.function_name());
    etl_putchar(']');
    etl_putchar(' ');
  }
  LogFormat(fmt.str, static_cast<Args&&>(args)...);
  etl_putchar('\n');
  sk_emit_str(kReset);
}

}  // namespace detail

/**
 * @brief Debug 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 * @note 使用 CTAD 推导模板参数
 */
template <typename... Args>
struct Debug {
  explicit Debug(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kDebug>(location, fmt,
                                              static_cast<Args&&>(args)...);
  }
};
template <typename... Args>
Debug(detail::KernelFormatString<Args...>, Args&&...) -> Debug<Args...>;

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
      sk_emit_hex(static_cast<unsigned long long>(
                      reinterpret_cast<const uint8_t*>(data)[i]),
                  /*width=*/2, /*upper=*/1);
      etl_putchar(' ');
    }
    sk_emit_str(detail::kReset);
    etl_putchar('\n');
  }
}

/**
 * @brief Info 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Info {
  explicit Info(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kInfo>(location, fmt,
                                             static_cast<Args&&>(args)...);
  }
};
template <typename... Args>
Info(detail::KernelFormatString<Args...>, Args&&...) -> Info<Args...>;

/**
 * @brief Warn 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Warn {
  explicit Warn(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kWarn>(location, fmt,
                                             static_cast<Args&&>(args)...);
  }
};
template <typename... Args>
Warn(detail::KernelFormatString<Args...>, Args&&...) -> Warn<Args...>;

/**
 * @brief Err 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Err {
  explicit Err(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kErr>(location, fmt,
                                            static_cast<Args&&>(args)...);
  }
};
template <typename... Args>
Err(detail::KernelFormatString<Args...>, Args&&...) -> Err<Args...>;

/// @brief 流式日志实例（使用 inline 避免跨 TU 重复定义）
/// @{
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kInfo> info;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kWarn> warn;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kDebug> debug;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kErr> err;
/// @}

/// @todo 可插拔输出 sink：使用 etl::delegate 将输出重定向到串口/内存缓冲区等
/// 需要引入缓冲机制后实现

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
