# EasyLogger Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the `klog` C++ stream-style logging with EasyLogger's printf-style API across all 58 kernel files, while preserving the lock-free raw_* API for interrupt contexts.

**Architecture:** EasyLogger (git submodule) uses stb_sprintf for freestanding printf. Port layer in `kernel_log.cpp` bridges to `sk_print_str()`. Config macros injected via CMake interface target. LOG_TAG defined per-module in CMakeLists.txt. raw_* classes retained in `kernel_log.hpp`.

**Tech Stack:** C23/C++23, freestanding, EasyLogger (C), stb_sprintf (C), CMake, GoogleTest

---

## File Reference

| File | Role |
|------|------|
| `3rd/easylogger` | **NEW submodule** — EasyLogger source |
| `3rd/stb` | **NEW submodule** — stb single-header libraries |
| `src/libc/include/sk_printf.h` | **NEW** — vsnprintf/snprintf declarations |
| `src/libc/sk_printf.c` | **NEW** — stb_sprintf impl + wrappers |
| `src/libc/sk_string.c` | **MODIFY** — add strstr() |
| `src/libc/include/sk_string.h` | **MODIFY** — add strstr() declaration |
| `src/libc/CMakeLists.txt` | **MODIFY** — add sk_printf.c |
| `src/include/elog_cfg.h` | **NEW** — minimal shim for EasyLogger |
| `src/include/kernel_log.hpp` | **MODIFY** — strip to raw_* + elog includes + KernelLogInit |
| `src/kernel_log.cpp` | **NEW** — elog_port_* + KernelLogInit() |
| `cmake/compile_config.cmake` | **MODIFY** — add elog_config + elog_lib targets |
| `src/CMakeLists.txt` | **MODIFY** — add kernel_log.cpp, link elog_lib, set LOG_TAG |
| `src/task/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/device/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/device/ns16550a/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/device/pl011/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/device/virtio/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/filesystem/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/filesystem/vfs/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/filesystem/ramfs/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/filesystem/fatfs/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/memory/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| `src/arch/CMakeLists.txt` | **MODIFY** — add LOG_TAG |
| All 58 `.cpp`/`.hpp` files using `klog::` | **MODIFY** — migrate to elog API |

## Conventions Reminder

- Google C++ style, `.clang-format` enforced
- Header guards: `#ifndef SIMPLEKERNEL_SRC_INCLUDE_FILENAME_HPP_`
- Copyright: `/** @copyright Copyright The SimpleKernel Contributors */`
- Trailing return types: `auto Func() -> RetType`
- Members: `snake_case_` (trailing underscore)
- Constants: `kCamelCase`
- `Expected<T>` for errors, no exceptions/RTTI
- Doxygen `@brief`/`@pre`/`@post` on interface methods
- Includes: system → third-party → project
- CMake: UPPERCASE commands/keywords, 4-space indent, 80-char lines, space before `(`

---

### Task 1: Add git submodules

**Files:**
- Create: `3rd/easylogger` (submodule)
- Create: `3rd/stb` (submodule)

**Step 1: Add EasyLogger submodule**

```bash
git submodule add https://github.com/armink/EasyLogger.git 3rd/easylogger
```

**Step 2: Add stb submodule**

```bash
git submodule add https://github.com/nothings/stb.git 3rd/stb
```

**Step 3: Verify submodules exist**

```bash
ls 3rd/easylogger/easylogger/inc/elog.h
ls 3rd/stb/stb_sprintf.h
```

Expected: Both files exist.

**Step 4: Commit**

```bash
git add 3rd/easylogger 3rd/stb .gitmodules
git commit --signoff -m "build(3rd): add easylogger and stb submodules"
```

---

### Task 2: Add strstr() to kernel libc

EasyLogger's `elog.c` calls `strstr()`. The kernel libc does not provide it.

**Files:**
- Modify: `src/libc/include/sk_string.h`
- Modify: `src/libc/sk_string.c`

**Step 1: Add strstr declaration to sk_string.h**

Add after the `strnlen` declaration (before the `#ifdef __cplusplus` closing):

```c
// 查找子字符串在字符串中的首次出现
char* strstr(const char* haystack, const char* needle);
```

**Step 2: Add strstr implementation to sk_string.c**

Add before the closing `#ifdef __cplusplus`:

```c
// 查找子字符串在字符串中的首次出现
char* strstr(const char* haystack, const char* needle) {
  if (!*needle) {
    return (char*)haystack;
  }
  for (; *haystack; ++haystack) {
    const char* h = haystack;
    const char* n = needle;
    while (*h && *n && (*h == *n)) {
      ++h;
      ++n;
    }
    if (!*n) {
      return (char*)haystack;
    }
  }
  return NULL;
}
```

**Step 3: Verify compilation**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make libc
```

Expected: Build succeeds.

**Step 4: Commit**

```bash
git add src/libc/include/sk_string.h src/libc/sk_string.c
git commit --signoff -m "feat(libc): add strstr() for easylogger dependency"
```

---

### Task 3: Add stb_sprintf wrappers (sk_printf)

**Files:**
- Create: `src/libc/include/sk_printf.h`
- Create: `src/libc/sk_printf.c`
- Modify: `src/libc/CMakeLists.txt`

**Step 1: Create sk_printf.h**

```c
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_PRINTF_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_PRINTF_H_

#include <stdarg.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// printf-style format into buffer (freestanding, no float)
int vsnprintf(char* buf, size_t count, const char* fmt, va_list va);

/// printf-style format into buffer (freestanding, no float)
int snprintf(char* buf, size_t count, const char* fmt, ...);

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_PRINTF_H_
```

**Step 2: Create sk_printf.c**

```c
/** @copyright Copyright The SimpleKernel Contributors */

// stb_sprintf configuration — must be before include
#define STB_SPRINTF_IMPLEMENTATION
#define STB_SPRINTF_NOFLOAT

// Provide the stb_sprintf header from 3rd/stb
#include "stb_sprintf.h"

#include "sk_printf.h"

#ifdef __cplusplus
extern "C" {
#endif

int vsnprintf(char* buf, size_t count, const char* fmt, va_list va) {
  return stbsp_vsnprintf(buf, (int)count, fmt, va);
}

int snprintf(char* buf, size_t count, const char* fmt, ...) {
  va_list va;
  va_start(va, fmt);
  int result = stbsp_vsnprintf(buf, (int)count, fmt, va);
  va_end(va);
  return result;
}

#ifdef __cplusplus
}
#endif
```

**Step 3: Update libc CMakeLists.txt**

Replace the entire file with:

```cmake
# Copyright The SimpleKernel Contributors

ADD_LIBRARY (libc INTERFACE)

TARGET_INCLUDE_DIRECTORIES (libc INTERFACE include)

TARGET_INCLUDE_DIRECTORIES (libc INTERFACE ${CMAKE_SOURCE_DIR}/3rd/stb)

TARGET_SOURCES (libc INTERFACE sk_stdlib.c sk_string.c sk_ctype.c sk_stdio.c
                               sk_printf.c)
```

**Step 4: Verify compilation**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make libc
```

Expected: Build succeeds. `vsnprintf` and `snprintf` are available.

**Step 5: Commit**

```bash
git add src/libc/include/sk_printf.h src/libc/sk_printf.c src/libc/CMakeLists.txt
git commit --signoff -m "feat(libc): add vsnprintf/snprintf via stb_sprintf"
```

---

### Task 4: Add EasyLogger CMake targets and elog_cfg.h shim

**Files:**
- Modify: `cmake/compile_config.cmake`
- Create: `src/include/elog_cfg.h`

**Step 1: Create elog_cfg.h shim**

EasyLogger's `elog.h` does `#include <elog_cfg.h>`. The actual macros are injected by CMake, so the shim is minimal.

```c
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_ELOG_CFG_H_
#define SIMPLEKERNEL_SRC_INCLUDE_ELOG_CFG_H_

/*
 * EasyLogger configuration.
 *
 * All ELOG_* macros are defined via CMake TARGET_COMPILE_DEFINITIONS
 * on the elog_config interface target (see cmake/compile_config.cmake).
 * This file exists solely to satisfy elog.h's #include <elog_cfg.h>.
 */

#endif  // SIMPLEKERNEL_SRC_INCLUDE_ELOG_CFG_H_
```

**Step 2: Add elog_config and elog_lib targets to compile_config.cmake**

Append to the end of `cmake/compile_config.cmake`:

```cmake
# EasyLogger 配置宏
ADD_LIBRARY (elog_config INTERFACE)
TARGET_COMPILE_DEFINITIONS (
    elog_config
    INTERFACE
        ELOG_OUTPUT_ENABLE
        ELOG_OUTPUT_LVL=5
        ELOG_ASSERT_ENABLE
        ELOG_LINE_BUF_SIZE=512
        ELOG_LINE_NUM_MAX_LEN=5
        ELOG_FILTER_TAG_MAX_LEN=30
        ELOG_FILTER_KW_MAX_LEN=16
        ELOG_FILTER_TAG_LVL_MAX_NUM=5
        ELOG_NEWLINE_SIGN="\\n"
        ELOG_COLOR_ENABLE
        ELOG_ASYNC_OUTPUT_ENABLE
        ELOG_BUF_OUTPUT_ENABLE
        ELOG_ASYNC_LINE_BUF_SIZE=512
        ELOG_ASYNC_OUTPUT_BUF_SIZE=5120
        ELOG_ASYNC_POLL_GET_LOG_BUF_SIZE=512
        ELOG_BUF_OUTPUT_BUF_SIZE=512)

# EasyLogger 库
ADD_LIBRARY (elog_lib INTERFACE)
TARGET_INCLUDE_DIRECTORIES (
    elog_lib
    INTERFACE ${CMAKE_SOURCE_DIR}/3rd/easylogger/easylogger/inc
              ${CMAKE_SOURCE_DIR}/src/include)
TARGET_SOURCES (
    elog_lib
    INTERFACE ${CMAKE_SOURCE_DIR}/3rd/easylogger/easylogger/src/elog.c
              ${CMAKE_SOURCE_DIR}/3rd/easylogger/easylogger/src/elog_utils.c
              ${CMAKE_SOURCE_DIR}/3rd/easylogger/easylogger/src/elog_async.c
              ${CMAKE_SOURCE_DIR}/3rd/easylogger/easylogger/src/elog_buf.c)
TARGET_LINK_LIBRARIES (elog_lib INTERFACE elog_config libc)
```

**Step 3: Add elog_lib to kernel_link_libraries**

In `cmake/compile_config.cmake`, find the `kernel_link_libraries` target and add `elog_lib` to its `TARGET_LINK_LIBRARIES`:

```cmake
ADD_LIBRARY (kernel_link_libraries INTERFACE)
TARGET_LINK_LIBRARIES (
    kernel_link_libraries
    INTERFACE link_libraries
              kernel_compile_definitions
              kernel_compile_options
              kernel_link_options
              dtc-lib
              cpu_io
              bmalloc
              MPMCQueue
              fatfs_lib
              etl::etl
              elog_lib
              gcc
              $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},riscv64>:
              opensbi_interface
              >
              $<$<STREQUAL:${CMAKE_SYSTEM_PROCESSOR},aarch64>:
              teec
              >)
```

**Step 4: Verify cmake configure succeeds**

```bash
cmake --preset build_x86_64
```

Expected: CMake configure succeeds without errors.

**Step 5: Commit**

```bash
git add cmake/compile_config.cmake src/include/elog_cfg.h
git commit --signoff -m "build(cmake): add elog_config and elog_lib targets"
```

---

### Task 5: Implement port layer and KernelLogInit in kernel_log.cpp

**Files:**
- Create: `src/kernel_log.cpp`

**Step 1: Create kernel_log.cpp**

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#include "kernel_log.hpp"

#include <elog.h>

#include "sk_stdio.h"

extern "C" {

/// @brief EasyLogger port: initialize (no-op, serial already up)
void elog_port_init(void) {}

/// @brief EasyLogger port: deinitialize (no-op)
void elog_port_deinit(void) {}

/// @brief EasyLogger port: output log string to serial
void elog_port_output(const char* log, size_t size) {
  (void)size;
  sk_print_str(log);
}

/// @brief EasyLogger port: acquire output lock
void elog_port_output_lock(void) {
  klog::detail::log_lock.Lock().or_else([](auto&&) -> Expected<void> {
    sk_print_str("elog: lock failed\n");
    while (true) {
      cpu_io::Pause();
    }
    __builtin_unreachable();
  });
}

/// @brief EasyLogger port: release output lock
void elog_port_output_unlock(void) {
  klog::detail::log_lock.UnLock().or_else([](auto&&) -> Expected<void> {
    sk_print_str("elog: unlock failed\n");
    while (true) {
      cpu_io::Pause();
    }
    __builtin_unreachable();
  });
}

/// @brief EasyLogger port: get current time string
/// @return Pointer to static time string (empty — no RTC yet)
auto elog_port_get_time(void) -> const char* { return ""; }

/// @brief EasyLogger port: get process info string
/// @return Pointer to static process info string (empty — single address space)
auto elog_port_get_p_info(void) -> const char* { return ""; }

/// @brief EasyLogger port: get thread info string
/// @return Pointer to static thread info string (empty for now)
auto elog_port_get_t_info(void) -> const char* { return ""; }

}  // extern "C"

auto KernelLogInit() -> Expected<void> {
  elog_init();
  elog_start();
  return {};
}
```

**Step 2: Add kernel_log.cpp to src/CMakeLists.txt**

In `src/CMakeLists.txt`, modify the `ADD_EXECUTABLE` line to include `kernel_log.cpp`:

```cmake
ADD_EXECUTABLE (${PROJECT_NAME} main.cpp io_buffer.cpp syscall.cpp
                                kernel_log.cpp)
```

**Step 3: Verify compilation**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

Expected: Build succeeds (may have linker warnings about unused klog symbols — that's fine at this stage).

**Step 4: Commit**

```bash
git add src/kernel_log.cpp src/CMakeLists.txt
git commit --signoff -m "feat(log): implement EasyLogger port layer and KernelLogInit"
```

---

### Task 6: Strip kernel_log.hpp to raw_* only + elog includes

**Files:**
- Modify: `src/include/kernel_log.hpp`

**Step 1: Rewrite kernel_log.hpp**

Keep: `klog::detail` namespace with `LogLevel`, `log_lock`, ANSI colors, `EmitHeader`, `raw_log_buf_`, `LogLineRaw`, `LogStreamRaw`, and the `raw_debug()`/`raw_info`/`raw_warn`/`raw_err` instances.

Remove: `LogLine`, `LogStream`, `debug()`, `info`, `warn`, `err`, `DebugBlob`, `hex`, `HEX`.

Add: `#include <elog.h>`, `KernelLogInit()` declaration.

The new file:

```cpp
/** @copyright Copyright The SimpleKernel Contributors */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_

#include <elog.h>
#include <etl/enum_type.h>
#include <etl/format_spec.h>
#include <etl/string.h>
#include <etl/string_stream.h>

#include <cstddef>
#include <cstdint>
#include <source_location>

#include "expected.hpp"
#include "kstd_cstdio"
#include "spinlock.hpp"

/// @brief Initialize the kernel logging subsystem (EasyLogger)
/// @pre   Serial output (sk_print_str) is functional
/// @post  elog_*() macros are safe to call
auto KernelLogInit() -> Expected<void>;

namespace klog {
namespace detail {

// 日志专用的自旋锁实例 (used by port layer and raw_* API)
inline SpinLock log_lock("kernel_log");

/// ANSI 转义码
inline constexpr auto kReset = "\033[0m";
inline constexpr auto kRed = "\033[31m";
inline constexpr auto kGreen = "\033[32m";
inline constexpr auto kYellow = "\033[33m";
inline constexpr auto kBlue = "\033[34m";
inline constexpr auto kMagenta = "\033[35m";
inline constexpr auto kCyan = "\033[36m";
inline constexpr auto kWhite = "\033[37m";

/**
 * @brief 类型安全的日志级别枚举 (retained for raw_* API)
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

/// 编译期最低日志级别
inline constexpr auto kMinLogLevel =
    static_cast<LogLevel::enum_type>(SIMPLEKERNEL_MIN_LOG_LEVEL);

/// 每种日志级别对应的 ANSI 颜色
inline constexpr const char* kLogColors[LogLevel::kMax] = {
    kMagenta,
    kCyan,
    kYellow,
    kRed,
};

/// No-op tag for disabled log levels
struct NoOpTag {};

/// per-cpu 紧急日志缓冲区，用于无锁日志输出
inline etl::string<512> raw_log_buf_[SIMPLEKERNEL_MAX_CORE_COUNT]{};

/**
 * @brief Lock-free RAII stream-style log line (per-CPU buffer)
 *
 * 用于中断/异常上下文的无锁日志输出。
 * 构造时获取 CPU ID、清空对应核心缓冲区并写入 ANSI 前缀。
 * 析构时调用 sk_print_str() 输出，无需持有锁。
 *
 * @tparam Level 日志级别
 */
template <LogLevel::enum_type Level>
class LogLineRaw {
 public:
  explicit LogLineRaw(NoOpTag)
      : cpu_id_(0), stream_(raw_log_buf_[0]), released_(true) {}

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
      sk_print_str(raw_log_buf_[cpu_id_].c_str());
    }
  }

  template <typename T>
  auto operator<<(T&& val) -> LogLineRaw& {
    if (!released_) {
      stream_ << static_cast<T&&>(val);
    }
    return *this;
  }

  auto operator<<(bool val) -> LogLineRaw& {
    if (!released_) {
      stream_ << (val ? "true" : "false");
    }
    return *this;
  }

  auto operator<<(char c) -> LogLineRaw& {
    if (!released_) {
      raw_log_buf_[cpu_id_].push_back(c);
    }
    return *this;
  }

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
  uint64_t cpu_id_;
  etl::string_stream stream_;
  bool released_ = false;
};

/**
 * @brief 无锁惰性流式日志代理（LogStreamRaw）
 */
template <LogLevel::enum_type Level>
struct LogStreamRaw {
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

}  // namespace detail

/**
 * @brief 创建无锁 Debug 级别的 LogLineRaw
 */
[[nodiscard]] inline auto raw_debug(
    std::source_location loc = std::source_location::current())
    -> detail::LogLineRaw<detail::LogLevel::kDebug> {
  if constexpr (detail::LogLevel::kDebug < detail::kMinLogLevel) {
    return detail::LogLineRaw<detail::LogLevel::kDebug>{detail::NoOpTag{}};
  }
  return detail::LogLineRaw<detail::LogLevel::kDebug>{loc};
}

/// 无锁流式日志实例 — 可在中断/异常上下文中安全使用
/// @{
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kInfo> raw_info;
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kWarn> raw_warn;
[[maybe_unused]] inline detail::LogStreamRaw<detail::LogLevel::kErr> raw_err;
/// @}

}  // namespace klog

#endif  // SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
```

**Step 2: Verify the header compiles (build will fail at link due to unmigrated call sites — that's expected)**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel 2>&1 | head -50
```

Expected: Compilation errors from call sites still using `klog::info`, `klog::err`, etc. This is expected — we migrate them in the next tasks.

**Step 3: Commit**

```bash
git add src/include/kernel_log.hpp
git commit --signoff -m "refactor(log): strip kernel_log.hpp to raw_* API + elog includes"
```

---

### Task 7: Define LOG_TAG per module in CMakeLists.txt

**Files:**
- Modify: `src/CMakeLists.txt` — `LOG_TAG="kernel"`
- Modify: `src/task/CMakeLists.txt` — `LOG_TAG="task"`
- Modify: `src/device/CMakeLists.txt` — `LOG_TAG="device"`
- Modify: `src/device/ns16550a/CMakeLists.txt` — `LOG_TAG="ns16550a"`
- Modify: `src/device/pl011/CMakeLists.txt` — `LOG_TAG="pl011"`
- Modify: `src/device/virtio/CMakeLists.txt` — `LOG_TAG="virtio"`
- Modify: `src/device/acpi/CMakeLists.txt` — `LOG_TAG="acpi"`
- Modify: `src/filesystem/CMakeLists.txt` — `LOG_TAG="fs"`
- Modify: `src/filesystem/vfs/CMakeLists.txt` — `LOG_TAG="vfs"`
- Modify: `src/filesystem/ramfs/CMakeLists.txt` — `LOG_TAG="ramfs"`
- Modify: `src/filesystem/fatfs/CMakeLists.txt` — `LOG_TAG="fatfs"`
- Modify: `src/memory/CMakeLists.txt` — `LOG_TAG="memory"`
- Modify: `src/arch/CMakeLists.txt` — `LOG_TAG="arch"`

**Step 1: Add LOG_TAG to each module's CMakeLists.txt**

For each INTERFACE library, add after `ADD_LIBRARY`:

```cmake
TARGET_COMPILE_DEFINITIONS (<target> INTERFACE LOG_TAG="<tag>")
```

For `src/CMakeLists.txt` (executable target), use:

```cmake
TARGET_COMPILE_DEFINITIONS (${PROJECT_NAME} PRIVATE LOG_TAG="kernel")
```

Read each CMakeLists.txt, find the target name, and add the appropriate line. The target names are:
- `src/CMakeLists.txt` → `${PROJECT_NAME}` (PRIVATE)
- `src/task/CMakeLists.txt` → `task` (INTERFACE)
- `src/device/CMakeLists.txt` → `device` (INTERFACE)
- `src/device/ns16550a/CMakeLists.txt` → `ns16550a_driver` (INTERFACE)
- `src/device/pl011/CMakeLists.txt` → `pl011_driver` (INTERFACE)
- `src/device/virtio/CMakeLists.txt` → `virtio_driver` (INTERFACE)
- `src/device/acpi/CMakeLists.txt` → `acpi_driver` (INTERFACE)
- `src/filesystem/CMakeLists.txt` → `filesystem` (INTERFACE)
- `src/filesystem/vfs/CMakeLists.txt` → `vfs` (INTERFACE)
- `src/filesystem/ramfs/CMakeLists.txt` → `ramfs` (INTERFACE)
- `src/filesystem/fatfs/CMakeLists.txt` → `fatfs` (INTERFACE)
- `src/memory/CMakeLists.txt` → `memory` (INTERFACE)
- `src/arch/CMakeLists.txt` → `arch` (INTERFACE)

Note: Sub-drivers (ns16550a, pl011, virtio) define their own specific tags that override the parent `device` tag for their compilation units.

Also add LOG_TAG for architecture sub-modules:
- `src/arch/x86_64/apic/CMakeLists.txt` — look at target name, set `LOG_TAG="apic"`
- `src/arch/riscv64/plic/CMakeLists.txt` — set `LOG_TAG="plic"`
- `src/arch/aarch64/gic/CMakeLists.txt` — set `LOG_TAG="gic"`

**Step 2: Verify cmake configure**

```bash
cmake --preset build_x86_64
```

Expected: Configure succeeds.

**Step 3: Commit**

```bash
git add src/CMakeLists.txt src/task/CMakeLists.txt src/device/CMakeLists.txt \
        src/device/ns16550a/CMakeLists.txt src/device/pl011/CMakeLists.txt \
        src/device/virtio/CMakeLists.txt src/device/acpi/CMakeLists.txt \
        src/filesystem/CMakeLists.txt src/filesystem/vfs/CMakeLists.txt \
        src/filesystem/ramfs/CMakeLists.txt src/filesystem/fatfs/CMakeLists.txt \
        src/memory/CMakeLists.txt src/arch/CMakeLists.txt \
        src/arch/x86_64/apic/CMakeLists.txt \
        src/arch/riscv64/plic/CMakeLists.txt \
        src/arch/aarch64/gic/CMakeLists.txt
git commit --signoff -m "build(cmake): define LOG_TAG per module for easylogger"
```

---

### Task 8: Add KernelLogInit() call to boot sequence

**Files:**
- Modify: `src/main.cpp`

**Step 1: Add KernelLogInit() call after ArchInit**

In the `main()` function, add `KernelLogInit()` after `ArchInit()` (serial is available after ArchInit):

```cpp
  // 架构相关初始化
  ArchInit(argc, argv);

  // 初始化日志子系统
  KernelLogInit();
```

Also change the first `klog::info` usage to `elog_i` to test:

```cpp
  elog_i(LOG_TAG, "Hello SimpleKernel");
```

**Step 2: This step is deferred — full migration happens in Task 9**

**Step 3: Commit**

```bash
git add src/main.cpp
git commit --signoff -m "feat(log): call KernelLogInit() in boot sequence"
```

---

### Task 9: Migrate all 58 files from klog:: to elog API

This is the largest task. It should be executed file-by-file or module-by-module. Each file needs:

1. Remove `klog::info/warn/err/debug()` usage → replace with `elog_i/elog_w/elog_e/elog_d`
2. Keep `klog::raw_info/raw_warn/raw_err/raw_debug()` usage unchanged
3. Remove `#include "kernel_log.hpp"` if only using elog (elog.h is enough) — BUT keep it if using raw_* API
4. Convert stream-style formatting to printf-style formatting

**Migration patterns:**

```cpp
// BEFORE                                    // AFTER
klog::info << "msg";                         elog_i(LOG_TAG, "msg");
klog::info << "val=" << x;                   elog_i(LOG_TAG, "val=%d", x);
klog::info << "ptr=" << ptr;                 elog_i(LOG_TAG, "ptr=%p", ptr);
klog::info << "hex=" << klog::hex << val;    elog_i(LOG_TAG, "hex=0x%llx", (unsigned long long)val);
klog::err << "msg: " << err.message();       elog_e(LOG_TAG, "msg: %s", err.message());
klog::warn << "x=" << x << " y=" << y;      elog_w(LOG_TAG, "x=%d y=%d", x, y);
klog::debug() << "msg";                      elog_d(LOG_TAG, "msg");
```

**Type format specifiers:**
- `int`, `uint32_t` → `%d`, `%u`
- `int64_t`, `uint64_t` → `%lld`, `%llu`
- `size_t` → `%zu`
- `const char*` → `%s`
- `void*` → `%p`
- Hex: `0x%llx` for `uint64_t`, `0x%x` for `uint32_t`
- `bool` → use ternary: `val ? "true" : "false"` as `%s`

**Files grouped by module (migrate in this order):**

#### 9a: src/ top-level (3 files)
- `src/main.cpp`
- `src/syscall.cpp`
- `src/kernel_log.cpp` (no klog:: usage — skip)

#### 9b: src/memory/ (2 files)
- `src/memory/virtual_memory.cpp`
- `src/memory/memory.cpp`

#### 9c: src/arch/ (12 files)
- `src/arch/x86_64/arch_main.cpp`
- `src/arch/x86_64/backtrace.cpp`
- `src/arch/x86_64/interrupt.cpp`
- `src/arch/x86_64/interrupt_main.cpp`
- `src/arch/x86_64/apic/io_apic.cpp`
- `src/arch/x86_64/apic/apic.cpp`
- `src/arch/x86_64/apic/local_apic.cpp`
- `src/arch/aarch64/arch_main.cpp`
- `src/arch/aarch64/backtrace.cpp`
- `src/arch/aarch64/interrupt.cpp`
- `src/arch/aarch64/interrupt_main.cpp`
- `src/arch/aarch64/gic/gic.cpp`
- `src/arch/riscv64/arch_main.cpp`
- `src/arch/riscv64/backtrace.cpp`
- `src/arch/riscv64/interrupt.cpp`
- `src/arch/riscv64/interrupt_main.cpp`
- `src/arch/riscv64/plic/plic.cpp`

#### 9d: src/device/ (6 files)
- `src/device/device_manager.cpp`
- `src/device/device.cpp`
- `src/device/virtio/virtio_driver.cpp`
- `src/device/pl011/pl011_driver.hpp` (header-only driver, has klog:: usage)
- `src/device/ns16550a/ns16550a_driver.hpp` (header-only driver, has klog:: usage)
- `src/device/virtio/device/blk/virtio_blk.hpp` (header-only)
- `src/device/include/platform_bus.hpp` (header-only)
- `src/device/include/device_manager.hpp` (header-only)

#### 9e: src/task/ (14 files)
- `src/task/sleep.cpp`
- `src/task/clone.cpp`
- `src/task/task_control_block.cpp`
- `src/task/schedule.cpp`
- `src/task/exit.cpp`
- `src/task/block.cpp`
- `src/task/mutex.cpp`
- `src/task/wakeup.cpp`
- `src/task/wait.cpp`
- `src/task/task_manager.cpp`
- `src/task/include/rr_scheduler.hpp`
- `src/task/include/fifo_scheduler.hpp`
- `src/task/include/cfs_scheduler.hpp`
- `src/task/include/task_fsm.hpp`

#### 9f: src/include/ headers (4 files)
- `src/include/spinlock.hpp`
- `src/include/kernel_fdt.hpp`
- `src/include/kernel_elf.hpp`
- `src/include/kernel_log.hpp` (already done — skip)

#### 9g: src/filesystem/ (10 files)
- `src/filesystem/ramfs/ramfs.cpp`
- `src/filesystem/filesystem.cpp`
- `src/filesystem/file_descriptor.cpp`
- `src/filesystem/vfs/mount.cpp`
- `src/filesystem/vfs/vfs.cpp`
- `src/filesystem/vfs/rmdir.cpp`
- `src/filesystem/vfs/open.cpp`
- `src/filesystem/vfs/unlink.cpp`
- `src/filesystem/vfs/mkdir.cpp`
- `src/filesystem/fatfs/diskio.cpp`
- `src/filesystem/fatfs/fatfs.cpp`

**For each sub-group, the commit pattern is:**

```bash
git add <files>
git commit --signoff -m "refactor(log): migrate <module> from klog to elog"
```

**Step: After all migrations, verify full build**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
```

Expected: Build succeeds with no errors.

---

### Task 10: Final verification

**Step 1: Build all three architectures**

```bash
cmake --preset build_x86_64 && cd build_x86_64 && make SimpleKernel
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
cmake --preset build_aarch64 && cd build_aarch64 && make SimpleKernel
```

Expected: All three builds succeed.

**Step 2: Run format check**

```bash
pre-commit run --all-files
```

Expected: All checks pass (or only pre-existing issues).

**Step 3: Run on QEMU (x86_64 or riscv64)**

```bash
cd build_x86_64 && make run
```

Expected: Kernel boots, log output visible with EasyLogger format (tags, colors).

**Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit --signoff -m "fix(log): address build/format issues from elog migration"
```
