/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <etl/enum_type.h>

#include <array>
#include <concepts>
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

/// 每种日志级别对应的 ANSI 颜色
constexpr std::array<const char*, LogLevel::kMax> kLogColors = {
    // kDebug
    kMagenta,
    // kInfo
    kCyan,
    // kWarn
    kYellow,
    // kErr
    kRed,
};

/**
 * @brief RAII 流式日志行
 *
 * 构造时获取自旋锁并输出颜色前缀+核心ID，
 * 析构时输出重置码并释放锁。
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
      sk_printf("LogLine: Failed to acquire lock: %s\n", err.message());
      while (true) {
        cpu_io::Pause();
      }
      return Expected<void>{};
    });
    sk_printf("%s[%ld]", kLogColors[Level], cpu_io::GetCurrentCoreId());
  }

  LogLine(LogLine&& other) noexcept : released_(other.released_) {
    other.released_ = true;
  }

  LogLine(const LogLine&) = delete;
  auto operator=(const LogLine&) -> LogLine& = delete;
  auto operator=(LogLine&&) -> LogLine& = delete;

  ~LogLine() {
    if (!released_) {
      sk_printf("%s", kReset);
      log_lock.UnLock().or_else([](auto&& err) {
        sk_printf("LogLine: Failed to release lock: %s\n", err.message());
        while (true) {
          cpu_io::Pause();
        }
        return Expected<void>{};
      });
    }
  }

  /// 输出有符号整数类型
  template <std::signed_integral T>
  auto operator<<(T val) -> LogLine& {
    if constexpr (sizeof(T) <= 4) {
      sk_printf("%d", static_cast<int32_t>(val));
    } else {
      sk_printf("%ld", static_cast<int64_t>(val));
    }
    return *this;
  }

  /// 输出无符号整数类型
  template <std::unsigned_integral T>
    requires(!std::same_as<T, bool> && !std::same_as<T, char>)
  auto operator<<(T val) -> LogLine& {
    if constexpr (sizeof(T) <= 4) {
      sk_printf("%u", static_cast<uint32_t>(val));
    } else {
      sk_printf("%lu", static_cast<uint64_t>(val));
    }
    return *this;
  }

  /// 输出 C 字符串
  auto operator<<(const char* val) -> LogLine& {
    sk_printf("%s", val);
    return *this;
  }

  /// 输出单个字符
  auto operator<<(char val) -> LogLine& {
    sk_printf("%c", val);
    return *this;
  }

  /// 输出布尔值
  auto operator<<(bool val) -> LogLine& {
    sk_printf("%s", val ? "true" : "false");
    return *this;
  }

  /// 输出指针地址
  auto operator<<(const void* val) -> LogLine& {
    sk_printf("%p", val);
    return *this;
  }

 private:
  bool released_ = false;
};

/**
 * @brief 流式日志入口
 *
 * 全局实例（如 klog::info），第一次 operator<< 创建 LogLine 临时对象，
 * 后续 << 链式调用在同一 LogLine 上进行，
 * 语句结束时 LogLine 析构释放锁。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
struct LogStarter {
  /// @brief 通过 << 创建 LogLine 并输出第一个值
  template <typename T>
  auto operator<<(T&& val) -> LogLine<Level> {
    LogLine<Level> line;
    line << std::forward<T>(val);
    return line;
  }
};

/**
 * @brief printf 风格日志输出
 *
 * 将 fmt 参数与可变参数分离，消除 -Wformat-security 警告。
 * 当无额外参数时使用 %s 包装 fmt。
 *
 * @tparam Level 日志级别
 * @tparam Args 格式化参数类型
 * @param fmt 格式化字符串
 * @param location 调用位置（自动捕获）
 * @param args 格式化参数
 */
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    [[maybe_unused]] const std::source_location& location, Args&&... args) {
  if constexpr (Level < kMinLogLevel) {
    return;
  }
  constexpr auto* color = kLogColors[Level];
  LockGuard<SpinLock> lock_guard(log_lock);
  sk_printf("%s[%ld]", color, cpu_io::GetCurrentCoreId());
  if constexpr (Level == LogLevel::kDebug) {
    sk_printf("[%s:%d %s] ", location.file_name(), location.line(),
              location.function_name());
  }
  if constexpr (sizeof...(Args) == 0) {
    // no-op: nothing to print
  } else if constexpr (sizeof...(Args) == 1) {
    // 单参数：视为纯字符串，使用 %s 包装消除 -Wformat-security
    sk_printf("%s", std::forward<Args>(args)...);
  } else {
    // 多参数：第一个参数为格式字符串
    sk_printf(std::forward<Args>(args)...);
  }
  sk_printf("%s", kReset);
}

}  // namespace detail

/**
 * @brief Debug 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 * @note 使用 CTAD 推导模板参数
 */
template <typename... Args>
struct Debug {
  explicit Debug(Args&&... args, const std::source_location& location =
                                     std::source_location::current()) {
    if constexpr (detail::LogLevel::kDebug >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kDebug>(location,
                                                std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Debug(Args&&...) -> Debug<Args...>;

/**
 * @brief 以十六进制输出内存块内容（仅 Debug 构建）
 * @param data 数据指针
 * @param size 数据大小（字节）
 */
__always_inline void DebugBlob([[maybe_unused]] const void* data,
                               [[maybe_unused]] size_t size) {
#ifdef SIMPLEKERNEL_DEBUG
  if constexpr (detail::LogLevel::kDebug >= detail::kMinLogLevel) {
    LockGuard<SpinLock> lock_guard(klog::detail::log_lock);
    sk_printf("%s[%ld] ", detail::kMagenta, cpu_io::GetCurrentCoreId());
    for (size_t i = 0; i < size; i++) {
      sk_printf("0x%02X ", reinterpret_cast<const uint8_t*>(data)[i]);
    }
    sk_printf("%s\n", detail::kReset);
  }
#endif
}

/**
 * @brief Info 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Info {
  explicit Info(Args&&... args, const std::source_location& location =
                                    std::source_location::current()) {
    if constexpr (detail::LogLevel::kInfo >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kInfo>(location,
                                               std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Info(Args&&...) -> Info<Args...>;

/**
 * @brief Warn 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Warn {
  explicit Warn(Args&&... args, const std::source_location& location =
                                    std::source_location::current()) {
    if constexpr (detail::LogLevel::kWarn >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kWarn>(location,
                                               std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Warn(Args&&...) -> Warn<Args...>;

/**
 * @brief Err 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Err {
  explicit Err(Args&&... args, const std::source_location& location =
                                   std::source_location::current()) {
    if constexpr (detail::LogLevel::kErr >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kErr>(location,
                                              std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Err(Args&&...) -> Err<Args...>;

/// @brief 流式日志实例（使用 inline 避免跨 TU 重复定义）
/// @{
[[maybe_unused]] inline detail::LogStarter<detail::LogLevel::kInfo> info;
[[maybe_unused]] inline detail::LogStarter<detail::LogLevel::kWarn> warn;
[[maybe_unused]] inline detail::LogStarter<detail::LogLevel::kDebug> debug;
[[maybe_unused]] inline detail::LogStarter<detail::LogLevel::kErr> err;
/// @}

/// @todo 可插拔输出 sink：使用 etl::delegate 将输出重定向到串口/内存缓冲区等
/// 需要引入缓冲机制后实现

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
