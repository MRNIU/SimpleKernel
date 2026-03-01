# ETL (Embedded Template Library) 在 SimpleKernel 中的应用总览

> **目标读者**：SimpleKernel 贡献者，希望了解如何利用已集成的 ETL 库改进或替换内核组件。

## 一、ETL 是什么

[ETL (Embedded Template Library)](https://www.etlcpp.com) 是一个专为嵌入式/裸机环境设计的 C++ 模板库，核心设计约束与 SimpleKernel 高度一致：

| ETL 约束 | SimpleKernel 约束 |
|---------|-----------------|
| 无动态内存分配（不使用堆）| 内存子系统初始化前禁止堆分配 |
| 无 RTTI (`dynamic_cast`, `typeid`) | 全局禁用 RTTI |
| 无异常 (`throw`/`catch`) | 全局禁用异常 |
| 仅头文件（header-only）| 符合内核 header-only 框架模式 |
| 固定容量容器（编译期确定大小）| `kstd::static_vector` 等同概念 |

ETL 已作为 Git submodule 集成在 `3rd/etl/`，并通过 `cmake/compile_config.cmake` 链接为 `etl::etl`。

## 二、当前配置

`src/include/etl_profile.h` 是 ETL 的内核专用配置文件：

```cpp
// 启用 C++23 支持
#define ETL_CPP23_SUPPORTED 1

// 禁用断言检查（内核环境无异常，由调用方保证正确性）
// 生产版本：禁用所有检查，最大性能
#define ETL_NO_CHECKS 1

// 调试版本（推荐在开发阶段使用）：
// #define ETL_NO_CHECKS 1   // 注释掉此行，启用 ETL 内部边界检查
// #define ETL_NO_EXCEPTIONS 1  // 不抛出异常，改为调用 error_handler
```

> **重要**：`ETL_NO_CHECKS` 禁用了 ETL 所有内部边界检查。内核代码**必须自行检查**前置条件（容器是否满、索引是否越界），不能依赖 ETL 的内部检查来防止错误。

### 生产 vs 调试配置

| 场景 | 配置 | 行为 |
|------|------|------|
| **生产/Release** | `ETL_NO_CHECKS 1` | 所有检查跳过，速度最快；越界=未定义行为 |
| **调试/Debug** | 注释掉 `ETL_NO_CHECKS`，启用 `ETL_NO_EXCEPTIONS` | ETL 发现问题时调用 `error_handler`，不崩溃但记录错误 |

调试时在 `etl_profile.h` 中注册 error_handler：

```cpp
// （仅调试期间——在 etl_profile.h 添加）
namespace etl {
  void error_handler::error(const etl::exception& e) {
    klog::Err("ETL error: %s at %s:%d\n", e.what(), e.file_name(), e.line_number());
    __builtin_unreachable();
  }
}
```

> **开发规范**：无论 `ETL_NO_CHECKS` 是否开启，内核代码**必须主动检查**（`full()`、`size()`、`is_valid()` 等）。`ETL_NO_CHECKS` 只是去掉了 ETL 的内部 fallback，不是允许省略调用方检查的理由。

## 三、可替换/增强的组件速览

| 内核现有组件 | 文件 | ETL 替代/增强 | 文档 |
|------------|------|--------------|------|
| `io::In<T>` / `io::Out<T>` | `src/include/io.hpp` | `etl::io_port_ro/wo/rw` | [etl_io_port.md](etl_io_port.md) |
| `kstd::static_vector`, `kstd::static_list` 等 | `src/task/include/task_manager.hpp` | `etl::vector`, `etl::list`, `etl::map` 等 | [etl_containers.md](etl_containers.md) |
| `InterruptFunc` 裸函数指针 | `src/include/interrupt_base.h` | `etl::delegate` | [etl_delegate_fsm.md](etl_delegate_fsm.md) |
| `TaskStatus` 枚举状态机 | `src/task/include/task_control_block.hpp` | `etl::fsm` | [etl_delegate_fsm.md](etl_delegate_fsm.md) |
| `CloneFlags` / `cpu_affinity` 位运算 | `src/task/include/task_control_block.hpp` | `etl::flags` / `etl::bitset` | [etl_delegate_fsm.md](etl_delegate_fsm.md) |
| `std::variant<...>` 驱动匹配键 | `src/device/include/driver_registry.hpp` | `etl::variant` | [etl_containers.md](etl_containers.md) |
| `Expected<T>` (std::expected 别名) | `src/include/expected.hpp` | 现有实现已足够；`etl::expected` 可作备选 | — |

## 四、不建议替换的组件

| 组件 | 原因 |
|------|------|
| `SpinLock` | 依赖 `cpu_io::GetCurrentCoreId()` 和 `cpu_io::Pause()`，与硬件强耦合，ETL mutex 后端不适用 |
| `Mutex` | 依赖任务调度子系统 (`Block`/`Wakeup`)，ETL 无此机制 |
| `Expected<T>` | 项目已全面使用 `std::expected`（C++23 标准），无需替换 |
| `kstd::priority_queue` 睡眠队列 | 与 `TaskControlBlock::WakeTickCompare` 深度耦合 |

## 五、引用 ETL 的正确方式

在内核代码中使用 ETL 时，**必须先包含配置文件**：

```cpp
// 正确：先包含配置文件
#include "etl_profile.h"
#include <etl/io_port.h>
#include <etl/vector.h>

// 错误：直接包含 ETL 头文件（会使用默认配置，可能引入不需要的功能）
#include <etl/vector.h>
```

## 六、各专题文档

1. **[etl_io_port.md](etl_io_port.md)** — `etl::io_port` 详解：类型安全 MMIO，替换 `io.hpp`
2. **[etl_containers.md](etl_containers.md)** — 固定容量容器详解：`etl::vector`、`etl::list`、`etl::map`、`etl::variant` 等
3. **[etl_delegate_fsm.md](etl_delegate_fsm.md)** — `etl::delegate`、`etl::fsm`、`etl::flags` 详解：中断回调、任务状态机、位标志

---
