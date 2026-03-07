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

/**
 * @brief 将 ETL basic_format_arg 不支持的类型映射为受支持的类型
 *
 * ETL format 缺少 `long` 和 `unsigned long` 的构造函数（LP64 问题），
 * 此处将其分别映射为 `long long` / `unsigned long long`，其余类型保持不变。
 */
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

/**
 * @brief 非模板桥接函数，转发调用至 etl::vformat_to
 *
 * 使用 __always_inline 保证零开销内联。
 */
__always_inline auto VFormatToN(
    char* buf, size_t n, etl::string_view fmt,
    etl::format_args<etl::private_format::limit_iterator<char*>> args)
    -> char* {
  return etl::vformat_to(WrapperIt(buf, n), fmt, args).get();
}

/**
 * @brief etl::vformat_to 的安全封装，格式化前将参数转换为 ETL 兼容类型
 *
 * 解决 ETL basic_format_arg 缺少 `long` / `unsigned long` 构造函数的问题。
 *
 * @note 实际的 vformat_to 调用位于 VFormatToN；若在 -mgeneral-regs-only
 *       下需要浮点实例化，可使用独立编译单元 (klog_format.cpp)。
 */
template <typename... Args>
__always_inline auto FormatToN(char* buf, size_t n,
                               etl::format_string<Args...> fmt, Args&&... args)
    -> char* {
  // 将转换后的值物化为具名局部变量，使 make_format_args 获得左值引用
  // EtlArgMap 将 long -> long long，unsigned long -> unsigned long long
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

/// @name ANSI 转义码
/// @{
inline constexpr auto kReset = "\033[0m";
inline constexpr auto kRed = "\033[31m";
inline constexpr auto kGreen = "\033[32m";
inline constexpr auto kYellow = "\033[33m";
inline constexpr auto kCyan = "\033[36m";
/// @}

/// 日志级别
enum Level : uint8_t {
  kDebug = 0,
  kInfo = 1,
  kWarn = 2,
  kErr = 3,
};

/// 编译期最低日志级别
inline constexpr auto kMinLevel =
    static_cast<Level>(SIMPLEKERNEL_MIN_LOG_LEVEL);

/// 日志级别标签（定宽，对齐输出）
inline constexpr const char* kLevelLabel[] = {
    "[DEBUG] ",
    "[INFO ] ",
    "[WARN ] ",
    "[ERROR] ",
};

/// 日志级别对应的颜色（按 Level 枚举索引）
inline constexpr const char* kLevelColor[] = {
    // 调试
    kGreen,
    // 信息
    kCyan,
    // 警告
    kYellow,
    // 错误
    kRed,
};

/// 存储于 MPMC 队列中的日志条目
struct LogEntry {
  uint64_t seq;
  uint64_t core_id;
  Level level;
  char msg[239];
};
static_assert(sizeof(LogEntry) == 256, "LogEntry must be 256 bytes");

/// 全局日志队列（64 KB 静态内存，constexpr 构造）
inline mpmc_queue::MPMCQueue<LogEntry, 256> log_queue;

/// 用于跨核心排序的单调递增序列计数器
inline std::atomic<uint64_t> log_seq{0};

/// 单消费者排空保护（非阻塞 try-lock）
inline std::atomic_flag drain_flag = ATOMIC_FLAG_INIT;

/// 队列满时丢弃的条目计数
inline std::atomic<uint64_t> dropped_count{0};

/// 通过 etl_putchar 输出字符串（空指针安全）
__always_inline void PutStr(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar(static_cast<unsigned char>(*s));
    ++s;
  }
}

/**
 * @brief 将队列中所有条目输出至串口
 *
 * 使用 atomic_flag 实现非阻塞 try-lock，同一时刻仅一个核心执行排空，
 * 其他核心直接返回，等待下次调用时再排空。
 */
inline void TryDrain() {
  // 非阻塞 try-lock：若其他核心正在排空则立即返回
  if (drain_flag.test_and_set(std::memory_order_acquire)) {
    return;
  }

  // 若有丢弃条目则上报
  auto dropped = dropped_count.exchange(0, std::memory_order_relaxed);
  if (dropped > 0) {
    char drop_buf[64];
    auto* end = FormatToN(drop_buf, sizeof(drop_buf) - 1,
                          "\033[31m[LOG] dropped {} entries\033[0m\n",
                          static_cast<uint64_t>(dropped));
    *end = '\0';
    PutStr(drop_buf);
  }

  // 排空循环
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

/**
 * @brief 核心实现：格式化消息并入队，随后尝试排空
 *
 * @tparam Lvl 编译期日志级别，低于 kMinLevel 时整个函数被编译器消除
 * @tparam Args 可变格式化参数类型
 */
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
    // 队列满：尝试排空后重试一次
    TryDrain();
    if (!log_queue.push(entry)) {
      dropped_count.fetch_add(1, std::memory_order_relaxed);
      return;
    }
  }

  TryDrain();
}

}  // namespace detail

/// 以 DEBUG 级别记录日志（SIMPLEKERNEL_MIN_LOG_LEVEL > 0 时编译期消除）
template <typename... Args>
inline void Debug(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kDebug < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kDebug>(fmt, static_cast<Args&&>(args)...);
}

/// 以 INFO 级别记录日志
template <typename... Args>
inline void Info(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kInfo < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kInfo>(fmt, static_cast<Args&&>(args)...);
}

/// 以 WARN 级别记录日志
template <typename... Args>
inline void Warn(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kWarn < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kWarn>(fmt, static_cast<Args&&>(args)...);
}

/// 以 ERROR 级别记录日志
template <typename... Args>
inline void Err(etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (detail::Level::kErr < detail::kMinLevel) {
    return;
  }
  detail::Log<detail::Level::kErr>(fmt, static_cast<Args&&>(args)...);
}

/// 强制将队列中所有日志条目输出至串口
__always_inline void Flush() { detail::TryDrain(); }

/// 绕过队列直接输出至串口（用于 panic 路径）
__always_inline void RawPut(const char* msg) { detail::PutStr(msg); }

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
