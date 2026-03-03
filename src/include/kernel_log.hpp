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

/// 底层字符串输出原语（逐字符调用 etl_putchar），NULL 安全
__always_inline void PutStr(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar(static_cast<unsigned char>(*s));
    ++s;
  }
}

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

/**
 * @brief 仅供 DebugBlob 使用：直接向终端输出日志行前缀（ANSI 颜色码 +
 *        [core_id]），不经过 LogLine 缓冲区。
 *        LogLine 自行在构造函数中以流式方式写入缓冲区前缀。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
__always_inline void EmitHeader() {
  PutStr(kLogColors[Level]);
  etl_putchar('[');
  char buf_core[8];
  stbsp_snprintf(buf_core, (int)sizeof(buf_core), "%lld",
                 static_cast<long long>(cpu_io::GetCurrentCoreId()));
  PutStr(buf_core);
  etl_putchar(']');
}

/// No-op tag for disabled log levels
struct NoOpTag {};

/**
 * @brief RAII 流式日志行（基于 etl::string 实例缓冲）
 *
 * 构造时获取自旋锁并向 etl::string<512> 写入颜色前缀+核心ID，
 * 析构时通过 PutStr 原子输出完整日志行并释放锁。
 *
 * @tparam Level 日志级别
 * @note 同一时刻只能存活一个 LogLine（由 log_lock 保护）。
 * @note 缓冲区容量为 512 字节，超出部分将被 etl::string_stream 静默截断。
 */
template <LogLevel::enum_type Level>
class LogLine {
 public:
  /// 构造空操作 LogLine（用于被禁用的日志级别）
  explicit LogLine(NoOpTag) : released_(true) {}

  /**
   * @brief 构造活跃 LogLine，获取锁并输出前缀
   * @param loc 调用处的源码位置（仅 kDebug 级别捕获）
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
      PutStr(s_buf_.c_str());
      ReleaseLock();
    }
  }

  /// 向流写入 etl::string_stream 支持的任意类型
  template <typename T>
  auto operator<<(T&& val) -> LogLine& {
    if (!released_) {
      stream_ << static_cast<T&&>(val);
    }
    return *this;
  }

  /// 将 bool 以 "true"/"false" 形式写入流
  auto operator<<(bool val) -> LogLine& {
    if (!released_) {
      stream_ << (val ? "true" : "false");
    }
    return *this;
  }

  /// 直接写入单个字符（绕过 ETL 整数格式化）
  auto operator<<(char c) -> LogLine& {
    if (!released_) {
      s_buf_.push_back(c);
    }
    return *this;
  }

  /// 将指针以 "0x<hex>" 形式写入流
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
  /// 获取 log_lock；失败时自旋停机（如递归加锁）。
  static void AcquireLock() {
    log_lock.Lock().or_else([](auto&& err) -> Expected<void> {
      PutStr("LogLine: Failed to acquire lock: ");
      PutStr(err.message());
      etl_putchar('\n');
      while (true) {
        cpu_io::Pause();
      }
      __builtin_unreachable();
    });
  }

  /// 释放 log_lock；失败时自旋停机（如未持有锁）。
  static void ReleaseLock() {
    log_lock.UnLock().or_else([](auto&& err) -> Expected<void> {
      PutStr("LogLine: Failed to release lock: ");
      PutStr(err.message());
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

/// per-cpu 紧急日志缓冲区，用于无锁日志输出
inline etl::string<512> raw_log_buf_[SIMPLEKERNEL_MAX_CORE_COUNT]{};

/**
 * @brief Lock-free RAII stream-style log line (per-CPU buffer)
 *
 * 与 LogLine<Level> 类似，但以每 CPU 缓冲区索引替代 log_lock。
 * 构造时获取 CPU ID、清空对应核心的缓冲区并写入 ANSI 颜色前缀。
 * 析构时对每 CPU 缓冲区调用 PutStr()，无需持有锁。
 *
 * @tparam Level 日志级别
 *
 * @note 即使 log_lock 被持有，也可在中断/异常上下文中安全使用。
 * @note 同一 CPU 上的嵌套 LogLineRaw（可重入中断）将覆盖同一缓冲区，
 *       先前行的内容将丢失。实际上，panic/异常期间中断已关闭，此情况极少发生。
 * @note 缓冲区容量为 512 字节，超出部分将被 etl::string_stream 静默截断。
 */
template <LogLevel::enum_type Level>
class LogLineRaw {
 public:
  /// 构造空操作 LogLineRaw（用于被禁用的日志级别）
  explicit LogLineRaw(NoOpTag)
      : cpu_id_(0), stream_(raw_log_buf_[0]), released_(true) {}

  /**
   * @brief 构造活跃 LogLineRaw，将前缀写入每 CPU 缓冲区。
   * @param loc 调用处的源码位置（仅 kDebug 级别捕获）
   */
  explicit LogLineRaw(
      const std::source_location& loc = std::source_location::current())
      : cpu_id_(cpu_io::GetCurrentCoreId()),
        stream_(raw_log_buf_[cpu_id_]),
        released_(false) {
    raw_log_buf_[cpu_id_].clear();
    stream_ << kLogColors[Level] << "[" << cpu_id_ << "]";
    if constexpr (Level == LogLevel::kDebug) {
      stream_ << "[" << loc.file_name() << ":" << loc.line() << " "
              << loc.function_name() << "] ";
    }
  }

  /// 移动构造函数：转移 cpu_id_，重新绑定 stream_ 到同一缓冲区。
  LogLineRaw(LogLineRaw&& other) noexcept
      : cpu_id_(other.cpu_id_),
        stream_(raw_log_buf_[cpu_id_]),
        released_(other.released_) {
    other.released_ = true;
  }

  LogLineRaw(const LogLineRaw&) = delete;
  auto operator=(const LogLineRaw&) -> LogLineRaw& = delete;
  auto operator=(LogLineRaw&&) -> LogLineRaw& = delete;

  ~LogLineRaw() {
    if (!released_) {
      stream_ << "\n" << kReset;
      PutStr(raw_log_buf_[cpu_id_].c_str());
    }
  }

  /// 向流写入 etl::string_stream 支持的任意类型
  template <typename T>
  auto operator<<(T&& val) -> LogLineRaw& {
    if (!released_) {
      stream_ << static_cast<T&&>(val);
    }
    return *this;
  }

  /// 将 bool 以 "true"/"false" 形式写入流
  auto operator<<(bool val) -> LogLineRaw& {
    if (!released_) {
      stream_ << (val ? "true" : "false");
    }
    return *this;
  }

  /// 直接写入单个字符
  auto operator<<(char c) -> LogLineRaw& {
    if (!released_) {
      raw_log_buf_[cpu_id_].push_back(c);
    }
    return *this;
  }

  /// 将指针以 "0x<hex>" 形式写入流
  auto operator<<(const void* val) -> LogLineRaw& {
    if (!released_) {
      stream_ << "0x";
      etl::format_spec spec{};
      spec.hex();
      stream_ << spec << reinterpret_cast<uintptr_t>(val);
    }
    return *this;
  }

  auto operator<<(void* val) -> LogLineRaw& {
    return operator<<(static_cast<const void*>(val));
  }

 private:
  // 必须在 stream_ 之前声明（初始化顺序）
  uint64_t cpu_id_;
  // 引用 raw_log_buf_[cpu_id_]
  etl::string_stream stream_;
  bool released_ = false;
};

/**
 * @brief 无锁惰性流式日志代理（LogStreamRaw）
 *
 * 全局实例（如 klog::raw_info 等）。第一次 operator<< 创建 LogLineRaw
 * 临时对象， 后续 << 链式调用在同一临时对象上进行， 语句结束时 LogLineRaw
 * 析构并刷新每 CPU 缓冲区。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
struct LogStreamRaw {
  /// 创建 LogLineRaw 并输出第一个值
  template <typename T>
  auto operator<<(T&& val) -> LogLineRaw<Level> {
    if constexpr (Level < kMinLogLevel) {
      return LogLineRaw<Level>{NoOpTag{}};
    }
    LogLineRaw<Level> line{};
    line << static_cast<T&&>(val);
    return line;
  }
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
  /**
   * @brief 通过 << 创建 LogLine 并输出第一个值
   * @param val  第一个要输出的值
   * @note source_location 仅在 klog::debug() 函数调用时捕获；
   *       info/warn/err 的流式 << 接口不支持捕获调用方位置，
   *       因为 C++ 不允许在成员函数模板的 operator<< 上使用
   *       std::source_location 默认参数。
   */
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
      char buf_hex[5];
      stbsp_snprintf(
          buf_hex, (int)sizeof(buf_hex), "%02X",
          static_cast<unsigned int>(reinterpret_cast<const uint8_t*>(data)[i]));
      PutStr(buf_hex);
      etl_putchar(' ');
    }
    PutStr(detail::kReset);
    etl_putchar('\n');
  }
}

/**
 * @brief 创建 Debug 级别的 LogLine，并在调用处捕获源码位置
 *
 * 用法：`klog::debug() << "msg " << val;`
 * @param loc 自动捕获的源码位置
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

/**
 * @brief 创建无锁 Debug 级别的 LogLineRaw，并捕获源码位置。
 *
 * 用法：`klog::raw_debug() << "msg " << val;`
 * @param loc 自动捕获的源码位置
 * @note 可在中断/异常上下文中安全使用（不获取锁）。
 */
[[nodiscard]] inline auto raw_debug(
    std::source_location loc = std::source_location::current())
    -> detail::LogLineRaw<detail::LogLevel::kDebug> {
  if constexpr (detail::LogLevel::kDebug < detail::kMinLogLevel) {
    return detail::LogLineRaw<detail::LogLevel::kDebug>{detail::NoOpTag{}};
  }
  return detail::LogLineRaw<detail::LogLevel::kDebug>{loc};
}

/// 流式整数格式化用的十六进制格式规格（小写 0x 前缀）
inline const etl::format_spec hex = etl::format_spec{}.hex().show_base(true);

/// 流式整数格式化用的十六进制格式规格（大写 0X 前缀）
inline const etl::format_spec HEX =
    etl::format_spec{}.hex().upper_case(true).show_base(true);

/// 流式日志实例（使用 inline 避免跨 TU 重复定义）
/// @{
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kInfo> info;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kWarn> warn;
[[maybe_unused]] inline detail::LogStream<detail::LogLevel::kErr> err;
/// @}

/// 无锁流式日志实例 — 可在中断/异常上下文中安全使用
/// @{
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kInfo> raw_info;
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kWarn> raw_warn;
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kErr> raw_err;
/// @}

/// @todo 可插拔输出 sink：使用 etl::delegate 将输出重定向到串口/内存缓冲区等
/// 需要引入缓冲机制后实现

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
