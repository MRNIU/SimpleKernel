# Abandon %-style printf — Full Migration to Direct Emission

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove nanoprintf and the entire `sk_printf`/`sk_snprintf`/`sk_vsnprintf` API. Replace every call site with direct `sk_putchar` emission via `__always_inline` helpers in `sk_stdio.h`. `BmallocLogger` becomes a silent no-op.

**Architecture:**
- `sk_stdio.h` keeps only `sk_putchar` (the hardware output primitive) plus four `__always_inline` emit helpers (`sk_emit_str`, `sk_emit_sint`, `sk_emit_uint`, `sk_emit_hex`) — available everywhere via the existing `kstd_cstdio` re-export header.
- `sk_stdio.c` shrinks to just `dummy_putchar` + `sk_putchar` assignment.
- `kernel_log.hpp`'s `FormatArg` overloads call the helpers directly; `LogEmit`/`LogLine` do the same for their prefix/suffix strings.
- All other callers (`spinlock.hpp`, `kstd_iostream.cpp`, `kstd_libcxx.cpp`, `schedule.cpp`, `syscall.cpp`) are updated in-place — each change is 1–5 lines.
- `BmallocLogger::operator()` ignores all arguments and returns 0.

**Tech Stack:** C23, C++23 freestanding, `sk_putchar`, `__always_inline`.

---

### Task 1: Rewrite `sk_stdio.h` — add emit helpers, remove printf API

**Files:**
- Modify: `src/libc/include/sk_stdio.h`

**Step 1: Replace the entire file with:**

```c
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
#define SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Output primitive — set by each arch's EarlyConsole constructor */
extern void (*sk_putchar)(int c, void* ctx);

/* ── Freestanding emit helpers (always-inline, no format strings) ────────── */

/** Emit a NUL-terminated C string character by character. NULL-safe. */
static __attribute__((always_inline)) inline void sk_emit_str(const char* s) {
  if (!s) { s = "(null)"; }
  while (*s) {
    sk_putchar((unsigned char)*s, NULL);
    ++s;
  }
}

/** Emit a signed 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_sint(long long v) {
  char buf[20];
  int n = 0;
  if (v < 0) {
    sk_putchar('-', NULL);
    /* avoid UB on LLONG_MIN: cast before negation */
    unsigned long long uv = (unsigned long long)(-(v + 1)) + 1ULL;
    if (uv == 0) { sk_putchar('0', NULL); return; }
    while (uv) { buf[n++] = (char)('0' + (int)(uv % 10)); uv /= 10; }
  } else {
    unsigned long long uv = (unsigned long long)v;
    if (uv == 0) { sk_putchar('0', NULL); return; }
    while (uv) { buf[n++] = (char)('0' + (int)(uv % 10)); uv /= 10; }
  }
  for (int i = n - 1; i >= 0; i--) sk_putchar(buf[i], NULL);
}

/** Emit an unsigned 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_uint(
    unsigned long long v) {
  char buf[20];
  int n = 0;
  if (v == 0) { sk_putchar('0', NULL); return; }
  while (v) { buf[n++] = (char)('0' + (int)(v % 10)); v /= 10; }
  for (int i = n - 1; i >= 0; i--) sk_putchar(buf[i], NULL);
}

/**
 * Emit an unsigned 64-bit integer in hexadecimal.
 * @param v     Value to emit.
 * @param width Minimum digit count (zero-padded with '0').
 * @param upper Non-zero → uppercase A-F.
 */
static __attribute__((always_inline)) inline void sk_emit_hex(
    unsigned long long v, int width, int upper) {
  static const char kLo[] = "0123456789abcdef";
  static const char kHi[] = "0123456789ABCDEF";
  const char* digits = upper ? kHi : kLo;
  char buf[16];
  int n = 0;
  if (v == 0) {
    buf[n++] = '0';
  } else {
    while (v) { buf[n++] = digits[v & 0xf]; v >>= 4; }
  }
  for (int i = n; i < width; i++) sk_putchar('0', NULL);
  for (int i = n - 1; i >= 0; i--) sk_putchar(buf[i], NULL);
}

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
```

**Step 2: Verify no syntax errors by re-reading the file after writing.**

---

### Task 2: Rewrite `sk_stdio.c` — strip to putchar only

**Files:**
- Modify: `src/libc/sk_stdio.c`

**Step 1: Replace the entire file with:**

```c
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_stdio.h"

#ifdef __cplusplus
extern "C" {
#endif

static void dummy_putchar(int c, void* ctx) {
  (void)c;
  (void)ctx;
}

void (*sk_putchar)(int, void*) = dummy_putchar;

#ifdef __cplusplus
}
#endif
```

---

### Task 3: Update `kernel_log.hpp` — replace all `sk_printf` with emit helpers

**Files:**
- Modify: `src/include/kernel_log.hpp`

This file has three clusters of `sk_printf` calls. Each cluster is listed below with its exact replacement.

#### Cluster A — `FormatArg` overloads (lines 138–164)

Replace:
```cpp
__always_inline void FormatArg(long long val) { sk_printf("%lld", val); }
__always_inline void FormatArg(long val) { sk_printf("%ld", val); }
__always_inline void FormatArg(int val) { sk_printf("%d", val); }

__always_inline void FormatArg(unsigned long long val) {
  sk_printf("%llu", val);
}
__always_inline void FormatArg(unsigned long val) { sk_printf("%lu", val); }
__always_inline void FormatArg(unsigned int val) { sk_printf("%u", val); }

__always_inline void FormatArg(bool val) {
  sk_printf("%s", val ? "true" : "false");
}

__always_inline void FormatArg(char val) { sk_putchar(val, nullptr); }

__always_inline void FormatArg(const char* val) {
  sk_printf("%s", val ? val : "(null)");
}

__always_inline void FormatArg(const void* val) { sk_printf("%p", val); }
__always_inline void FormatArg(void* val) {
  FormatArg(static_cast<const void*>(val));
}
```

With:
```cpp
__always_inline void FormatArg(long long val) { sk_emit_sint(val); }
__always_inline void FormatArg(long val) { sk_emit_sint((long long)val); }
__always_inline void FormatArg(int val) { sk_emit_sint((long long)val); }

__always_inline void FormatArg(unsigned long long val) { sk_emit_uint(val); }
__always_inline void FormatArg(unsigned long val) {
  sk_emit_uint((unsigned long long)val);
}
__always_inline void FormatArg(unsigned int val) {
  sk_emit_uint((unsigned long long)val);
}

__always_inline void FormatArg(bool val) {
  sk_emit_str(val ? "true" : "false");
}

__always_inline void FormatArg(char val) { sk_putchar(val, nullptr); }

__always_inline void FormatArg(const char* val) { sk_emit_str(val); }

__always_inline void FormatArg(const void* val) {
  sk_putchar('0', nullptr);
  sk_putchar('x', nullptr);
  sk_emit_hex((unsigned long long)(uintptr_t)val,
              (int)(sizeof(void*) * 2), /*upper=*/0);
}
__always_inline void FormatArg(void* val) {
  FormatArg(static_cast<const void*>(val));
}
```

Note: `uintptr_t` requires `#include <cstdint>` — already present in the file (line 12: `#include <cstdint>`). ✓

#### Cluster B — `LogLine` constructor/destructor and `operator<<` (lines ~241–310)

Replace every `sk_printf` in `LogLine`:

| Old | New |
|-----|-----|
| `sk_printf("LogLine: Failed to acquire lock: %s\n", err.message())` | `sk_emit_str("LogLine: Failed to acquire lock: "); sk_emit_str(err.message()); sk_putchar('\n', nullptr);` |
| `sk_printf("%s[%ld]", kLogColors[Level], cpu_io::GetCurrentCoreId())` | `sk_emit_str(kLogColors[Level]); sk_putchar('[', nullptr); sk_emit_sint((long long)cpu_io::GetCurrentCoreId()); sk_putchar(']', nullptr);` |
| `sk_printf("%s", kReset)` | `sk_emit_str(kReset)` |
| `sk_printf("LogLine: Failed to release lock: %s\n", err.message())` | `sk_emit_str("LogLine: Failed to release lock: "); sk_emit_str(err.message()); sk_putchar('\n', nullptr);` |
| `sk_printf("%lld", static_cast<long long>(val))` | `sk_emit_sint(static_cast<long long>(val))` |
| `sk_printf("%llu", static_cast<unsigned long long>(val))` | `sk_emit_uint(static_cast<unsigned long long>(val))` |
| `sk_printf("%s", val ? val : "(null)")` | `sk_emit_str(val)` |
| `sk_printf("%s", val ? "true" : "false")` | `sk_emit_str(val ? "true" : "false")` |
| `sk_printf("%p", val)` | `sk_putchar('0', nullptr); sk_putchar('x', nullptr); sk_emit_hex((unsigned long long)(uintptr_t)val, (int)(sizeof(void*)*2), 0);` |

#### Cluster C — `LogEmit` function (~lines 357–364)

Replace:
```cpp
sk_printf("%s[%ld]", kLogColors[Level], cpu_io::GetCurrentCoreId());
if constexpr (Level == LogLevel::kDebug) {
  sk_printf("[%s:%u %s] ", location.file_name(), location.line(),
            location.function_name());
}
LogFormat(fmt.str, static_cast<Args&&>(args)...);
sk_printf("%s", kReset);
```

With:
```cpp
sk_emit_str(kLogColors[Level]);
sk_putchar('[', nullptr);
sk_emit_sint((long long)cpu_io::GetCurrentCoreId());
sk_putchar(']', nullptr);
if constexpr (Level == LogLevel::kDebug) {
  sk_putchar('[', nullptr);
  sk_emit_str(location.file_name());
  sk_putchar(':', nullptr);
  sk_emit_uint((unsigned long long)location.line());
  sk_putchar(' ', nullptr);
  sk_emit_str(location.function_name());
  sk_putchar(']', nullptr);
  sk_putchar(' ', nullptr);
}
LogFormat(fmt.str, static_cast<Args&&>(args)...);
sk_emit_str(kReset);
```

#### Cluster D — `DebugBlob` function (~lines 397–402)

Replace:
```cpp
sk_printf("%s[%ld] ", detail::kMagenta, cpu_io::GetCurrentCoreId());
for (size_t i = 0; i < size; i++) {
  sk_printf("0x%02X ", reinterpret_cast<const uint8_t*>(data)[i]);
}
sk_printf("%s\n", detail::kReset);
```

With:
```cpp
sk_emit_str(detail::kMagenta);
sk_putchar('[', nullptr);
sk_emit_sint((long long)cpu_io::GetCurrentCoreId());
sk_putchar(']', nullptr);
sk_putchar(' ', nullptr);
for (size_t i = 0; i < size; i++) {
  sk_putchar('0', nullptr);
  sk_putchar('x', nullptr);
  sk_emit_hex((unsigned long long)reinterpret_cast<const uint8_t*>(data)[i],
              /*width=*/2, /*upper=*/1);
  sk_putchar(' ', nullptr);
}
sk_emit_str(detail::kReset);
sk_putchar('\n', nullptr);
```

**Step 2: Remove `#include "kstd_cstdio"` from kernel_log.hpp and replace with `#include "sk_stdio.h"`**

(They're equivalent since `kstd_cstdio` is just `#include "sk_stdio.h"`, but use the C header directly here for clarity. Actually, keep `kstd_cstdio` since that's the existing convention — it already pulls in `sk_stdio.h`.)

---

### Task 4: Update `spinlock.hpp`

**Files:**
- Modify: `src/include/spinlock.hpp`

**Context:** `spinlock.hpp` includes `kstd_cstdio` → `sk_stdio.h`, so `sk_emit_str` is available.

Replace all `sk_printf` calls:

| Old | New |
|-----|-----|
| `sk_printf("spinlock %s recursive lock detected.\n", name_)` | `sk_emit_str("spinlock "); sk_emit_str(name_); sk_emit_str(" recursive lock detected.\n");` |
| `sk_printf("spinlock %s IsLockedByCurrentCore == false.\n", name_)` | `sk_emit_str("spinlock "); sk_emit_str(name_); sk_emit_str(" IsLockedByCurrentCore == false.\n");` |
| `sk_printf("LockGuard: Failed to acquire lock: %s\n", err.message())` | `sk_emit_str("LockGuard: Failed to acquire lock: "); sk_emit_str(err.message()); sk_putchar('\n', nullptr);` |
| `sk_printf("LockGuard: Failed to release lock: %s\n", err.message())` | `sk_emit_str("LockGuard: Failed to release lock: "); sk_emit_str(err.message()); sk_putchar('\n', nullptr);` |

---

### Task 5: Update `kstd_iostream.cpp`

**Files:**
- Modify: `src/libcxx/kstd_iostream.cpp`

**Context:** Already includes `kstd_cstdio` → `sk_stdio.h`. All `sk_printf` calls are simple integer/string conversions.

Replace each `sk_printf` call with the appropriate emit helper. Full replacement of function bodies:

```cpp
auto ostream::operator<<(int8_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}
auto ostream::operator<<(uint8_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}
auto ostream::operator<<(const char* val) -> ostream& {
  sk_emit_str(val);
  return *this;
}
auto ostream::operator<<(int16_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}
auto ostream::operator<<(uint16_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}
auto ostream::operator<<(int32_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}
auto ostream::operator<<(uint32_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}
auto ostream::operator<<(int64_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}
auto ostream::operator<<(uint64_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}
```

Remove the `#include <cstdint>` include if it becomes unused (check); it may still be needed for the type declarations.

---

### Task 6: Update `kstd_libcxx.cpp` — fix `__assert_fail`

**Files:**
- Modify: `src/libcxx/kstd_libcxx.cpp`

The file does not include `sk_stdio.h` directly; it includes `kernel_log.hpp` (which pulls in `kstd_cstdio` → `sk_stdio.h`). So `sk_emit_str`/`sk_emit_uint` are available.

Replace:
```cpp
sk_printf("\n[ASSERT FAILED] %s:%u in %s\n Expression: %s\n", file, line,
          function, assertion);
```

With:
```cpp
sk_emit_str("\n[ASSERT FAILED] ");
sk_emit_str(file);
sk_putchar(':', nullptr);
sk_emit_uint((unsigned long long)line);
sk_emit_str(" in ");
sk_emit_str(function);
sk_emit_str("\n Expression: ");
sk_emit_str(assertion);
sk_putchar('\n', nullptr);
```

---

### Task 7: Update `schedule.cpp`

**Files:**
- Modify: `src/task/schedule.cpp`

Read the file first to see exact lines, then replace the `sk_printf` calls with `klog::Err`:

```cpp
// Old:
sk_printf("Schedule: Failed to acquire lock: %s\n", err.message());
// New:
klog::Err("Schedule: Failed to acquire lock: {}\n", err.message());

// Old:
sk_printf("Schedule: Failed to release lock: %s\n", err.message());
// New:
klog::Err("Schedule: Failed to release lock: {}\n", err.message());
```

Verify `schedule.cpp` already includes `kernel_log.hpp` (or add it). Remove `#include "sk_stdio.h"` / `kstd_cstdio` from that file if it was only there for `sk_printf`.

---

### Task 8: Update `syscall.cpp`

**Files:**
- Modify: `src/syscall.cpp`

Replace:
```cpp
sk_printf("%c", buf[i]);
```

With:
```cpp
sk_putchar(buf[i], nullptr);
```

Verify `syscall.cpp` still needs `sk_stdio.h` / `kstd_cstdio` for `sk_putchar`. If not (i.e., the only usage was `sk_printf`), remove the include and use `sk_putchar` via whatever header already provides it, or keep `kstd_cstdio`.

---

### Task 9: Update `memory.cpp` — empty `BmallocLogger`

**Files:**
- Modify: `src/memory/memory.cpp`

Replace the entire `BmallocLogger` struct:

```cpp
// Old:
struct BmallocLogger {
  int operator()(const char* format, ...) const {
    va_list args;
    va_start(args, format);
    char buffer[1024];
    int result = sk_vsnprintf(buffer, sizeof(buffer), format, args);
    va_end(args);
    klog::Err("{}", buffer);
    return result;
  }
};
```

With:
```cpp
struct BmallocLogger {
  int operator()(const char* format, ...) const {
    (void)format;
    return 0;
  }
};
```

Remove the unused includes that are no longer needed (only if nothing else in the file uses them). In particular, the file includes `"sk_stdlib.h"` — check if that's still needed. Do NOT remove includes that are used by other code in the file.

---

### Task 10: Remove nanoprintf from CMake

**Files:**
- Modify: `cmake/3rd.cmake` — delete the nanoprintf block (lines 61–75)
- Modify: `cmake/compile_config.cmake` — delete `nanoprintf-lib` from the link list (line 163)

#### `cmake/3rd.cmake` — delete lines 61–75:

```cmake
# https://github.com/charlesnicholson/nanoprintf.git
SET (nanoprintf_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/nanoprintf)
SET (nanoprintf_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/nanoprintf)
ADD_CUSTOM_TARGET (
    nanoprintf
    ...
TARGET_INCLUDE_DIRECTORIES (nanoprintf-lib INTERFACE ${nanoprintf_BINARY_DIR})
```

#### `cmake/compile_config.cmake` — delete line 163:

```cmake
              nanoprintf-lib
```

---

### Task 11: Remove nanoprintf git submodule

**Step 1:**
```bash
git submodule deinit -f 3rd/nanoprintf
git rm 3rd/nanoprintf
rm -rf .git/modules/3rd/nanoprintf
```

**Step 2: Verify:**
```bash
git status   # should show modified .gitmodules, deleted 3rd/nanoprintf
```

---

### Task 12: Build and verify

**Step 1: Clean configure**
```bash
cmake --preset build_riscv64
```
Expected: succeeds, no mention of nanoprintf.

**Step 2: Build**
```bash
cd build_riscv64 && make SimpleKernel -j$(nproc) 2>&1 | tail -20
```
Expected: exit code 0.

**Step 3: Run unit tests**
```bash
make unit-test
```
Expected: all pass.

**If build fails with "undefined reference to sk_printf":** a file still includes `sk_printf`. Search:
```bash
grep -rn "sk_printf\|sk_snprintf\|sk_vsnprintf" src/
```
Fix any remaining callsites.

**If build fails with "no member named sk_emit_str":** the file isn't including `sk_stdio.h`. Add `#include "sk_stdio.h"` or `#include "kstd_cstdio"`.

---

### Task 13: Commit

```bash
git add src/libc/include/sk_stdio.h \
        src/libc/sk_stdio.c \
        src/include/kernel_log.hpp \
        src/include/spinlock.hpp \
        src/libcxx/kstd_iostream.cpp \
        src/libcxx/kstd_libcxx.cpp \
        src/task/schedule.cpp \
        src/syscall.cpp \
        src/memory/memory.cpp \
        cmake/3rd.cmake \
        cmake/compile_config.cmake \
        .gitmodules
git add -u   # picks up deleted 3rd/nanoprintf

git commit --signoff -m "refactor(libc): remove nanoprintf, abandon %-style format API

Replace sk_printf/sk_snprintf/sk_vsnprintf with four __always_inline
emit helpers (sk_emit_str, sk_emit_sint, sk_emit_uint, sk_emit_hex)
in sk_stdio.h. All callers migrated to direct emission.

BmallocLogger becomes a silent no-op since bmalloc's va_list-based
logger interface cannot be migrated to {}-style.

- Remove 3rd/nanoprintf git submodule
- Remove nanoprintf-lib CMake target and link dependency
- sk_putchar remains as the sole public output primitive"
```

---

## Migration Reference

### Helper signatures (added to `sk_stdio.h`)

```c
void sk_emit_str(const char* s);                              // null-safe
void sk_emit_sint(long long v);                               // signed decimal
void sk_emit_uint(unsigned long long v);                      // unsigned decimal
void sk_emit_hex(unsigned long long v, int width, int upper); // hex, zero-padded
```

### Common substitution patterns

| Old `sk_printf` | New |
|---|---|
| `sk_printf("%d", val)` | `sk_emit_sint((long long)val)` |
| `sk_printf("%u", val)` | `sk_emit_uint((unsigned long long)val)` |
| `sk_printf("%lld", val)` | `sk_emit_sint(val)` |
| `sk_printf("%s", str)` | `sk_emit_str(str)` |
| `sk_printf("%p", ptr)` | `sk_putchar('0',0); sk_putchar('x',0); sk_emit_hex((ull)(uintptr_t)ptr, sizeof(void*)*2, 0)` |
| `sk_printf("%02X", byte)` | `sk_emit_hex(byte, 2, 1)` |
| `sk_printf("%s%s", a, b)` | `sk_emit_str(a); sk_emit_str(b)` |
| `sk_printf("Schedule: ... %s\n", msg)` | `klog::Err("Schedule: ... {}\n", msg)` |
| `sk_printf("%c", c)` | `sk_putchar(c, nullptr)` |
