# kernel_log.hpp ETL Rewrite — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers/executing-plans to implement this plan task-by-task.

**Goal:** Replace printf-style `sk_printf` in `kernel_log.hpp` with ETL's type-safe `etl::print` + `etl::format_string<Args...>`, and migrate all 289 call sites from `%s/%d` syntax to `{}` format syntax.

**Architecture:** Three-phase change — (1) add `etl_putchar` shim in each arch's `early_console.cpp` so `etl::print` has a physical output path; (2) rewrite `kernel_log.hpp` to use `etl::format_string<Args...>` for compile-time format validation and `etl::to_string` for the stream path; (3) batch-migrate all 289 call sites across 58 files.

**Tech Stack:** C++23 freestanding, ETL (`etl/format.h`, `etl/print.h`, `etl/to_string.h`, `etl/format_spec.h`, `etl/string.h`), GoogleTest for unit tests, CMake presets.

---

## Task 1: Add `etl_putchar` shim — riscv64

**Files:**
- Modify: `src/arch/riscv64/early_console.cpp`

### Step 1: Add `etl_putchar` at the end of the anonymous namespace

After the closing `}  // namespace` line in `src/arch/riscv64/early_console.cpp`, add:

```cpp
extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

The full file should become:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

#include "kstd_cstdio"

namespace {

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  sbi_debug_console_write_byte(c);
}

struct EarlyConsole {
  EarlyConsole() { sk_putchar = console_putchar; }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

### Step 2: Build to verify no compile error

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
```

Expected: Build succeeds (no new errors from riscv64 early_console.cpp).

### Step 3: Commit

```bash
git add src/arch/riscv64/early_console.cpp
git commit --signoff -m "feat(arch/riscv64): add etl_putchar shim for ETL print support"
```

---

## Task 2: Add `etl_putchar` shim — aarch64

**Files:**
- Modify: `src/arch/aarch64/early_console.cpp`

### Step 1: Add `etl_putchar` after the anonymous namespace

```cpp
extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

The full file should become:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kstd_cstdio"
#include "pl011/pl011_driver.hpp"
#include "pl011_singleton.h"

namespace {

pl011::Pl011Device* pl011_uart = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (pl011_uart) {
    pl011_uart->PutChar(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011_uart = &Pl011Singleton::instance();
    sk_putchar = console_putchar;  // Pl011 ctor already initializes HW
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

### Step 2: Commit

```bash
git add src/arch/aarch64/early_console.cpp
git commit --signoff -m "feat(arch/aarch64): add etl_putchar shim for ETL print support"
```

---

## Task 3: Add `etl_putchar` shim — x86_64

**Files:**
- Modify: `src/arch/x86_64/early_console.cpp`

### Step 1: Add `etl_putchar` after the anonymous namespace

```cpp
extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

The full file should become:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <etl/singleton.h>

#include "kstd_cstdio"

using SerialSingleton = etl::singleton<cpu_io::Serial>;

namespace {

cpu_io::Serial* serial = nullptr;

void console_putchar(int c, [[maybe_unused]] void* ctx) {
  if (serial) {
    serial->Write(c);
  }
}

struct EarlyConsole {
  EarlyConsole() {
    SerialSingleton::create(cpu_io::kCom1);
    serial = &SerialSingleton::instance();

    sk_putchar = console_putchar;
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) { sk_putchar(c, nullptr); }
```

### Step 2: Commit

```bash
git add src/arch/x86_64/early_console.cpp
git commit --signoff -m "feat(arch/x86_64): add etl_putchar shim for ETL print support"
```

---

## Task 4: Rewrite `kernel_log.hpp`

**Files:**
- Modify: `src/include/kernel_log.hpp`

### Step 1: Replace the file with the new implementation

> **Key design decisions:**
> - `LogEmit` takes `etl::format_string<Args...>` as first param after `location` — compile-time format check
> - `LogLine` gains `etl::format_spec spec_` member + operator<< overloads for ETL manipulators
> - Integer/string `operator<<` uses `etl::to_string(val, buf, spec_)` then loops over `buf` calling `sk_putchar`
> - CTAD deduction guides updated to match `etl::format_string<Args...>` first param
> - `DebugBlob` retains its internal `sk_printf` (separate concern, not migrated)
> - Zero-arg `LogEmit` overload is removed — `klog::Debug()` with no args no longer compiles (no call sites exist)

Write the new `src/include/kernel_log.hpp`:

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <etl/enum_type.h>
#include <etl/format.h>
#include <etl/format_spec.h>
#include <etl/print.h>
#include <etl/string.h>
#include <etl/to_string.h>

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

  LogLine(LogLine&& other) noexcept
      : spec_(other.spec_), released_(other.released_) {
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

  // ── ETL manipulator overloads ────────────────────────────────────────────

  /// 进制：etl::hex / etl::dec / etl::oct / etl::bin
  auto operator<<(
      etl::private_basic_format_spec::base_spec s) -> LogLine& {
    spec_.base(s.base);
    return *this;
  }

  /// 宽度：etl::setw(N)
  auto operator<<(
      etl::private_basic_format_spec::width_spec s) -> LogLine& {
    spec_.width(s.width);
    return *this;
  }

  /// 填充：etl::setfill('0')
  auto operator<<(
      etl::private_basic_format_spec::fill_spec<char> s) -> LogLine& {
    spec_.fill(s.fill);
    return *this;
  }

  /// 大小写：etl::uppercase / etl::nouppercase
  auto operator<<(
      etl::private_basic_format_spec::uppercase_spec s) -> LogLine& {
    spec_.upper_case(s.upper_case);
    return *this;
  }

  /// 布尔：etl::boolalpha / etl::noboolalpha
  auto operator<<(
      etl::private_basic_format_spec::boolalpha_spec s) -> LogLine& {
    spec_.boolalpha(s.boolalpha);
    return *this;
  }

  /// 显示进制前缀：etl::showbase / etl::noshowbase
  auto operator<<(
      etl::private_basic_format_spec::showbase_spec s) -> LogLine& {
    spec_.show_base(s.show_base);
    return *this;
  }

  /// 直接传入完整 format_spec（高级用法）
  auto operator<<(const etl::format_spec& s) -> LogLine& {
    spec_ = s;
    return *this;
  }

  // ── Value overloads ──────────────────────────────────────────────────────

  /// 输出有符号整数类型
  template <std::signed_integral T>
  auto operator<<(T val) -> LogLine& {
    etl::string<32> buf;
    etl::to_string(val, buf, spec_);
    for (char c : buf) {
      sk_putchar(c, nullptr);
    }
    spec_ = {};
    return *this;
  }

  /// 输出无符号整数类型
  template <std::unsigned_integral T>
    requires(!std::same_as<T, bool> && !std::same_as<T, char>)
  auto operator<<(T val) -> LogLine& {
    etl::string<32> buf;
    etl::to_string(val, buf, spec_);
    for (char c : buf) {
      sk_putchar(c, nullptr);
    }
    spec_ = {};
    return *this;
  }

  /// 输出 C 字符串
  auto operator<<(const char* val) -> LogLine& {
    etl::string<256> buf;
    etl::to_string(etl::string_view(val), buf, spec_);
    for (char c : buf) {
      sk_putchar(c, nullptr);
    }
    spec_ = {};
    return *this;
  }

  /// 输出单个字符
  auto operator<<(char val) -> LogLine& {
    sk_putchar(val, nullptr);
    return *this;
  }

  /// 输出布尔值
  auto operator<<(bool val) -> LogLine& {
    etl::string<8> buf;
    etl::to_string(val, buf, spec_);
    for (char c : buf) {
      sk_putchar(c, nullptr);
    }
    spec_ = {};
    return *this;
  }

  /// 输出指针地址
  auto operator<<(const void* val) -> LogLine& {
    etl::string<32> buf;
    etl::format_spec ptr_spec;
    ptr_spec.base(16).show_base(true);
    etl::to_string(reinterpret_cast<uintptr_t>(val), buf, ptr_spec);
    for (char c : buf) {
      sk_putchar(c, nullptr);
    }
    return *this;
  }

 private:
  etl::format_spec spec_{};
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
 * @brief printf 风格日志输出（ETL 类型安全版本）
 *
 * 使用 etl::format_string<Args...> 进行编译期格式字符串校验。
 * 格式语法：{} 整数/字符串，{:x} 十六进制，{:#x} 带前缀十六进制，{:08x} 宽度填充。
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
    etl::format_string<Args...> fmt, Args&&... args) {
  if constexpr (Level < kMinLogLevel) {
    return;
  }
  LockGuard<SpinLock> lock_guard(log_lock);
  etl::print("{}[{}]", kLogColors[Level], cpu_io::GetCurrentCoreId());
  if constexpr (Level == LogLevel::kDebug) {
    etl::print("[{}:{} {}] ", location.file_name(), location.line(),
               location.function_name());
  }
  etl::print(etl::move(fmt), etl::forward<Args>(args)...);
  etl::print("{}", kReset);
}

}  // namespace detail

/**
 * @brief Debug 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 * @note 使用 CTAD 推导模板参数
 */
template <typename... Args>
struct Debug {
  explicit Debug(etl::format_string<Args...> fmt, Args&&... args,
                 const std::source_location& location =
                     std::source_location::current()) {
    if constexpr (detail::LogLevel::kDebug >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kDebug>(
          location, etl::move(fmt), std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Debug(etl::format_string<Args...>, Args&&...) -> Debug<Args...>;

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
  explicit Info(etl::format_string<Args...> fmt, Args&&... args,
                const std::source_location& location =
                    std::source_location::current()) {
    if constexpr (detail::LogLevel::kInfo >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kInfo>(
          location, etl::move(fmt), std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Info(etl::format_string<Args...>, Args&&...) -> Info<Args...>;

/**
 * @brief Warn 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Warn {
  explicit Warn(etl::format_string<Args...> fmt, Args&&... args,
                const std::source_location& location =
                    std::source_location::current()) {
    if constexpr (detail::LogLevel::kWarn >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kWarn>(
          location, etl::move(fmt), std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Warn(etl::format_string<Args...>, Args&&...) -> Warn<Args...>;

/**
 * @brief Err 级别 printf 风格日志
 * @tparam Args 格式化参数类型
 */
template <typename... Args>
struct Err {
  explicit Err(etl::format_string<Args...> fmt, Args&&... args,
               const std::source_location& location =
                   std::source_location::current()) {
    if constexpr (detail::LogLevel::kErr >= detail::kMinLogLevel) {
      detail::LogEmit<detail::LogLevel::kErr>(
          location, etl::move(fmt), std::forward<Args>(args)...);
    }
  }
};
template <typename... Args>
Err(etl::format_string<Args...>, Args&&...) -> Err<Args...>;

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
```

### Step 2: Attempt to build riscv64 to see compile errors from call sites

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | grep "error:" | head -40
```

Expected: Many errors of the form `cannot convert 'const char*' to 'etl::format_string<...>'` — these are the 289 call sites we will fix in Tasks 5–11.

**Do NOT commit yet** — the build is intentionally broken until call sites are migrated.

---

## Task 5: Migrate call sites — `src/syscall.cpp`

**Files:**
- Modify: `src/syscall.cpp`

### Format migration table

| printf syntax | ETL syntax |
|---|---|
| `%d` / `%i` / `%u` | `{}` |
| `%ld` / `%lu` | `{}` |
| `%lld` / `%llu` | `{}` |
| `%x` / `%X` | `{:x}` / `{:X}` |
| `%#x` / `%#lx` | `{:#x}` |
| `%08x` | `{:08x}` |
| `%p` | `{:#x}` with `reinterpret_cast<uintptr_t>(ptr)` |
| `%s` | `{}` |
| `%c` | `{}` |
| `%zu` | `{}` |
| `\n` at end | keep as-is (LogEmit does NOT auto-add newline) |

### Step 1: Apply format string migration to every `klog::` call in `src/syscall.cpp`

Example transformations:
```cpp
// Before
klog::Err("[Syscall] Unknown syscall id: %d\n", syscall_id);
// After
klog::Err("[Syscall] Unknown syscall id: {}\n", syscall_id);

// Before
klog::Debug("[Syscall] FUTEX_WAIT on %p (val=%d)\n", uaddr, val);
// After
klog::Debug("[Syscall] FUTEX_WAIT on {:#x} (val={})\n", reinterpret_cast<uintptr_t>(uaddr), val);

// Before
klog::Debug("[Syscall] Set CPU affinity for task %zu to 0x%lx\n", target->pid, ...);
// After
klog::Debug("[Syscall] Set CPU affinity for task {} to {:#x}\n", target->pid, ...);
```

### Step 2: Build to verify `src/syscall.cpp` now compiles cleanly

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep "syscall.cpp" | head -20
```

Expected: No more errors from `syscall.cpp`.

---

## Task 6: Migrate call sites — `src/device/`

**Files:**
- Modify: `src/device/include/device_manager.hpp`
- Modify: `src/device/include/platform_bus.hpp`
- Modify: `src/device/ns16550a/ns16550a_driver.hpp`
- Modify: `src/device/device.cpp`
- Modify: `src/device/device_manager.cpp`
- Modify: `src/device/virtio/device/blk/virtio_blk.hpp`
- Modify: `src/device/virtio/device/device_initializer.hpp`
- Modify: `src/device/virtio/transport/mmio.hpp`
- Modify: `src/device/virtio/virtio_driver.cpp`

### Step 1: Run grep to see all klog calls in device/

```bash
grep -rn "klog::" src/device/ --include="*.cpp" --include="*.hpp"
```

### Step 2: Apply format string migration to every call found

Apply the same transformation table from Task 5. Pay special attention to:
- Pointer args (`%p` → `{:#x}` with `reinterpret_cast<uintptr_t>`)
- `%s` with `const char*` vars → `{}`
- `%lu` / `%zu` with `size_t` args → `{}`

### Step 3: Build to verify device/ compiles cleanly

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep "device" | grep "error:" | head -20
```

Expected: No errors from device/ files.

### Step 4: Commit

```bash
git add src/device/
git commit --signoff -m "refactor(device): migrate klog calls to ETL format syntax"
```

---

## Task 7: Migrate call sites — `src/task/`

**Files:**
- Modify: `src/task/wakeup.cpp`
- Modify: `src/task/task_manager.cpp`
- Modify: `src/task/task_control_block.cpp`
- Modify: `src/task/include/cfs_scheduler.hpp`
- Modify: `src/task/include/task_fsm.hpp`
- Modify: `src/task/include/rr_scheduler.hpp`
- Modify: `src/task/include/fifo_scheduler.hpp`
- Modify: `src/task/exit.cpp`
- Modify: `src/task/block.cpp`
- Modify: `src/task/wait.cpp`
- Modify: `src/task/mutex.cpp`
- Modify: `src/task/sleep.cpp`
- Modify: `src/task/clone.cpp`

### Step 1: Run grep to see all klog calls in task/

```bash
grep -rn "klog::" src/task/ --include="*.cpp" --include="*.hpp"
```

### Step 2: Apply format string migration

Same table as Task 5.

### Step 3: Build to verify task/ compiles cleanly

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep "task" | grep "error:" | head -20
```

### Step 4: Commit

```bash
git add src/task/
git commit --signoff -m "refactor(task): migrate klog calls to ETL format syntax"
```

---

## Task 8: Migrate call sites — `src/arch/`

**Files:**
- Modify: `src/arch/x86_64/arch_main.cpp`
- Modify: `src/arch/x86_64/apic/apic.cpp`
- Modify: `src/arch/x86_64/apic/io_apic.cpp`
- (Check for others): `grep -rl "klog::" src/arch/`

### Step 1: Run grep to see all klog calls in arch/

```bash
grep -rn "klog::" src/arch/ --include="*.cpp" --include="*.hpp"
```

### Step 2: Apply format string migration

Same table. x86_64 APIC code often uses `%#lx` for hardware addresses — migrate to `{:#x}`.

### Step 3: Build to verify arch/ compiles cleanly

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep "arch" | grep "error:" | head -20
```

### Step 4: Commit

```bash
git add src/arch/
git commit --signoff -m "refactor(arch): migrate klog calls to ETL format syntax"
```

---

## Task 9: Migrate call sites — `src/include/` headers and remaining `src/*.cpp`

**Files:**
- Modify: `src/include/basic_info.hpp`
- Modify: `src/include/kernel_fdt.hpp`
- Modify: `src/include/kernel_elf.hpp`
- Modify: `src/syscall.cpp` (if not done in Task 5)
- (Check for others): `grep -rl "klog::" src/ --include="*.cpp" --include="*.hpp" | grep -v "src/arch\|src/task\|src/device\|kernel_log"`

### Step 1: Run grep to find remaining files

```bash
grep -rl "klog::" src/ --include="*.cpp" --include="*.hpp" | grep -v "src/arch\|src/task\|src/device\|src/include/kernel_log"
```

### Step 2: Apply format string migration to each file found

### Step 3: Full build — zero errors expected

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep "error:" | head -20
```

Expected: **No errors.**

### Step 4: Commit

```bash
git add src/
git commit --signoff -m "refactor(src): migrate remaining klog calls to ETL format syntax"
```

---

## Task 10: Full build verification across all architectures

### Step 1: Build riscv64

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | tail -5
```

Expected: `[100%] Built target SimpleKernel` or similar success message.

### Step 2: Build x86_64

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | tail -5
```

Expected: Success.

### Step 3: Build aarch64

```bash
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel 2>&1 | tail -5
```

Expected: Success.

### Step 4: Run unit tests (host arch only)

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make unit-test 2>&1 | tail -20
```

Expected: All tests pass (or pre-existing failures unchanged).

### Step 5: Run clang-format check

```bash
pre-commit run --all-files 2>&1 | tail -30
```

Fix any formatting issues, then commit formatting fixes separately:

```bash
git add -u
git commit --signoff -m "style: apply clang-format after ETL log rewrite"
```

---

## Task 11: Final commit — `kernel_log.hpp` and `syscall.cpp`

Commit the header rewrite and syscall migration (these were intentionally held back):

```bash
git add src/include/kernel_log.hpp src/syscall.cpp
git commit --signoff -m "feat(klog): rewrite kernel_log.hpp with ETL type-safe format API

Replace printf-style sk_printf backend with etl::print + etl::format_string<Args...>
for compile-time format string validation. Stream path (LogLine) gains etl::format_spec
support with manipulator overloads (etl::hex, etl::setw, etl::setfill, etc.).

Closes: kernel_log ETL rewrite (design doc: doc/plans/2026-03-01-kernel-log-etl-rewrite-design.md)"
```

---

## Format Migration Quick Reference

When migrating call sites, use this table:

| printf | ETL `{}` syntax | Notes |
|--------|-----------------|-------|
| `%d` / `%i` | `{}` | |
| `%u` | `{}` | |
| `%ld` / `%lu` | `{}` | |
| `%lld` / `%llu` | `{}` | |
| `%x` | `{:x}` | lowercase hex |
| `%X` | `{:X}` | uppercase hex |
| `%#x` | `{:#x}` | hex with 0x prefix |
| `%08x` | `{:08x}` | 8-wide zero-padded |
| `%#010x` | `{:#010x}` | 0x-prefixed, 10-wide |
| `%p` | `{:#x}` | MUST add `reinterpret_cast<uintptr_t>(ptr)` |
| `%s` | `{}` | |
| `%c` | `{}` | |
| `%zu` | `{}` | |
| `%02X` | `{:02X}` | 2-wide zero-padded uppercase |
| `\n` | `\n` | keep as-is; LogEmit does NOT auto-add newline |

**Pointer pattern:**
```cpp
// Before
klog::Debug("addr=%p\n", some_ptr);
// After
klog::Debug("addr={:#x}\n", reinterpret_cast<uintptr_t>(some_ptr));
```

**String from `error().message()` pattern:**
```cpp
// Before
klog::Err("failed: %s\n", result.error().message());
// After — message() returns const char*, {} works fine
klog::Err("failed: {}\n", result.error().message());
```
