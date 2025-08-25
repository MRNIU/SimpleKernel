/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief 内核日志相关函数
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <array>
#include <cstdarg>
#include <cstdint>
#include <source_location>

#include "config.h"
#include "singleton.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "spinlock.hpp"

namespace klog {
namespace detail {

// 日志专用的自旋锁实例
static SpinLock log_lock("kernel_log");

/// ANSI 转义码，在支持 ANSI 转义码的终端中可以显示颜色
static constexpr const auto kReset = "\033[0m";
static constexpr const auto kRed = "\033[31m";
static constexpr const auto kGreen = "\033[32m";
static constexpr const auto kYellow = "\033[33m";
static constexpr const auto kBlue = "\033[34m";
static constexpr const auto kMagenta = "\033[35m";
static constexpr const auto kCyan = "\033[36m";
static constexpr const auto kWhite = "\033[37m";

template <template <typename...> class OutputFunction>
class Logger : public sk_std::ostream {
 public:
  auto operator<<(int8_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(uint8_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(const char* val) -> Logger& override {
    OutputFunction("%s", val);
    return *this;
  }

  auto operator<<(int16_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(uint16_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(int32_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(uint32_t val) -> Logger& override {
    OutputFunction("%d", val);
    return *this;
  }

  auto operator<<(int64_t val) -> Logger& override {
    OutputFunction("%ld", val);
    return *this;
  }

  auto operator<<(uint64_t val) -> Logger& override {
    OutputFunction("%ld", val);
    return *this;
  }
};

enum LogLevel {
  kDebug,
  kInfo,
  kWarn,
  kErr,
  kLogLevelMax,
};

constexpr std::array<const char*, kLogLevelMax> kLogColors = {
    // kDebug
    detail::kMagenta,
    // kInfo
    detail::kCyan,
    // kWarn
    detail::kYellow,
    // kErr
    detail::kRed,
};

template <LogLevel Level, typename... Args>
struct LogBase {
  explicit LogBase(Args&&... args,
                   [[maybe_unused]] const std::source_location& location =
                       std::source_location::current()) {
    constexpr auto* color = kLogColors[Level];
    LockGuard<SpinLock> lock_guard(log_lock);
    sk_printf("%s[%ld]", color, cpu_io::GetCurrentCoreId());
    if constexpr (Level == kDebug && kSimpleKernelDebugLog) {
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

__always_inline void DebugBlob(const void* data, size_t size) {
  if constexpr (kSimpleKernelDebugLog) {
    LockGuard<SpinLock> lock_guard(klog::detail::log_lock);
    sk_printf("%s[%ld] ", detail::kMagenta, cpu_io::GetCurrentCoreId());
    for (size_t i = 0; i < size; i++) {
      sk_printf("0x%02X ", reinterpret_cast<const uint8_t*>(data)[i]);
    }
    sk_printf("%s\n", detail::kReset);
  }
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

[[maybe_unused]] static detail::Logger<Info> info;
[[maybe_unused]] static detail::Logger<Warn> warn;
[[maybe_unused]] static detail::Logger<Debug> debug;
[[maybe_unused]] static detail::Logger<Err> err;

}  // namespace klog

#if defined(UNIT_TEST)
#define INFO(...)
#define WARN(...)
#define ERR(...)
#define DEBUG(...)
#define DEBUG_BLOB(data, size)
#else
#define INFO(...) klog::Info(__VA_ARGS__, std::source_location::current())
#define WARN(...) klog::Warn(__VA_ARGS__, std::source_location::current())
#define ERR(...) klog::Err(__VA_ARGS__, std::source_location::current())
#define DEBUG(...) klog::Debug(__VA_ARGS__, std::source_location::current())
#define DEBUG_BLOB(data, size) klog::DebugBlob(data, size)
#endif

#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_ */
