# Remove nanoprintf — Replace with ETL + hand-rolled va_list fallback

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate the nanoprintf git submodule entirely; re-implement `sk_printf`, `sk_snprintf`, and `sk_vsnprintf` using `etl::print` / `etl::format_to_n` from the already-present ETL dependency, with a minimal self-contained `va_list` path for the one caller that needs it.

**Architecture:**
- `etl::print` (via `etl/print.h`) writes characters through the `etl_putchar` hook — we bridge that to `sk_putchar`.
- `etl::format_to_n` (via `etl/format.h`) writes into a `char*` buffer via a custom output iterator — this replaces `npf_vsnprintf`.
- The only remaining `va_list` call site is `memory.cpp`'s `BmallocLogger::operator()`, which already passes a `%s`-only string; we keep `sk_vsnprintf` as a thin shim that formats via a local stack buffer using a custom mini `%s`/`%d` loop — no nanoprintf required.
- All `sk_printf` callers in `kernel_log.hpp`, `kstd_iostream.cpp`, etc. continue to compile unchanged; the only difference is the backing implementation.

**Tech Stack:** C23, C++23 freestanding, ETL (`etl/print.h`, `etl/format.h`), existing `sk_putchar` / `sk_stdio.h` API.

---

### Task 1: Implement `etl_putchar` bridge and rewrite `sk_stdio.c`

**Files:**
- Modify: `src/libc/sk_stdio.c`

**Context:**
The current file uses `npf_vpprintf` and `npf_vsnprintf`. We replace the entire body:
- `sk_printf(fmt, ...)` → still uses `va_list` / `%`-style format. The ETL `etl::print` uses `{}`-style and is C++ only. Since `sk_stdio.c` is C and all callers use `%`-style, we cannot directly use `etl::print` here without changing all callers. Instead, we implement a minimal `%`-format engine in C (replacing nanoprintf), OR we convert the file to C++ and use ETL.

**Decision — convert `sk_stdio.c` → `sk_stdio.cpp`:** This lets us call ETL. However, all existing `%`-style callers would then need to move to `{}`-style, which is large scope. **Better approach:** keep `%`-style API surface, but implement the body ourselves without nanoprintf.

**Simpler approach (minimal-change):** Write a tiny freestanding `%`-format engine in C (≈ 80 lines) that covers the subset actually used across the codebase:
- `%d`, `%ld`, `%lld` — signed decimal
- `%u`, `%lu`, `%llu` — unsigned decimal
- `%x`, `%X`, `%02X`, `%08X` — hex with optional width/zero-pad
- `%s` — string
- `%c` — character
- `%p` — pointer (hex with `0x` prefix)
- `%%` — literal percent

This removes the nanoprintf dependency with zero changes to callers.

**Step 1: Write the new `src/libc/sk_stdio.c`**

Replace the entire file with the following content (keep copyright header, keep `sk_putchar` / `dummy_putchar`, add mini format engine):

```c
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_stdio.h"

#ifdef __cplusplus
extern "C" {
#endif

static void dummy_putchar(int c, void* ctx) { (void)c; (void)ctx; }

void (*sk_putchar)(int, void*) = dummy_putchar;

// ── Internal helpers ─────────────────────────────────────────────────────────

static void emit(int c) { sk_putchar(c, NULL); }

static void emit_str(const char* s) {
  if (!s) s = "(null)";
  while (*s) emit((unsigned char)*s++);
}

static void emit_uint_base(unsigned long long val, int base, int upper,
                           int min_width, char pad) {
  static const char kLower[] = "0123456789abcdef";
  static const char kUpper[] = "0123456789ABCDEF";
  const char* digits = upper ? kUpper : kLower;
  char buf[66];
  int len = 0;
  if (val == 0) {
    buf[len++] = '0';
  } else {
    unsigned long long v = val;
    while (v) {
      buf[len++] = digits[v % (unsigned)base];
      v /= (unsigned)base;
    }
  }
  // Pad with pad character to min_width
  for (int i = len; i < min_width; i++) emit(pad);
  // Emit digits in reverse
  for (int i = len - 1; i >= 0; i--) emit(buf[i]);
}

// ── Core va_list formatter ────────────────────────────────────────────────────

static int vformat_putchar(const char* fmt, va_list ap) {
  int count = 0;
  for (; *fmt; ++fmt) {
    if (*fmt != '%') {
      emit((unsigned char)*fmt);
      count++;
      continue;
    }
    ++fmt;  // skip '%'
    // Flags
    char pad = ' ';
    if (*fmt == '0') { pad = '0'; ++fmt; }
    // Width
    int width = 0;
    while (*fmt >= '0' && *fmt <= '9') { width = width * 10 + (*fmt++ - '0'); }
    // Length modifier
    int is_long = 0, is_longlong = 0;
    if (*fmt == 'l') {
      ++fmt;
      if (*fmt == 'l') { is_longlong = 1; ++fmt; }
      else { is_long = 1; }
    }
    // Conversion
    switch (*fmt) {
      case 'd': case 'i': {
        long long v = is_longlong ? va_arg(ap, long long)
                    : is_long    ? va_arg(ap, long)
                                 : va_arg(ap, int);
        if (v < 0) { emit('-'); count++; v = -v; }
        emit_uint_base((unsigned long long)v, 10, 0, width, pad);
        break;
      }
      case 'u': {
        unsigned long long v = is_longlong ? va_arg(ap, unsigned long long)
                             : is_long     ? va_arg(ap, unsigned long)
                                           : va_arg(ap, unsigned int);
        emit_uint_base(v, 10, 0, width, pad);
        break;
      }
      case 'x': {
        unsigned long long v = is_longlong ? va_arg(ap, unsigned long long)
                             : is_long     ? va_arg(ap, unsigned long)
                                           : va_arg(ap, unsigned int);
        if (*fmt == 'x' && width == 0 && *(fmt-1) == '#') {
          emit('0'); emit('x'); count += 2;
        }
        emit_uint_base(v, 16, 0, width, pad);
        break;
      }
      case 'X': {
        unsigned long long v = is_longlong ? va_arg(ap, unsigned long long)
                             : is_long     ? va_arg(ap, unsigned long)
                                           : va_arg(ap, unsigned int);
        emit_uint_base(v, 16, 1, width, pad);
        break;
      }
      case 'p': {
        uintptr_t v = (uintptr_t)va_arg(ap, void*);
        emit('0'); emit('x'); count += 2;
        emit_uint_base((unsigned long long)v, 16, 0,
                       (int)(sizeof(void*) * 2), '0');
        count += (int)(sizeof(void*) * 2);
        continue;  // count already updated, skip bottom ++count
      }
      case 's': {
        const char* s = va_arg(ap, const char*);
        emit_str(s);
        break;
      }
      case 'c': {
        emit((unsigned char)va_arg(ap, int));
        break;
      }
      case '%': {
        emit('%');
        break;
      }
      default:
        emit('%'); emit((unsigned char)*fmt);
        break;
    }
    count++;
  }
  return count;
}

// ── Public API ───────────────────────────────────────────────────────────────

int sk_printf(const char* format, ...) {
  va_list args;
  va_start(args, format);
  const int ret = vformat_putchar(format, args);
  va_end(args);
  return ret;
}

// Buffer output iterator for snprintf-style formatting
typedef struct {
  char* buf;
  size_t capacity;
  size_t written;  // Total chars that WOULD be written (may exceed capacity)
} SnprintfState;

static void snprintf_emit(SnprintfState* st, char c) {
  if (st->written < st->capacity - 1) {
    st->buf[st->written] = c;
  }
  st->written++;
}

// Minimal buffer-writing vformat (mirrors vformat_putchar but writes to buffer)
static int vformat_buf(char* buffer, size_t bufsz, const char* fmt, va_list ap) {
  if (!buffer || bufsz == 0) return 0;
  SnprintfState st = {buffer, bufsz, 0};
  // Re-use same logic but write to buffer. We duplicate to avoid function
  // pointer overhead in freestanding context.
  // For simplicity, call vformat_putchar with a thread-local putchar override
  // is not safe in SMP. Instead, inline the buffer logic here.
  // (Implementation: same switch, calls snprintf_emit(&st, c))

  for (; *fmt; ++fmt) {
    if (*fmt != '%') {
      snprintf_emit(&st, (char)(unsigned char)*fmt);
      continue;
    }
    ++fmt;
    char pad = ' ';
    if (*fmt == '0') { pad = '0'; ++fmt; }
    int width = 0;
    while (*fmt >= '0' && *fmt <= '9') { width = width * 10 + (*fmt++ - '0'); }
    int is_long = 0, is_longlong = 0;
    if (*fmt == 'l') {
      ++fmt;
      if (*fmt == 'l') { is_longlong = 1; ++fmt; }
      else { is_long = 1; }
    }

    // Emit number to a temp buf, then copy to snprintf state
    char tmp[66]; int tlen = 0;
    int is_signed_neg = 0;

    switch (*fmt) {
      case 'd': case 'i': {
        long long v = is_longlong ? va_arg(ap, long long)
                    : is_long    ? va_arg(ap, long)
                                 : va_arg(ap, int);
        if (v < 0) { is_signed_neg = 1; v = -v; }
        unsigned long long uv = (unsigned long long)v;
        if (uv == 0) { tmp[tlen++] = '0'; }
        else { while (uv) { tmp[tlen++] = '0' + (int)(uv % 10); uv /= 10; } }
        if (is_signed_neg) snprintf_emit(&st, '-');
        for (int i = tlen; i < width; i++) snprintf_emit(&st, pad);
        for (int i = tlen - 1; i >= 0; i--) snprintf_emit(&st, tmp[i]);
        break;
      }
      case 'u': {
        unsigned long long v = is_longlong ? va_arg(ap, unsigned long long)
                             : is_long     ? va_arg(ap, unsigned long)
                                           : va_arg(ap, unsigned int);
        if (v == 0) { tmp[tlen++] = '0'; }
        else { while (v) { tmp[tlen++] = '0' + (int)(v % 10); v /= 10; } }
        for (int i = tlen; i < width; i++) snprintf_emit(&st, pad);
        for (int i = tlen - 1; i >= 0; i--) snprintf_emit(&st, tmp[i]);
        break;
      }
      case 'x': case 'X': {
        static const char kLo[] = "0123456789abcdef";
        static const char kHi[] = "0123456789ABCDEF";
        const char* digs = (*fmt == 'X') ? kHi : kLo;
        unsigned long long v = is_longlong ? va_arg(ap, unsigned long long)
                             : is_long     ? va_arg(ap, unsigned long)
                                           : va_arg(ap, unsigned int);
        if (v == 0) { tmp[tlen++] = '0'; }
        else { while (v) { tmp[tlen++] = digs[v & 0xf]; v >>= 4; } }
        for (int i = tlen; i < width; i++) snprintf_emit(&st, pad);
        for (int i = tlen - 1; i >= 0; i--) snprintf_emit(&st, tmp[i]);
        break;
      }
      case 'p': {
        static const char kLo[] = "0123456789abcdef";
        uintptr_t v = (uintptr_t)va_arg(ap, void*);
        snprintf_emit(&st, '0'); snprintf_emit(&st, 'x');
        int ptrsz = (int)(sizeof(void*) * 2);
        for (int i = ptrsz - 1; i >= 0; i--) {
          tmp[i] = kLo[v & 0xf]; v >>= 4;
        }
        for (int i = 0; i < ptrsz; i++) snprintf_emit(&st, tmp[i]);
        break;
      }
      case 's': {
        const char* s = va_arg(ap, const char*);
        if (!s) s = "(null)";
        while (*s) snprintf_emit(&st, *s++);
        break;
      }
      case 'c': {
        snprintf_emit(&st, (char)(unsigned char)va_arg(ap, int));
        break;
      }
      case '%': snprintf_emit(&st, '%'); break;
      default:
        snprintf_emit(&st, '%'); snprintf_emit(&st, (char)(unsigned char)*fmt);
        break;
    }
  }
  // NUL-terminate
  size_t nul_pos = st.written < bufsz ? st.written : bufsz - 1;
  st.buf[nul_pos] = '\0';
  return (int)st.written;
}

int sk_snprintf(char* buffer, size_t bufsz, const char* format, ...) {
  va_list val;
  va_start(val, format);
  int rv = vformat_buf(buffer, bufsz, format, val);
  va_end(val);
  return rv;
}

int sk_vsnprintf(char* buffer, size_t bufsz, const char* format,
                 va_list vlist) {
  return vformat_buf(buffer, bufsz, format, vlist);
}

#ifdef __cplusplus
}
#endif
```

**Step 2: Verify the file has no syntax errors — inspect carefully before saving.**

(The code above is the complete replacement. Ensure `uintptr_t` is available: `sk_stdio.h` must pull in or forward `<stdint.h>`.)

**Step 3: Check `sk_stdio.h` includes `<stdint.h>`**

Read `src/libc/include/sk_stdio.h` to confirm `uintptr_t` is available (comes from `<stdint.h>` or `<stddef.h>`). If not, add `#include <stdint.h>` to `sk_stdio.c` at the top.

Run: `pre-commit run --all-files` (or check with clang-tidy) — not available in freestanding build, so just inspect.

---

### Task 2: Remove nanoprintf from CMake

**Files:**
- Modify: `cmake/3rd.cmake` — delete lines 61–75 (nanoprintf block)
- Modify: `cmake/compile_config.cmake` — delete line 163 (`nanoprintf-lib`)

**Step 1: Delete nanoprintf block in `cmake/3rd.cmake`**

Lines 61–75:
```cmake
# https://github.com/charlesnicholson/nanoprintf.git
SET (nanoprintf_SOURCE_DIR ${CMAKE_SOURCE_DIR}/3rd/nanoprintf)
SET (nanoprintf_BINARY_DIR ${CMAKE_BINARY_DIR}/3rd/nanoprintf)
ADD_CUSTOM_TARGET (
    nanoprintf
    COMMENT "build nanoprintf..."
    # make 时编译
    ALL
    WORKING_DIRECTORY ${nanoprintf_SOURCE_DIR}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${nanoprintf_BINARY_DIR}
    COMMAND ${CMAKE_COMMAND} -E copy ${nanoprintf_SOURCE_DIR}/nanoprintf.h
            ${nanoprintf_BINARY_DIR}/nanoprintf.h)
ADD_LIBRARY (nanoprintf-lib INTERFACE)
ADD_DEPENDENCIES (nanoprintf-lib nanoprintf)
TARGET_INCLUDE_DIRECTORIES (nanoprintf-lib INTERFACE ${nanoprintf_BINARY_DIR})
```

Remove these 15 lines entirely.

**Step 2: Delete `nanoprintf-lib` from `cmake/compile_config.cmake`**

Line 163:
```cmake
              nanoprintf-lib
```

Remove this one line from the `TARGET_LINK_LIBRARIES` block.

**Step 3: Verify CMake syntax is valid** — no dangling `ADD_DEPENDENCIES` or references to `nanoprintf-lib`.

---

### Task 3: Remove nanoprintf git submodule

**Step 1: Deinit and remove submodule**

```bash
git submodule deinit -f 3rd/nanoprintf
git rm 3rd/nanoprintf
```

**Step 2: Optionally delete `.git/modules/3rd/nanoprintf`**

```bash
rm -rf .git/modules/3rd/nanoprintf
```

**Step 3: Verify `3rd/nanoprintf` is gone**

```bash
ls 3rd/nanoprintf  # should fail
git status         # should show modified .gitmodules and deleted 3rd/nanoprintf
```

---

### Task 4: Verify the build compiles without nanoprintf

**Step 1: Run cmake configure**

```bash
cmake --preset build_riscv64
```

Expected: configuration succeeds with no mention of nanoprintf.

**Step 2: Build the kernel**

```bash
cd build_riscv64 && make SimpleKernel -j$(nproc)
```

Expected: exit code 0, no undefined reference errors.

**Step 3: Run unit tests (on host arch)**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make unit-test
```

Expected: all tests pass.

**Step 4: If linker errors appear**

Most likely cause: `sk_stdio.c` was not recompiled (stale cache). Fix:
```bash
rm -rf build_riscv64 && cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel
```

---

### Task 5: Commit

**Step 1: Stage all changes**

```bash
git add src/libc/sk_stdio.c cmake/3rd.cmake cmake/compile_config.cmake .gitmodules
git add -u  # picks up deleted 3rd/nanoprintf
```

**Step 2: Commit with signoff**

```bash
git commit --signoff -m "refactor(libc): remove nanoprintf, replace with self-contained format engine

Replace the nanoprintf single-header dependency with a freestanding
~150-line C format engine in sk_stdio.c that covers all format
specifiers used in the kernel (%d/%u/%x/%X/%s/%c/%p/%%,
with width + zero-padding).

- Remove 3rd/nanoprintf git submodule
- Remove nanoprintf-lib CMake target and link dependency
- sk_printf/sk_snprintf/sk_vsnprintf public API unchanged
- No changes to callers required"
```

---

## Notes

### Format specifiers actually used in the codebase

| Specifier | Locations |
|-----------|-----------|
| `%d` | `kstd_iostream.cpp` (int8/16/32), `kernel_log.hpp` FormatArg(int) |
| `%ld` | `kstd_iostream.cpp` (int64), `kernel_log.hpp` FormatArg(long) |
| `%lld` | `kernel_log.hpp` FormatArg(long long), LogLine::operator<< |
| `%llu` | `kernel_log.hpp` FormatArg(unsigned long long) |
| `%lu` | `kernel_log.hpp` FormatArg(unsigned long) |
| `%u` | `kernel_log.hpp` FormatArg(unsigned int) |
| `%s` | All files |
| `%c` | `syscall.cpp`, `kstd_libcxx.cpp` |
| `%p` | `kernel_log.hpp` FormatArg(const void*) |
| `%02X` | `kernel_log.hpp` DebugBlob |
| `%08X` | (not found, but future-safe) |

All covered by the engine above.

### Why not use `etl::print` / `etl::format_to_n` directly

- `sk_printf` is `extern "C"` and called from C files; ETL headers are C++
- All 15+ callers use `%`-style format strings; migrating all would be a separate task
- The mini engine is simpler and more auditable than adding a C++ wrapper shim
- ETL `print` can be used in a future task to replace `kernel_log.hpp`'s `FormatArg()` calls with `{}` syntax if desired
