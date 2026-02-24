# AGENTS.md — SimpleKernel

## 项目概述

SimpleKernel 是一个**面向 AI 辅助学习的现代化操作系统内核项目**。使用 **C++23/C23** 编写，支持 **x86_64**、**RISC-V 64** 和 **AArch64** 三种架构。

### 核心设计理念：接口驱动（Interface-Driven）

- **项目主体是接口定义**——头文件（`.h/.hpp`）只包含类声明、纯虚接口、类型定义和 Doxygen 文档
- **实现由 AI 完成**——用户阅读接口契约，AI 根据头文件生成 `.cpp` 实现
- **现有代码是参考实现**——用于验证 AI 生成代码的正确性，不是唯一正确答案
- **测试套件验证契约**——GoogleTest 测试用例验证实现是否符合接口要求

### 你作为 AI 的角色

当用户要求你实现某个模块时：
1. **先阅读对应的接口头文件**——理解类声明、纯虚方法、Doxygen 契约（`@pre`/`@post`/`@note`）
2. **在独立的 `.cpp` 文件中生成实现**——不要修改头文件中的接口定义
3. **确保实现符合契约**——满足前置条件检查、后置条件保证、不引入额外的公共接口
4. **运行测试验证**——`make unit-test` 或 `make SimpleKernel && make run`

### 关键接口文件（你最常需要阅读的文件）

| 接口文件 | 职责 | 对应实现 |
|---------|------|---------|
| `src/arch/arch.h` | 架构无关的统一入口 | `src/arch/{arch}/` 各文件 |
| `src/include/interrupt_base.h` | 中断子系统抽象基类 | `src/arch/{arch}/interrupt.cpp` |
| `src/device/include/device_manager.hpp` | 设备管理器 | header-only |
| `src/device/include/driver_registry.hpp` | 驱动注册中心 | header-only |
| `src/device/include/platform_bus.hpp` | 平台总线（FDT 枚举） | header-only |
| `src/device/include/driver/ns16550a_driver.hpp` | NS16550A UART 驱动 | header-only（Probe/Remove 模式） |
| `src/include/virtual_memory.hpp` | 虚拟内存管理 | `src/virtual_memory.cpp` |
| `src/include/kernel_fdt.hpp` | 设备树解析 | `src/kernel_fdt.cpp` |
| `src/include/kernel_elf.hpp` | ELF 解析 | `src/kernel_elf.cpp` |
| `src/task/include/scheduler_base.hpp` | 调度器抽象基类 | `src/task/*_scheduler.cpp` |
| `src/include/spinlock.hpp` | 自旋锁 | header-only（性能要求） |
| `src/include/mutex.hpp` | 互斥锁 | `src/task/mutex.cpp` |

## 构建命令

```bash
# 前置条件: GCC 交叉工具链, QEMU, CMake 3.27+
# Docker 替代方案: ptrnull233/simple_kernel:latest
git submodule update --init --recursive   # 首次克隆后必须执行

# 配置 (选择架构: riscv64 | aarch64 | x86_64)
cmake --preset build_riscv64

# 编译内核 (目标名称是 "SimpleKernel"，不是 "kernel")
cd build_riscv64 && make SimpleKernel

# 在 QEMU 中运行
make run

# 调试模式 (GDB 连接 localhost:1234)
make debug

# 运行单元测试 (架构需要与 HOST 一致)
cmake --preset build_x86_64
cd build_x86_64 && make unit-test

# 单元测试 + 覆盖率报告
cd build_x86_64 && make coverage

# 运行单个测试 (需先构建 unit-test 目标)
cd build_x86_64 && ./tests/unit_test/unit-test --gtest_filter="TestSuiteName.TestName"

# 格式检查 (pre-commit 在 commit 时自动运行)
pre-commit run --all-files
```

**aarch64 特殊要求**: 运行前需要先启动两个串口终端任务 (`::54320` 和 `::54321`)。

## 测试框架

- **GoogleTest**，使用 `gtest_discover_tests()`。测试二进制文件: `tests/unit_test/unit-test`。
- 单元测试仅在宿主机运行 (`CMAKE_SYSTEM_PROCESSOR == CMAKE_HOST_SYSTEM_PROCESSOR`)。
- 测试编译选项: `--coverage`、`-fsanitize=leak`、`-fsanitize=address`。
- 系统测试在 QEMU 中运行；集成测试可能需要 OpenSBI/U-Boot。
- 添加模块时: libc/libcxx 需要 unit-test + system-test；架构/内核代码需要 system-test。

## 编码规范

### 格式化与静态检查

- **C/C++**: Google Style，通过 `.clang-format` (`BasedOnStyle: Google`) 自动格式化，pre-commit 强制执行。
- **静态分析**: `.clang-tidy` 启用 `bugprone-*`、`google-*`、`misc-*`、`modernize-*`、`performance-*`、`portability-*`、`readability-*`（含部分排除）。
- **CMake**: `cmake-format` + `cmake-lint`，配置文件 `.cmake-format.json`。命令**大写**，关键字**大写**，4 空格缩进，80 字符行宽，函数名与 `(` 之间有空格。
- **Shell**: `shellcheck`，通过 pre-commit 执行。

### 命名约定

| 元素 | 约定 | 示例 |
|------|------|------|
| 文件 | `snake_case` | `kernel_log.hpp` |
| 类/结构体 | `PascalCase` | `TaskManager` |
| 函数 | `PascalCase` 或 `snake_case` | `ArchInit()`、`sys_yield()` |
| 变量 | `snake_case` | `per_cpu_data` |
| 成员变量 | `snake_case_`（尾部下划线） | `locked_`、`core_id_` |
| 常量 | `kCamelCase` | `kPageSize`、`kReset` |
| 宏 | `SCREAMING_SNAKE_CASE` | `SIMPLEKERNEL_DEBUG` |
| 内核 libc/libcxx 头文件 | `sk_` 前缀 | `sk_cstdio`、`sk_vector` |

### Header Guards

使用 `#ifndef` / `#define` / `#endif`，格式为完整路径:
```cpp
#ifndef SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
#define SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_
// ...
#endif /* SIMPLEKERNEL_SRC_INCLUDE_KERNEL_LOG_HPP_ */
```

### 版权头

每个文件开头:
```cpp
/**
 * @copyright Copyright The SimpleKernel Contributors
 */
```
CMake 文件使用: `# Copyright The SimpleKernel Contributors`

### Include 顺序

- 系统/标准头文件在前 (`<atomic>`、`<cstdint>`)，然后第三方 (`<MPMCQueue.hpp>`)，最后项目头文件 (`"arch.h"`、`"kernel_log.hpp"`)。
- 接口头文件只包含声明所需的头文件；实现所需的头文件放在 `.cpp` 中。
- 使用内核自己的 libc/libcxx (`sk_cstdio`、`sk_vector`)，**禁止**使用标准库的动态内存分配。

### 类型与语言特性

- **C++23** 和 **C23** 标准。编译选项: `-ffreestanding`、`-fno-rtti`、`-fno-exceptions`。
- 禁止 RTTI 和异常。使用 `Expected<T>`（基于 `std::expected`）进行错误处理。
- 适当使用 `std::atomic`、`std::concepts`、`std::source_location`。
- 尾部返回类型: `auto FunctionName() -> ReturnType`。
- 性能关键函数使用 `__always_inline`，允许保留在头文件中（如 `SpinLock::Lock()`）。
- 适当使用 `[[maybe_unused]]`、`[[nodiscard]]` 属性。
- 不可达代码路径使用 `__builtin_unreachable()`。

### 错误处理

- 返回 `Expected<T>`（`std::expected<T, Error>` 的别名）——禁止抛出异常。
- `Error` 类型包含 `ErrorCode` 枚举和 `.message()` 方法。
- 检查返回值；关键失败时记录日志并停机 (`while (true) { cpu_io::Pause(); }`)。

### 接口文件规范（最重要）

- 头文件**只包含**: 类声明、纯虚接口、类型定义、常量。
- **禁止在头文件中实现方法**（例外: 标记 `__always_inline` 的性能关键方法）。
- 每个类和方法必须有 Doxygen 文档: `@brief`、`@pre`（前置条件）、`@post`（后置条件）。
- 实现放在独立的 `.cpp` 文件中——禁止修改接口头文件来添加实现。
- 例外：与内核核心无关的工具类（如 `kernel_elf`、`kernel_fdt` 等解析器）可以将实现直接内联在 `.hpp` 头文件中，无需单独的 `.cpp` 文件。

## Git Commit 规范

使用 --signoff 进行提交

```
<type>(<scope>): <subject>

type: feat|fix|bug|docs|style|refactor|perf|test|build|revert|merge|sync|comment
scope: 可选，影响的模块 (如 arch, device, libc, task 等)
subject: 不超过50字符，不加句号
```

## 项目结构

```
src/include/               # 公共接口头文件（先读这里）
src/arch/                  # 架构相关代码 (aarch64/riscv64/x86_64)
src/arch/arch.h            # 架构无关统一接口
src/device/                # 设备管理框架
src/device/include/        # 设备框架接口 (DeviceManager, DriverRegistry, Bus 等)
src/device/include/driver/ # 具体驱动 (ns16550a_driver.hpp, virtio_blk_driver.hpp)
src/device/device.cpp      # 设备初始化入口 (DeviceInit)
src/task/                  # 任务管理 (调度器, TCB, 同步原语)
src/task/include/          # 调度器/任务接口
src/libc/                  # 内核 C 标准库 (sk_ 前缀)
src/libcxx/                # 内核 C++ 运行时 (sk_ 前缀)
tests/unit_test/           # GoogleTest 单元测试 (仅宿主机)
tests/integration_test/    # 集成测试
tests/system_test/         # 系统测试 (QEMU)
cmake/                     # 工具链文件、编译配置、辅助函数
3rd/                       # Git 子模块 (opensbi, u-boot, googletest, bmalloc, MPMCQueue, device_framework 等)
```

## 关键 API

```cpp
// 日志系统 (kernel_log.hpp)
klog::Debug("msg %d", val);    // Debug，带 source_location
klog::Info("msg\n");           // Info
klog::Warn("msg\n");           // Warning
klog::Err("msg\n");            // Error（注意：是 Err 不是 Error）
klog::info << "stream style\n"; // 流式 API

// 单例模式
Singleton<TaskManager>::GetInstance().AddTask(task);

// RAII 锁
LockGuard<SpinLock> guard(my_lock);

// 系统调用
sys_sleep(1000);  sys_yield();  sys_exit(0);

// 设备框架
Singleton<DriverRegistry>::GetInstance().Register<Ns16550aDriver>(Ns16550aDriver::GetDescriptor());
Singleton<DeviceManager>::GetInstance().RegisterBus(platform_bus);
Singleton<DeviceManager>::GetInstance().ProbeAll();
```

## 架构开发指南

### 添加新架构功能
1. 在 `src/arch/{arch}/` 目录下实现架构特定代码
2. 更新 `src/arch/arch.h` 中的统一接口
3. 必需文件: `boot.S`、`link.ld`、`arch_main.cpp`

### 添加新驱动
1. 在 `src/device/include/driver/` 下创建驱动头文件
2. 实现 `Probe(DeviceNode&)` / `Remove(DeviceNode&)` 接口和 `GetDescriptor()` 静态方法
3. 定义 `kMatchTable[]` 匹配表（FDT compatible 字符串）
4. 在 `DriverRegistry` 中注册驱动

### 添加测试
1. 每当添加新模块时，在 `tests/` 下创建测试文件
2. 使用 GoogleTest 编写测试用例
3. 更新相应的 `CMakeLists.txt` 以包含新测试
4. 如果是 libc/libcxx，需要创建 unit-test 与 system-test 两类测试
5. 如果是架构相关代码或内核代码，需要创建 system-test 测试

### 引导链

| 架构 | 引导流程 |
|------|---------|
| x86_64 | U-Boot -> kernel.elf |
| riscv64 | U-Boot SPL -> OpenSBI -> U-Boot -> kernel.elf |
| aarch64 | U-Boot -> ATF -> OP-TEE -> kernel.elf |

## 关键约束

- **禁止标准库堆分配** — 使用 `src/libc/` 和 `src/libcxx/` 中的实现。
- **`-ffreestanding` 环境** — 仅 freestanding 头文件可用（见 cppreference）。
- **禁止 RTTI 和异常** — 使用 `Expected<T>` 进行错误处理。
- **交叉编译** — 内核交叉编译；单元测试仅在宿主机架构运行。
- **引导链因架构而异**: x86_64 (U-Boot)、riscv64 (U-Boot SPL -> OpenSBI -> U-Boot)、aarch64 (U-Boot -> ATF -> OP-TEE)。

## 资源链接

- **文档目录**: `doc/` (工具链、系统启动、调试输出、中断)
- **Docker 使用**: `doc/docker.md`
- **Git 规范**: `doc/git_commit.md`
- **接口重构计划**: `doc/TODO_interface_refactor.md`
- **调试信息**: `build_{arch}/bin` 目录会自动生成 objdump、nm、map、dts 等文件，QEMU 的运行日志也在这里
