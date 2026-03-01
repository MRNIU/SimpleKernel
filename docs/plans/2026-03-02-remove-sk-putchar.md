# Remove sk_putchar, Use etl_putchar Directly

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Delete `sk_putchar` (global function pointer with unused `ctx` param) and replace every call site with `etl_putchar(int c)` — the ETL output hook that each arch's `early_console.cpp` already defines.

**Architecture:** `etl_putchar` becomes the sole low-level output primitive. Each arch defines it directly (calling SBI / PL011 / COM1 with a null-guard where needed). The `sk_emit_*` helpers in `sk_stdio.h` declare `etl_putchar` and call it. Everything above (`kernel_log`, `spinlock`, `libcxx`, `syscall`) calls `etl_putchar` transitively through `kstd_cstdio → sk_stdio.h`.

**Tech Stack:** C23, C++23 freestanding, `etl_putchar`, `sk_stdio.h`.

---

### Task 1: Update `sk_stdio.h` — swap the primitive

**Files:**
- Modify: `src/libc/include/sk_stdio.h`

Replace the `sk_putchar` function-pointer declaration with a plain `etl_putchar` declaration, and update all internal `sk_emit_*` call sites.

**Step 1: Apply the diff**

Replace the declaration block (line 15) and every internal `sk_putchar(…, NULL)` call:

```c
/* REMOVE */
extern void (*sk_putchar)(int c, void* ctx);

/* ADD — inside the existing extern "C" { } block */
void etl_putchar(int c);
```

Then do a mechanical s/`sk_putchar(/etl_putchar(/` + remove the `, NULL)` / `, nullptr)` second argument throughout the file. The affected lines are:

| Old | New |
|-----|-----|
| `sk_putchar((unsigned char)*s, NULL);` | `etl_putchar((unsigned char)*s);` |
| `sk_putchar('-', NULL);` | `etl_putchar('-');` |
| `sk_putchar('0', NULL);` | `etl_putchar('0');` |
| `sk_putchar(buf[i], NULL);` (×4) | `etl_putchar(buf[i]);` |
| `sk_putchar('0', NULL);` in hex | `etl_putchar('0');` |

Final `sk_stdio.h` content (replace file entirely):

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

/** Output primitive — implemented per-arch in early_console.cpp */
void etl_putchar(int c);

/* ── Freestanding emit helpers (always-inline, no format strings) ────────── */

/** Emit a NUL-terminated C string character by character. NULL-safe. */
static __attribute__((always_inline)) inline void sk_emit_str(const char* s) {
  if (!s) {
    s = "(null)";
  }
  while (*s) {
    etl_putchar((unsigned char)*s);
    ++s;
  }
}

/** Emit a signed 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_sint(long long v) {
  char buf[20];
  int n = 0;
  if (v < 0) {
    etl_putchar('-');
    /* avoid UB on LLONG_MIN: cast before negation */
    unsigned long long uv = (unsigned long long)(-(v + 1)) + 1ULL;
    if (uv == 0) {
      etl_putchar('0');
      return;
    }
    while (uv) {
      buf[n++] = (char)('0' + (int)(uv % 10));
      uv /= 10;
    }
  } else {
    unsigned long long uv = (unsigned long long)v;
    if (uv == 0) {
      etl_putchar('0');
      return;
    }
    while (uv) {
      buf[n++] = (char)('0' + (int)(uv % 10));
      uv /= 10;
    }
  }
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
}

/** Emit an unsigned 64-bit integer in decimal. */
static __attribute__((always_inline)) inline void sk_emit_uint(
    unsigned long long v) {
  char buf[20];
  int n = 0;
  if (v == 0) {
    etl_putchar('0');
    return;
  }
  while (v) {
    buf[n++] = (char)('0' + (int)(v % 10));
    v /= 10;
  }
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
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
    while (v) {
      buf[n++] = digits[v & 0xf];
      v >>= 4;
    }
  }
  for (int i = n; i < width; i++) etl_putchar('0');
  for (int i = n - 1; i >= 0; i--) etl_putchar(buf[i]);
}

#ifdef __cplusplus
}
#endif

#endif  // SIMPLEKERNEL_SRC_LIBC_INCLUDE_SK_STDIO_H_
```

**Step 2: Verify no `sk_putchar` remains in the file**

```bash
grep sk_putchar src/libc/include/sk_stdio.h
# Expected: no output
```

---

### Task 2: Delete `sk_stdio.c` and update CMakeLists

**Files:**
- Delete: `src/libc/sk_stdio.c`
- Modify: `src/libc/CMakeLists.txt` line 11

The file only contained `dummy_putchar` + the `sk_putchar` function-pointer definition. After Task 1 those are gone.

**Step 1: Remove the source file**

```bash
rm src/libc/sk_stdio.c
```

**Step 2: Remove from CMakeLists**

`src/libc/CMakeLists.txt` before:
```cmake
TARGET_SOURCES (
    libc
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_stdlib.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_string.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_stdio.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_ctype.c)
```

After (remove the `sk_stdio.c` line):
```cmake
TARGET_SOURCES (
    libc
    INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/sk_stdlib.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_string.c
              ${CMAKE_CURRENT_SOURCE_DIR}/sk_ctype.c)
```

**Step 3: Verify**

```bash
grep sk_stdio src/libc/CMakeLists.txt
# Expected: no output
```

---

### Task 3: Rewrite `riscv64/early_console.cpp`

**Files:**
- Modify: `src/arch/riscv64/early_console.cpp`

SBI is always available — no init needed, `EarlyConsole` struct and `kstd_cstdio` include can both be removed.

**Step 1: Replace file content**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <opensbi_interface.h>

extern "C" void etl_putchar(int c) {
  sbi_debug_console_write_byte(c);
}
```

**Step 2: Verify no `sk_putchar` remains**

```bash
grep sk_putchar src/arch/riscv64/early_console.cpp
# Expected: no output
```

---

### Task 4: Rewrite `aarch64/early_console.cpp`

**Files:**
- Modify: `src/arch/aarch64/early_console.cpp`

`EarlyConsole` still needed to initialize PL011 hardware. `console_putchar` helper is collapsed into `etl_putchar`. Null-guard preserved.

**Step 1: Replace file content**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "pl011/pl011_driver.hpp"
#include "pl011_singleton.h"

namespace {

pl011::Pl011Device* pl011_uart = nullptr;

struct EarlyConsole {
  EarlyConsole() {
    Pl011Singleton::create(SIMPLEKERNEL_EARLY_CONSOLE_BASE);
    pl011_uart = &Pl011Singleton::instance();
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) {
  if (pl011_uart) {
    pl011_uart->PutChar(c);
  }
}
```

**Step 2: Verify**

```bash
grep sk_putchar src/arch/aarch64/early_console.cpp
# Expected: no output
```

---

### Task 5: Rewrite `x86_64/early_console.cpp`

**Files:**
- Modify: `src/arch/x86_64/early_console.cpp`

Same pattern as aarch64 — keep EarlyConsole for hardware init, collapse `console_putchar` into `etl_putchar`.

**Step 1: Replace file content**

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>
#include <etl/singleton.h>

using SerialSingleton = etl::singleton<cpu_io::Serial>;

namespace {

cpu_io::Serial* serial = nullptr;

struct EarlyConsole {
  EarlyConsole() {
    SerialSingleton::create(cpu_io::kCom1);
    serial = &SerialSingleton::instance();
  }
};

EarlyConsole early_console;

}  // namespace

extern "C" void etl_putchar(int c) {
  if (serial) {
    serial->Write(c);
  }
}
```

**Step 2: Verify**

```bash
grep sk_putchar src/arch/x86_64/early_console.cpp
# Expected: no output
```

---

### Task 6: Update `kernel_log.hpp`

**Files:**
- Modify: `src/include/kernel_log.hpp`

Replace every `sk_putchar(X, nullptr)` with `etl_putchar(X)`. The `kstd_cstdio` include (already present) will pull in the new `etl_putchar` declaration from `sk_stdio.h` — no include changes needed.

**Affected lines** (all in `src/include/kernel_log.hpp`):

```
157, 164, 165, 180, 185, 199, 209, 212, 215, 248, 255, 257, 274,
308, 320, 321, 373, 375, 377, 379, 381, 383, 384, 422, 424, 425,
427, 428, 431, 434
```

Mechanical substitution: `s/sk_putchar(\(.*\), nullptr)/etl_putchar(\1)/g`

**Step 1: Apply substitution**

```bash
sed -i 's/sk_putchar(\(.*\), nullptr)/etl_putchar(\1)/g' src/include/kernel_log.hpp
```

**Step 2: Verify**

```bash
grep sk_putchar src/include/kernel_log.hpp
# Expected: no output
grep -c etl_putchar src/include/kernel_log.hpp
# Expected: ~30
```

---

### Task 7: Update `spinlock.hpp`, `kstd_libcxx.cpp`, `syscall.cpp`

**Files:**
- Modify: `src/include/spinlock.hpp` (2 occurrences — lines 144, 159)
- Modify: `src/libcxx/kstd_libcxx.cpp` (2 occurrences — lines 204, 210)
- Modify: `src/syscall.cpp` (1 occurrence — line 60)

All three files already get `etl_putchar` through their existing includes:
- `spinlock.hpp` → `#include "kstd_cstdio"` → `sk_stdio.h` → `etl_putchar` declaration ✓
- `kstd_libcxx.cpp` → `#include "kernel_log.hpp"` → `kstd_cstdio` → `sk_stdio.h` ✓
- `syscall.cpp` → `#include "kernel_log.hpp"` → same chain ✓

**Step 1: Apply substitution to all three**

```bash
sed -i 's/sk_putchar(\(.*\), nullptr)/etl_putchar(\1)/g' \
    src/include/spinlock.hpp \
    src/libcxx/kstd_libcxx.cpp \
    src/syscall.cpp
```

**Step 2: Verify**

```bash
grep sk_putchar src/include/spinlock.hpp src/libcxx/kstd_libcxx.cpp src/syscall.cpp
# Expected: no output
```

---

### Task 8: Update interrupt handlers

**Files:**
- Modify: `src/arch/riscv64/interrupt_main.cpp` (line 91)
- Modify: `src/arch/aarch64/interrupt_main.cpp` (line 145)

Both already include `kstd_cstdio`.

**Step 1: riscv64**

Line 91 before:
```cpp
sk_putchar(ch, nullptr);
```
After:
```cpp
etl_putchar(ch);
```

**Step 2: aarch64**

Line 145 before:
```cpp
[](uint8_t ch) { sk_putchar(ch, nullptr); });
```
After:
```cpp
[](uint8_t ch) { etl_putchar(ch); });
```

**Step 3: Verify**

```bash
grep sk_putchar src/arch/riscv64/interrupt_main.cpp src/arch/aarch64/interrupt_main.cpp
# Expected: no output
```

---

### Task 9: Final sweep and build

**Step 1: Confirm zero remaining `sk_putchar` in source**

```bash
grep -r sk_putchar src/ --include="*.cpp" --include="*.c" --include="*.h" --include="*.hpp"
# Expected: no output
```

**Step 2: Build**

```bash
cmake --preset build_riscv64 && cd build_riscv64 && make SimpleKernel 2>&1 | tail -20
# Expected: successful link, no errors
```

**Step 3: Commit**

```bash
git add -A
git commit --signoff -m "refactor(libc): replace sk_putchar with etl_putchar

Remove the sk_putchar function pointer (and its unused ctx parameter).
etl_putchar(int c) — already defined per-arch in early_console.cpp —
becomes the sole low-level output primitive.

- Delete sk_stdio.c (contained only dummy_putchar + sk_putchar)
- Remove sk_stdio.c from libc CMakeLists
- sk_stdio.h declares etl_putchar; sk_emit_* helpers call it directly
- riscv64/early_console: drop EarlyConsole struct (SBI always ready)
- aarch64/x86_64 early_console: collapse console_putchar into etl_putchar
- Replace all sk_putchar(c, nullptr) → etl_putchar(c) across 8 files"
```
