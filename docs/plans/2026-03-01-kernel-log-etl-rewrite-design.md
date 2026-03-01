# Design: kernel_log.hpp ETL Rewrite

**Date**: 2026-03-01
**Status**: Approved

---

## 背景

当前 `src/include/kernel_log.hpp` 存在以下问题：

1. **类型安全缺失**：printf 风格 API（`klog::Debug("0x%lx", val)`）通过模板转发调用
   `sk_printf(std::forward<Args>(args)...)`，`__attribute__((format))` 对模板转发调用
   无效，格式字符串与参数类型不匹配只能在运行时触发错误。
2. **流式 API 无格式控制**：`klog::info << val` 的每个 `operator<<` 直接调用
   `sk_printf("%d"/"0x%lx"/...)` 硬编码格式，无法控制进制、宽度、填充等。
3. **输出 sink 不可插拔**：头文件 `@todo` 中已提出，但尚未实现。

本次重写目标：
- 用 `etl::print` + `etl::format_string<Args...>` 替换 printf 风格路径，获得**编译期格式
  字符串验证**。
- 用 `etl::to_string` + `etl::format_spec` 增强流式 API，支持 `etl::hex`、`etl::setw` 等
  操纵符。
- 实现 `etl_putchar` shim，统一两条路径的物理输出。

---

## 约束

- **freestanding 环境**：`etl/format.h` 包含 `<cmath>`（用于浮点格式化）。内核日志不会
  格式化 `float`/`double`，相关模板特化永远不会实例化，`<cmath>` 符号不会被链接。
  风险可接受。
- **Breaking change 可接受**：285 个调用点从 `%s/%d` 迁移到 `{}` 格式语法。
- **保留两套 API**：printf 风格（`klog::Debug("{}", val)`）+ 流式（`klog::info << val`）。
- **不实现 sink 抽象**：`etl::delegate` sink 计划留给后续 PR，本次只做 `etl_putchar` shim
  为其铺路。

---

## 架构

```
klog::Debug("{:#x}", val)              klog::info << etl::hex << val
        │                                          │
        ▼                                          ▼
  LogEmit<kDebug>(loc, fmt, args...)       LogLine::operator<<
  etl::print(fmt, args...)               etl::to_string(val, buf, spec_)
        │                                          │
        ▼                                          ▼
  etl_putchar(c)                         sk_putchar(c, nullptr)
        │
        ▼
  sk_putchar(c, nullptr)
        │
        ▼
     串口 / 早期控制台
```

---

## 变更 1：etl_putchar shim

`etl/print.h` 要求项目提供 `extern "C" void etl_putchar(int c)` 实现，作为其字符输出
原语。

| 文件 | 操作 | 内容 |
|------|------|------|
| `src/arch/riscv64/early_console.cpp` | 修改 | 末尾添加 `etl_putchar` 实现 |
| `src/arch/aarch64/early_console.cpp` | 修改 | 末尾添加 `etl_putchar` 实现 |
| `src/arch/x86_64/early_console.cpp`  | 修改 | 末尾添加 `etl_putchar` 实现 |

实现内容（三处相同）：

```cpp
extern "C" void etl_putchar(int c) {
    sk_putchar(c, nullptr);
}
```

---

## 变更 2：kernel_log.hpp 主体重写

### 2a. 新增头文件依赖

```cpp
// 新增
#include <etl/format.h>     // etl::format_string, etl::print
#include <etl/print.h>      // etl::print, etl::println
#include <etl/to_string.h>  // etl::to_string
#include <etl/format_spec.h> // etl::format_spec, etl::hex, etl::setw, ...
#include <etl/string.h>     // etl::string<N>

// 移除
// #include "kstd_cstdio"  (LogLine 内部不再使用 sk_printf)
// kstd_cstdio 仍由 LogEmit 使用的 sk_printf 保留，视具体实现决定是否移除
```

### 2b. LogEmit：printf 风格路径改造

**当前签名**：
```cpp
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    const std::source_location& location, Args&&... args);
```

**新签名**：
```cpp
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    [[maybe_unused]] const std::source_location& location,
    etl::format_string<Args...> fmt,   // ← 编译期格式字符串校验
    Args&&... args);
```

**新实现逻辑**：

```cpp
template <LogLevel::enum_type Level, typename... Args>
__always_inline void LogEmit(
    [[maybe_unused]] const std::source_location& location,
    etl::format_string<Args...> fmt,
    Args&&... args) {
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
```

**零参数情况**：原实现有 `sizeof...(Args) == 0` 的特判（no-op）。新实现中，零参数时
调用 `klog::Debug()` 不再有意义（没有 format string），可以在公开 API 层禁止此用法，或
保留一个无参重载打印空行：

```cpp
// 可选：保留无参重载
template <LogLevel::enum_type Level>
__always_inline void LogEmitEmpty(const std::source_location& location) {
  if constexpr (Level < kMinLogLevel) { return; }
  LockGuard<SpinLock> lock_guard(log_lock);
  etl::print("{}[{}]\n{}", kLogColors[Level],
             cpu_io::GetCurrentCoreId(), kReset);
}
```

### 2c. 公开 printf 风格 struct（Debug / Info / Warn / Err）

**当前 CTAD deduction guide**：
```cpp
template <typename... Args>
Debug(Args&&...) -> Debug<Args...>;
```

**新 CTAD deduction guide**（匹配 `etl::format_string` 首参）：
```cpp
template <typename... Args>
Debug(etl::format_string<Args...>, Args&&...) -> Debug<Args...>;
```

**新构造函数签名**：
```cpp
explicit Debug(
    etl::format_string<Args...> fmt,
    Args&&... args,
    const std::source_location& location = std::source_location::current());
```

Info / Warn / Err 同理。

### 2d. LogLine：流式路径增强

`LogLine<Level>` 新增 `etl::format_spec spec_` 成员，以及针对 ETL 操纵符类型的
`operator<<` 重载。

**新增成员**：
```cpp
private:
  etl::format_spec spec_{};   // 当前格式 spec，每次输出后重置
  bool released_ = false;
```

**新增 operator<< 重载（操纵符）**：
```cpp
// 进制：etl::hex / etl::dec / etl::oct / etl::bin
auto operator<<(etl::private_basic_format_spec::base_spec s) -> LogLine& {
    spec_.base(s.base); return *this;
}
// 宽度：etl::setw(N)
auto operator<<(etl::private_basic_format_spec::width_spec s) -> LogLine& {
    spec_.width(s.width); return *this;
}
// 填充：etl::setfill('0')
auto operator<<(etl::private_basic_format_spec::fill_spec<char> s) -> LogLine& {
    spec_.fill(s.fill); return *this;
}
// 大小写：etl::uppercase / etl::nouppercase
auto operator<<(etl::private_basic_format_spec::uppercase_spec s) -> LogLine& {
    spec_.upper_case(s.upper_case); return *this;
}
// 布尔：etl::boolalpha / etl::noboolalpha
auto operator<<(etl::private_basic_format_spec::boolalpha_spec s) -> LogLine& {
    spec_.boolalpha(s.boolalpha); return *this;
}
// 显示进制前缀：etl::showbase / etl::noshowbase
auto operator<<(etl::private_basic_format_spec::showbase_spec s) -> LogLine& {
    spec_.show_base(s.show_base); return *this;
}
// 直接传入完整 format_spec（高级用法）
auto operator<<(const etl::format_spec& s) -> LogLine& {
    spec_ = s; return *this;
}
```

**整数 operator<< 改为 etl::to_string**：
```cpp
template <std::integral T>
  requires(!std::same_as<T, bool> && !std::same_as<T, char>)
auto operator<<(T val) -> LogLine& {
    etl::string<32> buf;
    etl::to_string(val, buf, spec_);
    for (char c : buf) sk_putchar(c, nullptr);
    spec_ = {};   // 每次值输出后重置（避免 sticky 格式影响后续字段）
    return *this;
}
```

**字符串 operator<< 保持简单输出**（`etl::to_string` 对字符串做宽度/填充）：
```cpp
auto operator<<(const char* val) -> LogLine& {
    etl::string<256> buf;
    etl::to_string(etl::string_view(val), buf, spec_);
    for (char c : buf) sk_putchar(c, nullptr);
    spec_ = {};
    return *this;
}
```

**bool / char 保持原行为**（或接入 `etl::to_string` + `boolalpha_spec`）：
```cpp
auto operator<<(bool val) -> LogLine& {
    etl::string<8> buf;
    etl::to_string(val, buf, spec_);   // spec_.boolalpha() 生效
    for (char c : buf) sk_putchar(c, nullptr);
    spec_ = {};
    return *this;
}
```

### 2e. 移除旧行为

- 移除 `LogEmit` 中 `sizeof...(Args) == 1` 的 `%s` 包装特判（type safety 解决了此问题）
- 移除 `LogEmit` 中多参数的 `sk_printf(std::forward<Args>(args)...)` 调用

---

## 变更 3：调用点迁移（285 处）

格式字符串从 printf 语法迁移到 `std::format`/ETL 语法：

| printf | etl::print |
|--------|------------|
| `%d` / `%i` | `{}` |
| `%u` | `{}` |
| `%ld` / `%lu` | `{}` |
| `%lld` / `%llu` | `{}` |
| `%x` / `%X` | `{:x}` / `{:X}` |
| `%#x` | `{:#x}` |
| `%08x` | `{:#010x}` 或 `{:08x}` |
| `%p` | `{:#x}`（指针转 `uintptr_t`）|
| `%s` | `{}` |
| `%c` | `{}` |
| `%zu` | `{}` |
| `\n` 结尾 | 移除（LogEmit 不自动换行；需要换行时在 fmt 末尾保留） |

**注意**：`klog::Debug` 在 Debug 构建中会输出 `source_location`，格式字符串不需要手动
加文件/行号信息——这部分由 `LogEmit` 自动处理。

---

## 调用侧示例对比

### printf 风格

```cpp
// 前
klog::Info("VirtIO block device initialized at 0x%lx, %lu sectors\n",
           base_addr, num_sectors);

// 后
klog::Info("VirtIO block device initialized at {:#x}, {} sectors\n",
           base_addr, num_sectors);
```

### 流式风格

```cpp
// 前
klog::info << "reg=0x" << val << "\n";   // 无法控制十六进制格式

// 后
klog::info << "reg=" << etl::showbase << etl::hex << val << "\n";
```

---

## 不在本次范围内

- `etl::delegate` 可插拔 sink（留给后续 PR）
- 指针 `%p` 迁移时需要显式转换 `reinterpret_cast<uintptr_t>(ptr)`，调用点逐一处理
- `DebugBlob` 函数内部的 `sk_printf` 调用（可保留 or 迁移，独立决定）
