/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <array>
#include <cstdarg>
#include <cstdint>
#include <source_location>

#include "kstd_cstdio"
#include "kstd_iostream"
#include "singleton.hpp"
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

enum LogLevel {
  kDebug,
  kInfo,
  kWarn,
  kErr,
  kLogLevelMax,
};

constexpr std::array<const char*, kLogLevelMax> kLogColors = {
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
 * @brief 流式日志输出器
 * @tparam Level 日志级别
 * @note 使用 LogLevel 枚举作为模板参数，避免模板模板参数的复杂性
 */
template <LogLevel Level>
class Logger : public kstd::ostream {
 public:
  auto operator<<(int8_t val) -> Logger& override {
    output("%d", val);
    return *this;
  }

  auto operator<<(uint8_t val) -> Logger& override {
    output("%u", static_cast<unsigned>(val));
    return *this;
  }

  auto operator<<(const char* val) -> Logger& override {
    output("%s", val);
    return *this;
  }

  auto operator<<(int16_t val) -> Logger& override {
    output("%d", val);
    return *this;
  }

  auto operator<<(uint16_t val) -> Logger& override {
    output("%u", static_cast<unsigned>(val));
    return *this;
  }

  auto operator<<(int32_t val) -> Logger& override {
    output("%d", val);
    return *this;
  }

  auto operator<<(uint32_t val) -> Logger& override {
    output("%u", val);
    return *this;
  }

  auto operator<<(int64_t val) -> Logger& override {
    output("%ld", val);
    return *this;
  }

  auto operator<<(uint64_t val) -> Logger& override {
    output("%lu", val);
    return *this;
  }

 private:
  /**
   * @brief 输出格式化内容
   * @tparam Args 可变参数类型
   * @param fmt 格式化字符串
   * @param args 格式化参数
   */
  template <typename... Args>
  void output(const char* fmt, Args&&... args) {
    constexpr auto* color = kLogColors[Level];
    LockGuard<SpinLock> lock_guard(log_lock);
    sk_printf("%s[%ld]", color, cpu_io::GetCurrentCoreId());
    sk_printf(fmt, std::forward<Args>(args)...);
    sk_printf("%s", kReset);
  }
};

template <LogLevel Level, typename... Args>
struct LogBase {
  explicit LogBase(Args&&... args,
                   [[maybe_unused]] const std::source_location& location =
                       std::source_location::current()) {
    constexpr auto* color = kLogColors[Level];
    LockGuard<SpinLock> lock_guard(log_lock);
    sk_printf("%s[%ld]", color, cpu_io::GetCurrentCoreId());
    if constexpr (Level == kDebug) {
      sk_printf("[%s] ", location.function_name());
    }
/// @todo 解决警告
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wformat-security"
    sk_printf(args...);
#pragma GCC diagnostic pop
    sk_printf("%s", detail::kReset);
  }
};

}  // namespace detail

template <typename... Args>
struct Debug : public detail::LogBase<detail::kDebug, Args...> {
  explicit Debug(Args&&... args, const std::source_location& location =
                                     std::source_location::current())
      : detail::LogBase<detail::kDebug, Args...>(std::forward<Args>(args)...,
                                                 location) {}
};
template <typename... Args>
Debug(Args&&...) -> Debug<Args...>;

__always_inline void DebugBlob([[maybe_unused]] const void* data,
                               [[maybe_unused]] size_t size) {
#ifdef SIMPLEKERNEL_DEBUG
  LockGuard<SpinLock> lock_guard(klog::detail::log_lock);
  sk_printf("%s[%ld] ", detail::kMagenta, cpu_io::GetCurrentCoreId());
  for (size_t i = 0; i < size; i++) {
    sk_printf("0x%02X ", reinterpret_cast<const uint8_t*>(data)[i]);
  }
  sk_printf("%s\n", detail::kReset);
#endif
}

template <typename... Args>
struct Info : public detail::LogBase<detail::kInfo, Args...> {
  explicit Info(Args&&... args, const std::source_location& location =
                                    std::source_location::current())
      : detail::LogBase<detail::kInfo, Args...>(std::forward<Args>(args)...,
                                                location) {}
};
template <typename... Args>
Info(Args&&...) -> Info<Args...>;

template <typename... Args>
struct Warn : public detail::LogBase<detail::kWarn, Args...> {
  explicit Warn(Args&&... args, const std::source_location& location =
                                    std::source_location::current())
      : detail::LogBase<detail::kWarn, Args...>(std::forward<Args>(args)...,
                                                location) {}
};
template <typename... Args>
Warn(Args&&...) -> Warn<Args...>;

template <typename... Args>
struct Err : public detail::LogBase<detail::kErr, Args...> {
  explicit Err(Args&&... args, const std::source_location& location =
                                   std::source_location::current())
      : detail::LogBase<detail::kErr, Args...>(std::forward<Args>(args)...,
                                               location) {}
};
template <typename... Args>
Err(Args&&...) -> Err<Args...>;

[[maybe_unused]] static detail::Logger<detail::kInfo> info;
[[maybe_unused]] static detail::Logger<detail::kWarn> warn;
[[maybe_unused]] static detail::Logger<detail::kDebug> debug;
[[maybe_unused]] static detail::Logger<detail::kErr> err;

}  // namespace klog

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_ */
