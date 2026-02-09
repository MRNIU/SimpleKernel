# SimpleKernel Copilot Instructions

## 项目概述

SimpleKernel 是一个**面向 AI 辅助学习的现代化操作系统内核项目**。使用 **C++23** 编写，支持 **x86_64**、**RISC-V 64** 和 **AArch64** 三种架构。

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
| `src/driver/include/console_driver.h` | 控制台驱动抽象 | `ns16550a.cpp` / `pl011.cpp` |
| `src/include/virtual_memory.hpp` | 虚拟内存管理 | `src/virtual_memory.cpp` |
| `src/include/kernel_fdt.hpp` | 设备树解析 | `src/kernel_fdt.cpp` |
| `src/include/kernel_elf.hpp` | ELF 解析 | `src/kernel_elf.cpp` |
| `src/task/include/scheduler_base.hpp` | 调度器抽象基类 | `src/task/*_scheduler.cpp` |
| `src/include/spinlock.hpp` | 自旋锁 | header-only（性能要求） |
| `src/include/mutex.hpp` | 互斥锁 | `src/task/mutex.cpp` |

## 技术栈

| 组件 | 技术选型 |
|------|---------|
| 语言标准 | C23 / C++23 |
| 构建系统 | CMake 3.27+ (CMakePresets) |
| 编译器 | GCC 交叉编译工具链 |
| 模拟器 | QEMU |
| 代码风格 | Google Style (clang-format/clang-tidy) |
| 测试框架 | GoogleTest |
| 容器化 | Docker (`ptrnull233/simple_kernel:latest`) |

## 项目结构

```
SimpleKernel/
├── src/                    # 内核源码
│   ├── include/            # 📐 公共接口头文件（项目核心，你应该先读这里）
│   ├── arch/               # 架构相关代码 (aarch64/riscv64/x86_64)
│   │   ├── arch.h          # 📐 架构无关统一接口
│   │   └── {arch}/         # 每架构: boot.S, link.ld, arch_main.cpp 等
│   ├── driver/             # 设备驱动
│   │   ├── include/        # 📐 驱动接口 (console_driver.h 等)
│   │   └── ...             # 驱动实现 (ns16550a/pl011/gic/plic/apic 等)
│   ├── task/               # 任务管理
│   │   ├── include/        # 📐 调度器接口 (scheduler_base.hpp 等)
│   │   └── ...             # 调度器和任务管理实现
│   ├── libc/               # 内核 C 标准库实现
│   └── libcxx/             # 内核 C++ 运行时
├── tests/                  # 🧪 测试套件（验证实现是否符合接口契约）
│   ├── unit_test/          # 单元测试
│   ├── integration_test/   # 集成测试
│   └── system_test/        # 系统测试（QEMU 运行）
├── cmake/                  # CMake 模块和工具链文件
│   ├── {arch}-gcc.cmake    # 交叉编译工具链定义
│   ├── compile_config.cmake # 编译选项配置
│   ├── functions.cmake      # 项目使用的辅助函数
│   ├── project_config.cmake # 项目配置生成
│   └── 3rd.cmake           # 第三方依赖管理
├── 3rd/                    # 第三方依赖 (Git Submodule)
├── tools/                  # 构建工具和模板
└── doc/                    # 文档
```

## 构建命令

```bash
# 0. 确保子模块已初始化 (首次克隆后必须执行)
git submodule update --init --recursive

# 1. 配置 (选择架构: build_riscv64 / build_aarch64 / build_x86_64)
cmake --preset build_{arch}

# 2. 编译内核 (目标名称是 SimpleKernel，不是 kernel)
cd build_{arch} && make SimpleKernel

# 3. 在 QEMU 中运行
make run

# 4. 调试模式 (GDB 连接 localhost:1234)
make debug

# 5. 运行单元测试 ({arch} 需要与 HOST 架构一致)
cmake --preset build_{arch}
cd build_{arch} && make unit-test coverage
```

**VS Code 任务**: 使用 `Tasks: Run Task` 选择 `build_{arch}` 或 `run_{arch}`。

**⚠️ aarch64 特殊要求**: 运行前需要先启动两个串口终端任务 (`::54320` 和 `::54321`)。

## 编码规范

### 接口文件规范（最重要）

当创建或修改接口头文件时，必须遵循以下规范：

- **只包含声明**：类声明、纯虚接口、类型定义、常量，不包含方法实现
- **Doxygen 契约文档**：每个类和方法必须包含 `@brief`、`@pre`（前置条件）、`@post`（后置条件）
- **最小化 include**：接口头文件只包含声明所需的头文件，实现所需的头文件放在 `.cpp` 中
- **性能例外**：标记 `__always_inline` 的方法（如 `SpinLock::Lock()`）允许保留在头文件中

### 代码风格
- **格式化**: Google Style，使用 `.clang-format` 自动格式化
- **静态检查**: 使用 `.clang-tidy` 配置
- **Pre-commit**: 自动执行 clang-format、cmake-format、shellcheck

### Git Commit 规范
```
<type>(<scope>): <subject>

type: feat|fix|bug|docs|style|refactor|perf|test|build|revert|merge|sync|comment
scope: 可选，影响的模块 (如 arch, driver, libc)
subject: 不超过50字符，不加句号
```

### 命名约定
- **文件**: 小写下划线 (`kernel_log.hpp`)
- **类/结构体**: PascalCase (`TaskManager`)
- **函数**: PascalCase (`ArchInit`) 或 snake_case (`sys_yield`)
- **变量**: snake_case (`per_cpu_data`)
- **宏**: SCREAMING_SNAKE_CASE (`SIMPLEKERNEL_DEBUG`)
- **常量**: kCamelCase (`kPageSize`)
- **内核专用 libc/libc++ 头文件**: 使用 `sk_` 前缀 (`sk_cstdio`, `sk_vector`)

## 架构开发指南

### 添加新架构功能
1. 在 `src/arch/{arch}/` 目录下实现架构特定代码
2. 更新 `src/arch/arch.h` 中的统一接口
3. 必需文件: `boot.S`, `link.ld`, `arch_main.cpp`

### 添加新驱动
1. 在 `src/driver/` 下创建驱动目录
2. 实现驱动并更新 `src/driver/CMakeLists.txt`
3. 在架构初始化代码中调用驱动初始化

### 添加测试
1. 每当你添加一个新模块时，在 `tests/` 下创建测试文件
2. 使用 GoogleTest 编写测试用例
3. 更新相应的 `CMakeLists.txt` 以包含新测试
4. 如果是 libc/libcxx，需要创建 unit-test 与 system-test 两类测试
5. 如果是架构相关代码或内核代码，需要创建 system-test 测试

### 引导链
| 架构 | 引导流程 |
|------|---------|
| x86_64 | U-Boot → kernel.elf |
| riscv64 | U-Boot SPL → OpenSBI → U-Boot → kernel.elf |
| aarch64 | U-Boot → ATF → OP-TEE → kernel.elf |

## 关键 API

### 日志系统 (`kernel_log.hpp`)
```cpp
klog::Debug("message %d", value);
klog::Info("info message\n");
klog::Warn("warning\n");
klog::Error("error\n");
```

### 中断注册
```cpp
// 架构相关，参见各架构的 interrupt.h
RegisterInterruptHandler(irq_num, handler_func);
```

### 任务管理
```cpp
auto task = new TaskControlBlock("name", priority, func, arg);
Singleton<TaskManager>::GetInstance().AddTask(task);
```

## 注意事项

### 构建问题排查
- **子模块未初始化**: 运行 `git submodule update --init --recursive`
- **工具链缺失**: 使用 Docker 环境或参考 `doc/0_工具链.md`
- **aarch64 运行需要**: 先启动 VS Code 任务 `::54320` 和 `::54321` (串口终端)

### 常见陷阱
- 内核代码中禁止使用标准库的动态内存分配，使用 `libc/` 和 `libcxx/` 中的实现
- 不同架构的 `_start` 参数含义不同 (见 `kernel.h` 注释)
- 编译选项使用 `-ffreestanding`，在 https://en.cppreference.com/w/cpp/freestanding.html 查阅可用库函数

## 资源链接

- **文档目录**: `doc/` (工具链、系统启动、调试输出、中断)
- **Docker 使用**: `doc/docker.md`
- **Git 规范**: `doc/git_commit.md`
- **接口重构计划**: `doc/TODO_interface_refactor.md`
- **调试信息**: `build_{arch}/bin` 目录会自动生成 objdump、 nm、map、dts 等文件，qemu 的运行日志也在这里
