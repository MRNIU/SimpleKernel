
/**
 * @file kernel_log.hpp
 * @brief 内核日志相关函数
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-07-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleKernel
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-07-15<td>Zone.N (Zone.Niuzh@hotmail.com)<td>创建文件
 * </table>
 */

#ifndef SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_LOG_HPP_

#include <cstdarg>
#include <cstdint>

#include "../../project_config.h"
#include "singleton.hpp"
#include "sk_cstdio"
#include "sk_iostream"
#include "spinlock.hpp"

namespace klog {
namespace logger {

/// ANSI 转义码，在支持 ANSI 转义码的终端中可以显示颜色
constexpr const auto kReset = "\033[0m";
constexpr const auto kRed = "\033[31m";
constexpr const auto kGreen = "\033[32m";
constexpr const auto kYellow = "\033[33m";
constexpr const auto kBlue = "\033[34m";
constexpr const auto kMagenta = "\033[35m";
constexpr const auto kCyan = "\033[36m";
constexpr const auto kWhite = "\033[37m";

template <void (*OutputFunction)(const char* format, ...)>
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

}  // namespace logger

/**
 * @brief 与 printf 类似，只是颜色不同
 */
extern "C" inline void Debug(const char* format, ...) {
  (void)format;
#ifdef SIMPLEKERNEL_DEBUG_LOG
  Singleton<SpinLock>::GetInstance().lock();
  va_list args;
  va_start(args, format);
  printf("%s[%ld] ", logger::kMagenta, cpu_io::GetCurrentCoreId());
  vprintf(format, args);
  printf("%s", logger::kReset);
  va_end(args);
  Singleton<SpinLock>::GetInstance().unlock();
#endif
}

extern "C" inline void Info(const char* format, ...) {
  Singleton<SpinLock>::GetInstance().lock();
  va_list args;
  va_start(args, format);
  printf("%s[%ld] ", logger::kCyan, cpu_io::GetCurrentCoreId());
  vprintf(format, args);
  printf("%s", logger::kReset);
  va_end(args);
  Singleton<SpinLock>::GetInstance().unlock();
}

extern "C" inline void Warn(const char* format, ...) {
  Singleton<SpinLock>::GetInstance().lock();
  va_list args;
  va_start(args, format);
  printf("%s[%ld] ", logger::kYellow, cpu_io::GetCurrentCoreId());
  vprintf(format, args);
  printf("%s", logger::kReset);
  va_end(args);
  Singleton<SpinLock>::GetInstance().unlock();
}

extern "C" inline void Err(const char* format, ...) {
  Singleton<SpinLock>::GetInstance().lock();
  va_list args;
  va_start(args, format);
  printf("%s[%ld] ", logger::kRed, cpu_io::GetCurrentCoreId());
  vprintf(format, args);
  printf("%s", logger::kReset);
  va_end(args);
  Singleton<SpinLock>::GetInstance().unlock();
}

[[maybe_unused]] static logger::Logger<Info> info;
[[maybe_unused]] static logger::Logger<Warn> warn;
[[maybe_unused]] static logger::Logger<Debug> debug;
[[maybe_unused]] static logger::Logger<Err> err;

}  // namespace klog

#endif /* SIMPLEKERNEL_SRC_KERNEL_INCLUDE_KERNEL_LOG_HPP_ */
