# SimpleKernel 代码规范

> 基准文件: `src/include/spinlock.hpp`
> 格式化: `.clang-format` (Google 风格) 自动处理缩进、换行、空格等排版细节
> 语言标准: C++23 / C23 (freestanding)

本文档定义 clang-format **无法自动处理**的语义级规范，用于指导 AI 生成代码。

---

## 1. 文件结构

```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#pragma once

// 头文件包含

// 声明与定义
```

- 版权声明 `/** @copyright ... */` 必须在文件首行。
- 使用 `#pragma once`。
- 头文件包含分三组（空行分隔，组内字母序，clang-format 自动排序）：
  1. 第三方头文件（`<cpu_io.h>`, `<etl/singleton.h>`）
  2. 标准库头文件（`<atomic>`, `<cstddef>`）
  3. 项目头文件（`"expected.hpp"`, `"kstd_cstdio"`）

---

## 2. 命名规范

| 元素 | 风格 | 示例 |
|------|------|------|
| 文件 | `snake_case` | `kernel_log.hpp` |
| 类 / 结构体 | `PascalCase` | `SpinLock`, `DmaRegion` |
| 枚举类型 | `enum class` | `enum class ErrorCode : uint64_t` |
| 枚举值 | `kCamelCase` | `kSuccess`, `kElfInvalidMagic` |
| 方法 / 函数 | `PascalCase` | `Lock()`, `IsValid()` |
| 系统调用 | `sys_` + `snake_case` | `sys_write()`, `sys_clone()` |
| 命名空间 | `snake_case` | `klog::detail`, `kernel::config` |
| 类型别名 | `PascalCase` | `Expected<T>`, `DeviceManagerSingleton` |
| 常量 | `kCamelCase` | `kPageSize`, `kMaxDevices` |
| 宏 | `SCREAMING_SNAKE` | `SIMPLEKERNEL_DEBUG` |
| Concept | `PascalCase` | `Bus` |

### 成员变量

| 类型 | 可见性 | 命名 | 示例 |
|------|--------|------|------|
| `class` | `public` | `snake_case`（**无**后缀） | `name` |
| `class` | `protected` / `private` | `snake_case_`（后缀 `_`） | `locked_`, `data_` |
| `struct` | 全部 | `snake_case`（**无**后缀） | `virt`, `phys`, `core_id` |

构造函数参数与成员重名时，参数加 `_` 前缀：`explicit SpinLock(const char* _name) : name(_name) {}`

---

## 3. 类型选择

### `class` vs `struct`

| 用 `class` | 用 `struct` |
|------------|-------------|
| 有行为（方法修改状态） | 纯数据容器（POD） |
| 需要访问控制 | 所有成员逻辑上都是 public |
| 管理资源（RAII） | 可聚合初始化 |

示例：`class SpinLock`（有锁行为）、`class IoBuffer`（RAII）、`struct DmaRegion`（纯数据）、`struct LogEntry`（队列条目）

### `enum class`

```cpp
/// 内核错误码
enum class ErrorCode : uint64_t {
  kSuccess = 0,
  // SpinLock 相关错误 (0x300 - 0x3FF)
  kSpinLockRecursiveLock = 0x300,
  kSpinLockNotOwned = 0x301,
};
```

必须使用 scoped enum + 显式底层类型，值用 `kCamelCase`，按子系统分组并标注范围。

---

## 4. 类布局

以 `SpinLock` 为基准的典型布局：

```cpp
/**
 * @brief 类描述
 * @note 使用限制（编号列表）
 */
class ClassName {
 public:
  // 1) public 成员变量
  const char* name{"default"};

  // 2) public 方法（附 Doxygen 文档）
  [[nodiscard]] auto Method() -> Expected<void>;

  // 3) 构造/析构函数组（放在 public 最后）
  /// @name 构造/析构函数
  /// @{
  explicit ClassName(const char* _name);
  ClassName() = default;
  ClassName(const ClassName&) = delete;
  ClassName(ClassName&&) = default;
  auto operator=(const ClassName&) -> ClassName& = delete;
  auto operator=(ClassName&&) -> ClassName& = default;
  ~ClassName() = default;
  /// @}

 protected:
  // protected 成员变量
  std::atomic_flag locked_{ATOMIC_FLAG_INIT};

 private:
  // private 成员变量与常量
  static constexpr size_t kMaxItems = 64;
  uint8_t* data_{nullptr};
};
```

### 要点

- 访问修饰符顺序：`public` → `protected` → `private`，省略空的节。
- 构造/析构函数组用 `/// @name` ... `/// @{` ... `/// @}` 包裹，放在 public 区域最后。带参构造函数也放在组内。
- **六个特殊成员函数必须显式声明**（默认构造、拷贝构造、移动构造、拷贝赋值、移动赋值、析构）。
- RAII 类型必须 delete 拷贝；基类析构函数加 `virtual`。
- 赋值运算符用 trailing return type：`auto operator=(const T&) -> T&`
- 成员变量必须花括号初始化，不允许未初始化变量。

---

## 5. 方法签名

### Trailing Return Type（强制）

```cpp
auto Lock() -> Expected<void>;           // ✅
Expected<void> Lock();                    // ❌
```

构造函数和析构函数除外。

### 常用属性

| 属性 | 使用时机 |
|------|---------|
| `[[nodiscard]]` | 返回值需要检查时 |
| `[[maybe_unused]]` | 虚方法默认实现中的未使用参数 |
| `[[noreturn]]` | 不会返回的函数 |
| `__always_inline` | 性能关键路径（锁、日志热路径） |
| `explicit` | 单参数构造函数 |

多个属性的顺序：`[[nodiscard]] __always_inline auto Method() -> RetType`

---

## 6. 错误处理

### 可失败操作优先使用 `Expected<T>`

```cpp
// 返回错误
return std::unexpected(Error{ErrorCode::kSpinLockRecursiveLock});

// 返回成功（void）
return {};

// 返回成功（带值）
return some_value;
```

### 处理错误

```cpp
// 检查并传播
auto result = device.Open();
if (!result.has_value()) {
  return std::unexpected(result.error());
}

// RAII 场景用 .or_else()
mutex_.Lock().or_else([](auto&& err) {
  (void)err;
  while (true) { cpu_io::Pause(); }
  return Expected<void>{};
});
```

### 新增错误码

在 `src/include/expected.hpp` 中选择一个空闲范围（如 `0xD00 - 0xDFF`），添加 `kXxx` 枚举值并在 `GetErrorMessage()` 中补充对应 case。

### 禁止事项

**绝对禁止** `throw` / `try` / `catch` / `dynamic_cast` / `typeid`——freestanding 环境无运行时支持。

---

## 7. Doxygen 文档

### 类/结构体

```cpp
/**
 * @brief 一句话描述
 * @note 使用限制或注意事项
 * @pre  使用前提条件
 * @post 使用后保证
 */
```

`@brief` **必须**。`@note`, `@pre`, `@post` 按需添加。

### 方法

```cpp
/**
 * @brief 一句话描述
 * @tparam B      模板参数描述
 * @param  name   参数描述（空格对齐）
 * @return 类型   成功/失败描述
 * @pre  前置条件
 * @post 后置条件
 */
```

所有 public/protected 方法 **必须** 有 `@brief`、`@param`、`@return`。

### 成员变量

用 `///` 单行注释：

```cpp
/// 获得此锁的 core_id
std::atomic<size_t> core_id_{std::numeric_limits<size_t>::max()};
```

### 分组

用 `/// @name` 包裹相关声明（构造/析构函数组强制使用）：

```cpp
/// @name ANSI 转义码
/// @{
inline constexpr auto kReset = "\033[0m";
inline constexpr auto kRed = "\033[31m";
/// @}
```

### 禁止事项

单行注释使用 `///`，不要添加 `@brief`

---

## 8. 常量与变量

| 场景 | 写法 |
|------|------|
| 头文件 namespace 内 | `inline constexpr`（避免 ODR 问题） |
| .cpp 文件 / 内部链接 | `static constexpr` |
| 类成员 | `static constexpr` |

成员变量统一使用**花括号初始化**并提供默认值：

```cpp
std::atomic_flag locked_{ATOMIC_FLAG_INIT};
bool saved_intr_enable_{false};
uint8_t* data_{nullptr};
char name[32]{};
```

---

## 9. Freestanding 约束

- 内存子系统初始化前**禁止堆分配**。
- 使用 `sk_`（C）/ `kstd_`（C++）前缀的内核库替代标准库。
- 使用 ETL 固定容量容器：`etl::flat_map`, `etl::unordered_map`, `etl::string_view`。
- 原子操作显式指定 memory order（优先 `acquire` / `release`，避免 `seq_cst`）。
- `static_assert` 验证编译期不变量（结构体大小、对齐等）。

---

## 10. 禁止事项

| 禁止 | 原因 |
|------|------|
| `throw` / `try` / `catch` | 无异常运行时 |
| `dynamic_cast` / `typeid` | 无 RTTI |
| `std::string` / `std::vector` / `std::map` | 动态分配，非 freestanding |
| 未初始化成员变量 | 未定义行为 |
| 在接口头文件中放实现 | 分离到 `.cpp`（`__always_inline`、简单构造/析构函数和工具类解析器除外） |
| 裸 `new` / `delete` | 使用 RAII 或 `etl::unique_ptr` |
| C 风格转换 | 使用 `static_cast` / `reinterpret_cast` |
| 无注释的 `__builtin_unreachable()` | 必须附带 `// UNREACHABLE: 原因` |

---

## 附录: 检查清单

生成代码前确认：

- [ ] 版权声明 + `#pragma once`
- [ ] 头文件包含分三组
- [ ] 所有 class/struct 有 `@brief`
- [ ] 所有 public 方法有 `@brief` / `@param` / `@return`
- [ ] 方法使用 trailing return type
- [ ] 返回值的方法标注 `[[nodiscard]]`
- [ ] 单参数构造函数标注 `explicit`
- [ ] 构造/析构函数组用 `/// @name` 包裹，六个特殊成员显式声明
- [ ] `protected` / `private` 成员带 `_` 后缀，`public` 成员和 `struct` 成员不带
- [ ] 成员变量花括号初始化
- [ ] 可失败操作返回 `Expected<T>`
- [ ] 无异常、无 RTTI、无动态分配
