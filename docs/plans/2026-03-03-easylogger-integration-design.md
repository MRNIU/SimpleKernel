# EasyLogger Integration Design

## Goal

Replace the existing `klog` C++ stream-style logging module with [EasyLogger](https://github.com/armink/EasyLogger)'s printf-style API. The current `klog` system is header-only (459 lines in `kernel_log.hpp`) using ETL string streams and RAII log lines. EasyLogger is a pure C library with printf-style formatting, tag-based filtering, and a portable architecture. 58 files across the kernel use `klog::`.

## Architecture

EasyLogger is added as a git submodule. Its C-library dependencies (`vsnprintf`, `snprintf`) are satisfied by [stb_sprintf](https://github.com/nothings/stb) (also a submodule), which works in freestanding environments. The port layer (`elog_port_*` functions) lives in `kernel_log.cpp`, outputting through the existing `sk_print_str()` chain. The raw_* (lock-free, per-CPU) logging pattern is preserved for interrupt/exception contexts.

## Decisions

### 1. Full API replacement (not wrapper)

All 58 call sites migrate from `klog::info << "msg" << val;` to `elog_i(LOG_TAG, "fmt", ...)`. The `klog` namespace is removed entirely. This avoids a wrapper layer that would add complexity without value.

### 2. stb_sprintf for printf in freestanding

`stb_sprintf.h` provides `stbsp_vsnprintf`/`stbsp_snprintf` that work without libc. These are aliased to `vsnprintf`/`snprintf` in `src/libc/` so EasyLogger (and any future code) can call them directly. The implementation file uses a generic name (e.g. `sk_printf.c`), not one reflecting "stb".

### 3. Git submodules (Approach A)

- `3rd/easylogger` — EasyLogger source
- `3rd/stb` — stb single-header libraries

Both managed as git submodules, consistent with the project's existing pattern for 3rd-party dependencies.

### 4. Async and buffered modes

EasyLogger supports three output modes:
- **Synchronous** (default): `elog_port_output()` called inline. Used for normal kernel logging.
- **Async** (`elog_async.c`): Logs queued to a ring buffer, flushed by a background task. Useful once the task subsystem is running — defers I/O from hot paths.
- **Buffered** (`elog_buf.c`): Logs accumulated in a buffer, flushed periodically or on demand. Useful for batch output to storage.

The design enables all three by **not** disabling `ELOG_ASYNC_OUTPUT_ENABLE` or `ELOG_BUF_OUTPUT_ENABLE` at the config level. Instead:
- Synchronous mode is the initial/default mode at boot.
- Async mode can be activated after the task subsystem initializes (future work — port layer provides the thread/semaphore hooks).
- Buffered mode can be activated when a persistent log sink is available (future work).

The CMake config target defines the feature macros. The port layer in `kernel_log.cpp` provides stub implementations for async/buf hooks initially, with `// @todo` markers for future activation.

### 5. Log-related code location

All log-related code lives in two files:
- **`src/include/kernel_log.hpp`** — Stripped to: raw_* (lock-free per-CPU) log classes, EasyLogger header includes, log initialization function declaration, and any thin C++ convenience wrappers.
- **`kernel_log.cpp`** (new, in `src/`) — Contains: `elog_port_*` port layer implementation, kernel log initialization function (`KernelLogInit()` wrapping `elog_init()` + `elog_start()`), and any C++ glue.

No separate `src/elog/` directory or files.

### 6. EasyLogger config via CMake interface target

Instead of a standalone `elog_cfg.h` file, EasyLogger configuration macros are managed through a CMake interface library target in `cmake/compile_config.cmake`. This target (`elog_config`) defines all `ELOG_*` macros via `TARGET_COMPILE_DEFINITIONS`, and any code that includes `elog.h` links against it.

EasyLogger's `elog.h` includes `elog_cfg.h` — to satisfy this, a minimal `elog_cfg.h` shim is placed in the include path that simply declares the macros are provided externally (or is empty, since the macros are already defined via `-D` flags from CMake). The shim lives in `src/include/` alongside `kernel_log.hpp`.

Key config macros:
```
ELOG_OUTPUT_ENABLE          = 1
ELOG_OUTPUT_LVL             = ELOG_LVL_VERBOSE (5) — runtime filtering, not compile-time
ELOG_ASSERT_ENABLE          = 1
ELOG_LINE_BUF_SIZE          = 512
ELOG_LINE_NUM_MAX_LEN       = 5
ELOG_FILTER_TAG_MAX_LEN     = 30
ELOG_FILTER_KW_MAX_LEN      = 16
ELOG_FILTER_TAG_LVL_MAX_NUM = 5
ELOG_NEWLINE_SIGN            = "\\n"
ELOG_COLOR_ENABLE            = 1
ELOG_ASYNC_OUTPUT_ENABLE     = 1
ELOG_BUF_OUTPUT_ENABLE       = 1
ELOG_ASYNC_LINE_BUF_SIZE     = 512
ELOG_ASYNC_OUTPUT_BUF_SIZE   = (512 * 10)
ELOG_ASYNC_POLL_GET_LOG_BUF_SIZE = 512
ELOG_BUF_OUTPUT_BUF_SIZE     = 512
```

### 7. LOG_TAG per module via CMakeLists.txt

Each module's `CMakeLists.txt` defines `LOG_TAG` using `TARGET_COMPILE_DEFINITIONS`:

```cmake
TARGET_COMPILE_DEFINITIONS (task INTERFACE LOG_TAG="task")
TARGET_COMPILE_DEFINITIONS (device INTERFACE LOG_TAG="device")
TARGET_COMPILE_DEFINITIONS (filesystem INTERFACE LOG_TAG="filesystem")
# ... etc.
```

Source files do **not** `#define LOG_TAG`. The tag is injected at build time per compilation unit. For `src/CMakeLists.txt` top-level sources (main.cpp, syscall.cpp), the tag is set on the executable target.

Sub-modules (e.g., `ns16550a`, `virtio`, `vfs`) define their own more specific tags.

### 8. Initialization wrapped in kernel function

```cpp
/// @brief Initialize the kernel logging subsystem
/// @pre   Serial output (sk_print_str) is functional
/// @post  elog_*() macros are safe to call
auto KernelLogInit() -> Expected<void>;
```

This function encapsulates `elog_init()` + `elog_start()` + any additional kernel-specific setup (e.g., setting default filter levels). Called from the boot sequence in `src/main.cpp`.

### 9. Preserving raw_* API

The lock-free per-CPU `LogLineRaw` / `LogStreamRaw` classes remain in `kernel_log.hpp` for interrupt/exception contexts where EasyLogger's lock-based path is unsafe. The raw_* API continues to use ETL string streams and per-CPU buffers, unchanged from the current implementation.

Migration mapping:
| Current | New |
|---------|-----|
| `klog::info << "msg" << val;` | `elog_i(LOG_TAG, "msg %d", val);` |
| `klog::warn << "msg";` | `elog_w(LOG_TAG, "msg");` |
| `klog::err << "msg";` | `elog_e(LOG_TAG, "msg");` |
| `klog::debug() << "msg";` | `elog_d(LOG_TAG, "msg");` |
| `klog::raw_info << "msg";` | `klog::raw_info << "msg";` (unchanged) |
| `klog::raw_warn << "msg";` | `klog::raw_warn << "msg";` (unchanged) |
| `klog::raw_err << "msg";` | `klog::raw_err << "msg";` (unchanged) |
| `klog::raw_debug() << "msg";` | `klog::raw_debug() << "msg";` (unchanged) |
| `klog::DebugBlob(data, size)` | `elog_hexdump(LOG_TAG, 16, data, size)` or keep custom |

## Component Layout

```
3rd/easylogger/              # Git submodule
3rd/stb/                     # Git submodule

src/libc/
  sk_printf.c                # NEW — stb_sprintf impl + vsnprintf/snprintf wrappers
  include/
    sk_printf.h              # NEW — vsnprintf/snprintf declarations

src/include/
  kernel_log.hpp             # MODIFIED — raw_* only + elog includes + KernelLogInit decl
  elog_cfg.h                 # NEW — minimal shim (macros provided by CMake)

src/
  kernel_log.cpp             # NEW — elog_port_* + KernelLogInit()

cmake/
  compile_config.cmake       # MODIFIED — add elog_config interface target
```

## Output Chain

```
elog_i(LOG_TAG, "fmt", ...)
  → vsnprintf (stb_sprintf)
  → elog_port_output(log, size)
    → sk_print_str(log)
      → etl_putchar() per char
        → arch serial (COM1/PL011/SBI)
```

## Build Integration

```
kernel_link_libraries
  ├── ... (existing)
  ├── elog_config          # NEW — ELOG_* macro definitions
  └── elog_lib             # NEW — easylogger source files

libc
  └── sk_printf.c          # NEW — vsnprintf/snprintf from stb_sprintf
```

The `elog_lib` target is an INTERFACE library (matching project convention) that:
1. Adds `3rd/easylogger/easylogger/inc` to include path
2. Adds `elog.c`, `elog_utils.c`, `elog_async.c`, `elog_buf.c` as INTERFACE sources
3. Links `elog_config` for macro definitions
4. Links `libc` for `vsnprintf`/`snprintf` access

## Constraints

- Freestanding: no libc, no heap before memory init, no exceptions/RTTI
- stb_sprintf with `STB_SPRINTF_NOFLOAT` (no floating-point in freestanding)
- Google C++ style, `.clang-format` enforced
- `Expected<T>` for error returns
- CMake: UPPERCASE commands, 4-space indent, 80-char lines
