# kernel_log 简化重构实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 `kernel_log.hpp` 中 8 个已识别的复杂性问题，提升可维护性、正确性和易用性。

**Architecture:** 所有改动集中在 `src/include/kernel_log.hpp` 一个文件，加上用 ast-grep/sed 批量处理 51+ 个调用文件去掉冗余的尾部 `\n`。改动保持向后兼容的 API 形状（仅行为变化：自动换行、格式说明符生效）。

**Tech Stack:** C++23, freestanding, `__always_inline`, `consteval`, ast-grep

---

## 背景：8 个问题清单

| # | 问题 | 文件 |
|---|------|------|
| P1 | `FormatArg` 与 `LogLine::operator<<` 重复同一套类型打印逻辑 | `kernel_log.hpp` |
| P2 | Header prefix（颜色+`[core_id]`）在 `LogLine` 构造函数和 `LogEmit` 各写一遍 | `kernel_log.hpp` |
| P3 | `Debug`/`Info`/`Warn`/`Err` 结构体构造函数中有冗余的 `if constexpr` 级别检查 | `kernel_log.hpp` |
| P4 | 格式说明符 `{:x}` / `{:#x}` 被静默丢弃，地址以十进制打印 | `kernel_log.hpp` |
| P5 | `KernelFmtStr` 占位符不匹配时错误信息极隐晦（`__builtin_unreachable`） | `kernel_log.hpp` |
| P6 | `DebugBlob` 混用 `#ifdef SIMPLEKERNEL_DEBUG` 和 `if constexpr`，逻辑重叠 | `kernel_log.hpp` |
| P7 | `LogStarter` 命名不直观（实为惰性流代理）| `kernel_log.hpp` |
| P8 | 无自动换行，51 个调用文件每次都要手写 `"\n"` | 51 个 `.cpp` |

---

## Task 1：实现格式说明符支持（P4 — 最高优先）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`EmitUntilNextArg`、所有 `FormatArg` 重载、`LogFormat`）

**背景：**
`{:#x}` 在 syscall.cpp、virtio_driver.cpp 等处广泛使用，期望输出十六进制，但当前实现全部以十进制输出。根本原因是 `EmitUntilNextArg` 把说明符字符串吃掉后直接丢弃。

**Step 1：在 `fmt_detail` 命名空间中添加 `FmtSpec` 结构体**

在 `fmt_detail` namespace 的末尾（`CountFormatArgs` 函数后）添加：

```cpp
/// 格式占位符解析结果
struct FmtSpec {
  const char* next;        ///< 跳过占位符后的字符串指针
  const char* spec_begin;  ///< 说明符起始（'{' 之后），nullptr 表示无说明符
  const char* spec_end;    ///< 说明符末尾（'}' 之前）
};
```

**Step 2：将 `EmitUntilNextArg` 改为返回 `FmtSpec`**

把现有的 `EmitUntilNextArg` 函数替换为：

```cpp
/// 输出格式字符串中直到下一个占位符之前的字面量，
/// 返回 FmtSpec（占位符后的指针 + 说明符范围）
__always_inline auto EmitUntilNextArg(const char* s) -> fmt_detail::FmtSpec {
  while (*s != '\0') {
    if (s[0] == '{' && s[1] == '{') {
      etl_putchar('{');
      s += 2;
      continue;
    }
    if (s[0] == '}' && s[1] == '}') {
      etl_putchar('}');
      s += 2;
      continue;
    }
    if (s[0] == '{') {
      ++s;  // skip '{'
      const char* spec_begin = s;
      while (*s != '\0' && *s != '}') ++s;
      const char* spec_end = s;
      if (*s == '}') ++s;  // skip '}'
      return {s, spec_begin, spec_end};
    }
    etl_putchar(*s++);
  }
  return {s, nullptr, nullptr};
}
```

**Step 3：为所有 `FormatArg` 重载添加 spec 参数，实现整数十六进制**

删除现有所有 `FormatArg` 重载，替换为以下实现：

```cpp
// ── 内部辅助：解析说明符中是否含十六进制标志 ──────────────────────────
namespace fmt_detail {
__always_inline void ParseHexSpec(const char* sb, const char* se,
                                  bool& is_hex, bool& upper, bool& alt) {
  is_hex = false;
  upper = false;
  alt = false;
  if (sb == nullptr) return;
  for (const char* p = sb; p < se; ++p) {
    if (*p == 'x') {
      is_hex = true;
    } else if (*p == 'X') {
      is_hex = true;
      upper = true;
    } else if (*p == '#') {
      alt = true;
    }
  }
}
}  // namespace fmt_detail

/// 输出有符号整数（支持 {:x}/{:#x}/{:X}/{:#X} 说明符）
__always_inline void FormatArg(long long val, const char* sb,
                               const char* se) {
  bool is_hex, upper, alt;
  fmt_detail::ParseHexSpec(sb, se, is_hex, upper, alt);
  if (is_hex) {
    if (alt) {
      etl_putchar('0');
      etl_putchar(upper ? 'X' : 'x');
    }
    sk_emit_hex(static_cast<unsigned long long>(val), 0, upper ? 1 : 0);
  } else {
    sk_emit_sint(val);
  }
}
__always_inline void FormatArg(long val, const char* sb, const char* se) {
  FormatArg(static_cast<long long>(val), sb, se);
}
__always_inline void FormatArg(int val, const char* sb, const char* se) {
  FormatArg(static_cast<long long>(val), sb, se);
}

/// 输出无符号整数（支持 {:x}/{:#x}/{:X}/{:#X} 说明符）
__always_inline void FormatArg(unsigned long long val, const char* sb,
                               const char* se) {
  bool is_hex, upper, alt;
  fmt_detail::ParseHexSpec(sb, se, is_hex, upper, alt);
  if (is_hex) {
    if (alt) {
      etl_putchar('0');
      etl_putchar(upper ? 'X' : 'x');
    }
    sk_emit_hex(val, 0, upper ? 1 : 0);
  } else {
    sk_emit_uint(val);
  }
}
__always_inline void FormatArg(unsigned long val, const char* sb,
                               const char* se) {
  FormatArg(static_cast<unsigned long long>(val), sb, se);
}
__always_inline void FormatArg(unsigned int val, const char* sb,
                               const char* se) {
  FormatArg(static_cast<unsigned long long>(val), sb, se);
}

/// 输出布尔值（说明符忽略）
__always_inline void FormatArg(bool val, const char* /*sb*/,
                               const char* /*se*/) {
  sk_emit_str(val ? "true" : "false");
}

/// 输出字符（说明符忽略）
__always_inline void FormatArg(char val, const char* /*sb*/,
                               const char* /*se*/) {
  etl_putchar(val);
}

/// 输出 C 字符串（说明符忽略）
__always_inline void FormatArg(const char* val, const char* /*sb*/,
                               const char* /*se*/) {
  sk_emit_str(val);
}

/// 输出指针地址（始终以十六进制+0x前缀输出，说明符忽略）
__always_inline void FormatArg(const void* val, const char* /*sb*/,
                               const char* /*se*/) {
  etl_putchar('0');
  etl_putchar('x');
  sk_emit_hex(static_cast<unsigned long long>(reinterpret_cast<uintptr_t>(val)),
              static_cast<int>(sizeof(void*) * 2), /*upper=*/0);
}
__always_inline void FormatArg(void* val, const char* sb, const char* se) {
  FormatArg(static_cast<const void*>(val), sb, se);
}
```

**Step 4：更新 `LogFormat` 传递 spec**

删除现有 `LogFormat` 函数，替换为：

```cpp
/// Base case: 输出剩余字面量
__always_inline void LogFormat(const char* s) {
  while (*s != '\0') {
    if (s[0] == '{' && s[1] == '{') {
      etl_putchar('{');
      s += 2;
    } else if (s[0] == '}' && s[1] == '}') {
      etl_putchar('}');
      s += 2;
    } else {
      etl_putchar(*s++);
    }
  }
}

/// Recursive case: 输出到下一个占位符，格式化当前参数，递归处理剩余
template <typename T, typename... Rest>
__always_inline void LogFormat(const char* s, T&& val, Rest&&... rest) {
  auto [next, sb, se] = EmitUntilNextArg(s);
  FormatArg(static_cast<std::decay_t<T>>(val), sb, se);
  LogFormat(next, static_cast<Rest&&>(rest)...);
}
```

**Step 5：验证现有调用不变**
不需要运行测试，只需确认 `kernel_log.hpp` 编译通过（在后续 Task 集成完成后统一验证）。

---

## Task 2：消除 `LogLine::operator<<` 与 `FormatArg` 的重复（P1）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`LogLine` 类的 `operator<<` 区段）

**背景：**
Task 1 完成后，`FormatArg` 已是唯一的类型打印出口。`operator<<` 里的重复代码改为直接调用 `FormatArg`。

**Step 1：将 `LogLine` 中所有 `operator<<` 重载替换为委托调用**

删除现有所有 `operator<<` 重载，替换为：

```cpp
// ── Value overloads ──────────────────────────────────────────────────

/// 输出有符号整数类型
template <std::signed_integral T>
auto operator<<(T val) -> LogLine& {
  FormatArg(static_cast<long long>(val), nullptr, nullptr);
  return *this;
}

/// 输出无符号整数类型
template <std::unsigned_integral T>
  requires(!std::same_as<T, bool> && !std::same_as<T, char>)
auto operator<<(T val) -> LogLine& {
  FormatArg(static_cast<unsigned long long>(val), nullptr, nullptr);
  return *this;
}

/// 输出 C 字符串
auto operator<<(const char* val) -> LogLine& {
  FormatArg(val, nullptr, nullptr);
  return *this;
}

/// 输出单个字符
auto operator<<(char val) -> LogLine& {
  FormatArg(val, nullptr, nullptr);
  return *this;
}

/// 输出布尔值
auto operator<<(bool val) -> LogLine& {
  FormatArg(val, nullptr, nullptr);
  return *this;
}

/// 输出指针地址
auto operator<<(const void* val) -> LogLine& {
  FormatArg(val, nullptr, nullptr);
  return *this;
}
```

---

## Task 3：提取 `EmitHeader` 辅助函数，消除前缀重复（P2）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`LogLine` 构造函数、`LogEmit`）

**Step 1：在 `detail` 命名空间中添加 `EmitHeader`**

在 `kLogColors` 数组定义之后（`kLogColors` 定义约在第 61 行附近），添加：

```cpp
/// 输出日志行前缀：ANSI 颜色码 + [core_id]
template <LogLevel::enum_type Level>
__always_inline void EmitHeader() {
  sk_emit_str(kLogColors[Level]);
  etl_putchar('[');
  sk_emit_sint(static_cast<long long>(cpu_io::GetCurrentCoreId()));
  etl_putchar(']');
}
```

**Step 2：更新 `LogLine` 构造函数使用 `EmitHeader`**

把 `LogLine::LogLine()` 构造函数体中的前缀发射代码替换为：

```cpp
LogLine() {
  log_lock.Lock().or_else([](auto&& err) {
    sk_emit_str("LogLine: Failed to acquire lock: ");
    sk_emit_str(err.message());
    etl_putchar('\n');
    while (true) {
      cpu_io::Pause();
    }
    return Expected<void>{};
  });
  EmitHeader<Level>();
}
```

**Step 3：更新 `LogEmit` 使用 `EmitHeader`**

把 `LogEmit` 函数体中重复的前缀发射代码（`sk_emit_str(kLogColors[Level])` 那几行）替换为 `EmitHeader<Level>();`：

```cpp
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    [[maybe_unused]] const std::source_location& location,
    KernelFormatString<Args...> fmt, Args&&... args) {
  if constexpr (Level < kMinLogLevel) {
    return;
  }
  LockGuard<SpinLock> lock_guard(log_lock);
  EmitHeader<Level>();
  if constexpr (Level == LogLevel::kDebug) {
    etl_putchar('[');
    sk_emit_str(location.file_name());
    etl_putchar(':');
    sk_emit_uint(static_cast<unsigned long long>(location.line()));
    etl_putchar(' ');
    sk_emit_str(location.function_name());
    etl_putchar(']');
    etl_putchar(' ');
  }
  LogFormat(fmt.str, static_cast<Args&&>(args)...);
  sk_emit_str(kReset);
}
```

---

## Task 4：删除结构体构造函数中冗余的 `if constexpr` 级别检查（P3）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`Debug`、`Info`、`Warn`、`Err` 结构体）

**背景：**
`LogEmit` 内部已有 `if constexpr (Level < kMinLogLevel) { return; }`，结构体里的外层检查是无效冗余。

**Step 1：简化四个结构体的构造函数**

将四个结构体的构造函数都改为直接调用 `LogEmit`，去掉外层 `if constexpr`：

```cpp
// Debug
template <typename... Args>
struct Debug {
  explicit Debug(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kDebug>(location, fmt,
                                              static_cast<Args&&>(args)...);
  }
};

// Info
template <typename... Args>
struct Info {
  explicit Info(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kInfo>(location, fmt,
                                             static_cast<Args&&>(args)...);
  }
};

// Warn
template <typename... Args>
struct Warn {
  explicit Warn(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kWarn>(location, fmt,
                                             static_cast<Args&&>(args)...);
  }
};

// Err
template <typename... Args>
struct Err {
  explicit Err(
      detail::KernelFormatString<Args...> fmt, Args&&... args,
      const std::source_location& location = std::source_location::current()) {
    detail::LogEmit<detail::LogLevel::kErr>(location, fmt,
                                            static_cast<Args&&>(args)...);
  }
};
```

---

## Task 5：改善 `consteval` 格式字符串错误信息（P5）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`KernelFmtStr::KernelFmtStr` 构造函数）

**Step 1：将 `__builtin_unreachable()` 替换为 `throw`**

```cpp
template <typename _Str>
// NOLINTNEXTLINE(google-explicit-constructor)
consteval KernelFmtStr(const _Str& s) : str(s) {  // NOLINT
  if (fmt_detail::CountFormatArgs(s) != sizeof...(Args)) {
    throw "klog: format string placeholder count does not match argument count";
  }
}
```

在 C++23 的 `consteval` 函数中，`throw` 会触发编译期错误，并将字符串作为诊断信息输出，远比 `__builtin_unreachable()` 清晰。

---

## Task 6：修复 `DebugBlob` 中 `#ifdef` 与 `if constexpr` 混用（P6）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`DebugBlob` 函数）

**背景：**
`kMinLogLevel` 已经能在编译期消除 Debug 输出，外层 `#ifdef SIMPLEKERNEL_DEBUG` 与之逻辑重叠且不等价。统一用 `if constexpr` 处理。

**Step 1：重写 `DebugBlob`**

```cpp
__always_inline void DebugBlob([[maybe_unused]] const void* data,
                               [[maybe_unused]] size_t size) {
  if constexpr (detail::LogLevel::kDebug >= detail::kMinLogLevel) {
    LockGuard<SpinLock> lock_guard(klog::detail::log_lock);
    detail::EmitHeader<detail::LogLevel::kDebug>();
    etl_putchar(' ');
    for (size_t i = 0; i < size; i++) {
      etl_putchar('0');
      etl_putchar('x');
      sk_emit_hex(
          static_cast<unsigned long long>(
              reinterpret_cast<const uint8_t*>(data)[i]),
          /*width=*/2, /*upper=*/1);
      etl_putchar(' ');
    }
    sk_emit_str(detail::kReset);
    etl_putchar('\n');
  }
}
```

注意：`DebugBlob` 内部直接使用 `EmitHeader`（Task 3 中已提取），去掉了 `#ifdef SIMPLEKERNEL_DEBUG`，统一通过 `kMinLogLevel` 控制。

---

## Task 7：将 `LogStarter` 重命名为 `LogStream`（P7）

**Files:**
- Modify: `src/include/kernel_log.hpp`

**Step 1：全局替换类名**

将 `kernel_log.hpp` 中所有 `LogStarter` 替换为 `LogStream`（class 定义、注释、全局实例声明均涉及）：

- `template <LogLevel::enum_type Level> struct LogStarter` → `struct LogStream`
- `[[maybe_unused]] inline detail::LogStarter<...> info/warn/debug/err` → `detail::LogStream<...>`

同时更新 Doxygen 注释，将 "流式日志入口" 的描述更新为 "惰性流式日志代理（LogStream）"。

---

## Task 8：添加自动换行，批量去除调用处的尾部 `\n`（P8）

**Files:**
- Modify: `src/include/kernel_log.hpp`（`LogEmit`、`LogLine::~LogLine`）
- Modify: 所有使用 `klog::` 的 51 个 `.cpp` 文件

**Step 1：在 `LogEmit` 末尾添加自动换行**

在 `LogEmit` 函数体末尾（`sk_emit_str(kReset)` 之前）加一行：

```cpp
  LogFormat(fmt.str, static_cast<Args&&>(args)...);
  etl_putchar('\n');    // ← 新增自动换行
  sk_emit_str(kReset);
```

**Step 2：在 `LogLine::~LogLine` 添加自动换行**

在析构函数的 `sk_emit_str(kReset)` 之前加一行：

```cpp
~LogLine() {
  if (!released_) {
    etl_putchar('\n');    // ← 新增自动换行
    sk_emit_str(kReset);
    log_lock.UnLock().or_else([](auto&& err) {
      ...
    });
  }
}
```

**Step 3：用 sed 批量去除所有调用处的尾部 `\n"`**

在项目根目录执行：

```bash
# printf 风格：去除格式字符串末尾的 \n（仅最后一个）
# 匹配：任意 klog 调用中以 \n" 结尾的格式字符串
find src/ tests/ -name '*.cpp' | xargs sed -i 's/\\n"\(,\s*$\|)\s*;\|,\s*\\$/"\1/g'
```

由于 sed 的局限性，更安全的方式是使用 Python 脚本精确处理：

```bash
python3 - <<'EOF'
import re, os, glob

pattern = re.compile(r'(klog::[A-Za-z]+\s*\()(.*?)\\n(")', re.DOTALL)

def fix_file(path):
    with open(path) as f:
        content = f.read()
    # 找出所有 klog 调用，去掉第一个字符串参数末尾的 \n
    new_content = re.sub(r'(klog::[A-Za-z]+\s*(?:<[^>]*>)?\s*\()("(?:[^"\\]|\\.)*?)\\n"',
                         r'\1\2"', content)
    # 流风格: << "...\n"
    new_content = re.sub(r'(klog::\w+\s*<<\s*"(?:[^"\\]|\\.)*?)\\n"', r'\1"', new_content)
    if new_content != content:
        with open(path, 'w') as f:
            f.write(new_content)
        print(f'  fixed: {path}')

for p in glob.glob('src/**/*.cpp', recursive=True) + glob.glob('tests/**/*.cpp', recursive=True):
    fix_file(p)
EOF
```

**Step 4：目视抽查 5 个文件确认无双换行**

```bash
grep -n 'klog::' src/syscall.cpp | head -5
grep -n 'klog::' src/device/device.cpp | head -5
grep -n 'klog::' src/main.cpp | head -3
```

期望输出：所有格式字符串均不以 `\n"` 结尾。

---

## 最终验证

**Step 1：检查 `kernel_log.hpp` 结构完整性**

```bash
grep -n 'EmitHeader\|FmtSpec\|FormatArg\|LogStream\|LogFormat\|LogEmit\|DebugBlob' \
  src/include/kernel_log.hpp
```

确认：
- `FmtSpec` 出现在 `fmt_detail` namespace 中
- `EmitHeader` 出现一次（定义）并在 `LogLine` 和 `LogEmit` 中被调用
- `FormatArg` 重载每个都带 `(type, const char*, const char*)` 签名
- `LogStream` 替换了所有 `LogStarter`
- `DebugBlob` 不含 `#ifdef`

**Step 2：检查调用侧无尾部 `\n"`**

```bash
grep -rn '\\n"' src/ tests/ --include='*.cpp' | grep 'klog::'
```

期望：0 匹配。

**Step 3：提交**

```bash
git add src/include/kernel_log.hpp src/ tests/
git commit -s -m "refactor(klog): simplify kernel_log — 8 complexity fixes

- Support {:x}/{:#x}/{:X}/{:#X} format specifiers for integers
- Unify type printing: operator<< delegates to FormatArg
- Extract EmitHeader<Level>() to eliminate duplicated prefix code
- Remove redundant if constexpr level checks in struct constructors
- Replace __builtin_unreachable with throw in consteval for clearer errors
- Fix DebugBlob: use if constexpr only, remove #ifdef SIMPLEKERNEL_DEBUG
- Rename LogStarter -> LogStream for clarity
- Add auto-newline in LogEmit/~LogLine, strip trailing \n from call sites"
```
