<<<<<<< HEAD
[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-interrupt](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/main)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)
||||||| bd1a0006
=======
![github ci](https://github.com/Simple-XX/SimpleKernel/workflows/CMake/badge.svg) ![last-commit](https://img.shields.io/github/last-commit/google/skia.svg) ![languages](https://img.shields.io/github/languages/count/badges/shields.svg) ![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg) [![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE) [![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)
>>>>>>> upstream/main

<<<<<<< HEAD
[English](./README_ENG.md) | [中文](./README.md)
||||||| bd1a0006
# boot
=======
[English](./README_en.md) | [中文](./README.md)
>>>>>>> upstream/main

# SimpleKernel

<<<<<<< HEAD
**面向 AI 的操作系统学习项目 | Interface-Driven OS Kernel for AI-Assisted Learning**

> 🤖 **设计理念**：定义清晰的内核接口，由 AI 完成实现——学习操作系统的新范式

## 📖 目录

- [✨ 项目简介](#-项目简介)
- [🤖 面向 AI 的设计理念](#-面向-ai-的设计理念)
- [🏛️ 接口体系总览](#️-接口体系总览)
- [🏗️ 支持架构](#️-支持架构)
- [🚀 快速开始](#-快速开始)
- [📂 项目结构](#-项目结构)
- [🎯 学习路线](#-学习路线)
- [📦 第三方依赖](#-第三方依赖)
- [📝 开发指南](#-开发指南)
- [🤝 贡献指南](#-贡献指南)
- [📄 许可证](#-许可证)

## ✨ 项目简介

SimpleKernel 是一个**面向 AI 辅助学习的现代化操作系统内核项目**。采用 C++23 编写，支持 x86_64、RISC-V 64 和 AArch64 三种架构。

与传统 OS 教学项目不同，SimpleKernel 采用**接口驱动（Interface-Driven）** 的设计：

- **项目主体是接口定义**——完整的头文件（`.h/.hpp`）包含类声明、纯虚接口、类型定义、Doxygen 文档
- **实现由 AI 完成**——你只需要理解接口契约，让 AI 根据接口文档生成实现代码
- **参考实现可供对照**——项目提供完整的参考实现，用于验证 AI 生成代码的正确性

### 🌟 核心亮点

| 特性 | 说明 |
|------|------|
| 🤖 **AI-First 设计** | 接口文档即 prompt，AI 可直接根据头文件生成完整实现 |
| 📐 **接口与实现分离** | 头文件只有声明和契约，实现在独立的 `.cpp` 中 |
| 🌐 **三架构支持** | x86_64、RISC-V 64、AArch64，同一套接口适配不同硬件 |
| 🧪 **测试驱动验证** | GoogleTest 测试套件验证 AI 生成的实现是否符合接口契约 |
| 📖 **完整 Doxygen 文档** | 每个接口都有职责描述、前置条件、后置条件、使用示例 |
| 🏗️ **工程化基础设施** | CMake 构建、Docker 环境、CI/CD、clang-format/clang-tidy |

## 🤖 面向 AI 的设计理念

### 为什么要"面向 AI"？

传统 OS 教学项目的学习路径：**读代码 → 理解原理 → 模仿修改**。这种方式存在几个问题：

1. 内核代码量大，初学者容易迷失在实现细节中
2. 各模块耦合紧密，难以独立理解单个子系统
3. 从零实现一个模块的门槛很高，反馈周期长

SimpleKernel 提出一种新范式：**读接口 → 理解契约 → AI 实现 → 测试验证**

```
┌─────────────────────────────────────────────────────────┐
│                    SimpleKernel 学习流程                   │
│                                                         │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐         │
│   │ 📐 接口   │───▶│ 🤖 AI    │───▶│ 🧪 测试  │         │
│   │ 头文件    │    │ 生成实现  │    │ 验证正确性│         │
│   │ + Doxygen │    │ (.cpp)   │    │ GoogleTest│         │
│   └──────────┘    └──────────┘    └──────────┘         │
│        │                               │                │
│        │         ┌──────────┐          │                │
│        └────────▶│ 📚 参考   │◀─────────┘                │
│                  │ 实现对照  │                           │
│                  └──────────┘                           │
└─────────────────────────────────────────────────────────┘
```

### 核心工作流

#### 1️⃣ 阅读接口，理解契约

每个模块的头文件都包含完整的接口文档：

```cpp
/**
 * @brief 控制台驱动抽象基类
 *
 * 所有串口/控制台驱动必须实现此接口。
 *
 * @pre  硬件已完成基本初始化（时钟使能、引脚配置）
 * @post 调用 PutChar/GetChar 可进行字符级 I/O
 *
 * 已知实现：Ns16550a（RISC-V/x86_64）、Pl011（AArch64）
 */
class ConsoleDriver {
public:
  virtual ~ConsoleDriver() = default;
  virtual void PutChar(uint8_t c) const = 0;
  [[nodiscard]] virtual auto GetChar() const -> uint8_t = 0;
  [[nodiscard]] virtual auto TryGetChar() const -> uint8_t = 0;
};
```

#### 2️⃣ 让 AI 实现

将头文件作为上下文提供给 AI（如 GitHub Copilot、ChatGPT、Claude 等），要求其生成 `.cpp` 实现。接口的 Doxygen 注释就是最好的 prompt。

#### 3️⃣ 测试验证

运行项目自带的测试套件，验证 AI 生成的实现是否符合接口契约：

```shell
cmake --preset build_riscv64
cd build_riscv64 && make unit-test
```

#### 4️⃣ 对照参考实现

如果测试不通过，可以参考项目提供的参考实现进行对照和学习。

### 与 AI 工具的结合方式

| 场景 | 使用方式 |
|------|---------|
| **GitHub Copilot** | 打开头文件，在对应的 `.cpp` 中让 Copilot 自动补全实现 |
| **ChatGPT / Claude** | 将头文件内容粘贴为上下文，要求生成完整的 `.cpp` 实现 |
| **Copilot Chat / Cursor** | 在 IDE 中选中接口，要求 AI 解释契约含义或生成实现 |
| **自主学习** | 先独立思考实现思路，再让 AI 生成，对比差异 |

## 🏛️ 接口体系总览

SimpleKernel 的接口按功能分为以下层次：

```
┌──────────────────────────────────────────┐
│              应用/系统调用层               │
│         syscall.h · SyscallInit          │
├──────────────────────────────────────────┤
│               任务管理层                  │
│  TaskManager · SchedulerBase · Mutex     │
│  CfsScheduler · FifoScheduler · RR ...   │
├──────────────────────────────────────────┤
│               内存管理层                  │
│  VirtualMemory · PhysicalMemory          │
│  MapPage · UnmapPage · AllocFrame        │
├──────────────────────────────────────────┤
│               中断/异常层                 │
│  InterruptBase · RegisterInterruptFunc   │
│  TimerInit · InterruptInit               │
├──────────────────────────────────────────┤
│               驱动层                      │
│  ConsoleDriver · Ns16550a · Pl011        │
│  Gic · Plic · Apic · Timer drivers       │
├──────────────────────────────────────────┤
│             架构抽象层 (arch.h)            │
│  ArchInit · InterruptInit · TimerInit    │
│  EarlyConsole（全局构造阶段自动设置）      │
├──────────────────────────────────────────┤
│            运行时支持库                    │
│  libc (sk_cstdio, sk_cstring, ...)       │
│  libcxx (sk_vector, __cxa_*, ...)        │
├──────────────────────────────────────────┤
│            硬件 / QEMU                    │
│  x86_64 · RISC-V 64 · AArch64           │
└──────────────────────────────────────────┘
```

### 关键接口文件

| 接口文件 | 职责 | 实现文件 |
|---------|------|---------|
| `src/arch/arch.h` | 架构无关的统一入口 | 各 `src/arch/{arch}/` 目录 |
| `src/include/interrupt_base.h` | 中断子系统抽象基类 | `src/arch/{arch}/interrupt.cpp` |
| `src/driver/include/console_driver.h` | 控制台驱动抽象 | `ns16550a.cpp` / `pl011.cpp` |
| `src/include/virtual_memory.hpp` | 虚拟内存管理接口 | `src/virtual_memory.cpp` |
| `src/include/kernel_fdt.hpp` | 设备树解析接口 | `src/kernel_fdt.cpp` |
| `src/include/kernel_elf.hpp` | ELF 解析接口 | `src/kernel_elf.cpp` |
| `src/task/include/scheduler_base.hpp` | 调度器抽象基类 | `cfs_scheduler.cpp` 等 |
| `src/include/spinlock.hpp` | 自旋锁接口 | header-only（性能要求） |
| `src/include/mutex.hpp` | 互斥锁接口 | `src/task/mutex.cpp` |

> 📋 完整接口重构计划见 [doc/TODO_interface_refactor.md](./doc/TODO_interface_refactor.md)

## 🏗️ 支持架构

| 架构 | 引导链 | 串口 | 中断控制器 | 时钟 |
|:---:|:---:|:---:|:---:|:---:|
| **x86_64** | U-Boot | COM1 | 8259A PIC | 8253/8254 |
| **RISC-V 64** | U-Boot + OpenSBI | SBI Call | Direct 模式 | SBI Timer |
| **AArch64** | U-Boot + ATF + OP-TEE | PL011 | GICv3 | Generic Timer |

## 🚀 快速开始

### 📋 系统要求

- **操作系统**: Linux (推荐 Ubuntu 24.04) 或 macOS
- **容器引擎**: Docker 20.10+
- **工具链**: 已包含在 Docker 镜像中（GCC 交叉编译器、CMake、QEMU 等）
- **AI 工具（推荐）**: GitHub Copilot / ChatGPT / Claude

### 🛠️ 环境搭建

**方式一：使用 Docker（推荐）**

```shell
# 1. 克隆项目
git clone https://github.com/simple-xx/SimpleKernel.git
cd SimpleKernel
git submodule update --init --recursive

# 2. 启动开发环境
docker pull ptrnull233/simple_kernel:latest
docker run --name SimpleKernel-dev -itd -p 233:22 \
  -v $(pwd):/root/SimpleKernel ptrnull233/simple_kernel:latest

# 3. 进入开发容器
docker exec -it SimpleKernel-dev /bin/zsh
```

**方式二：本地环境**

参考 [工具链文档](./doc/0_工具链.md) 配置本地开发环境。

### ⚡ 编译与运行

```shell
cd SimpleKernel

# 选择目标架构编译（以 RISC-V 64 为例）
cmake --preset build_riscv64
cd build_riscv64

# 编译内核
make SimpleKernel

# 在 QEMU 模拟器中运行
make run

# 运行单元测试（验证你的实现）
make unit-test
```

**支持的架构预设：**
- `build_riscv64` - RISC-V 64 位架构
- `build_aarch64` - ARM 64 位架构
- `build_x86_64` - x86 64 位架构

### 🎯 AI 辅助开发工作流

```shell
# 1. 在 VS Code 中打开项目（推荐安装 GitHub Copilot 扩展）
code ./SimpleKernel

# 2. 阅读头文件中的接口定义（例如 src/include/virtual_memory.hpp）

# 3. 创建/编辑对应的 .cpp 文件，让 AI 根据接口生成实现

# 4. 编译验证
cd build_riscv64 && make SimpleKernel

# 5. 运行测试
make unit-test

# 6. 在 QEMU 中运行，观察行为
make run
```

## 📂 项目结构

```
SimpleKernel/
├── src/                        # 内核源码
│   ├── include/                # 📐 公共接口头文件（项目核心）
│   │   ├── virtual_memory.hpp  #   虚拟内存管理接口
│   │   ├── kernel_fdt.hpp      #   设备树解析接口
│   │   ├── kernel_elf.hpp      #   ELF 解析接口
│   │   ├── spinlock.hpp        #   自旋锁接口
│   │   ├── mutex.hpp           #   互斥锁接口
│   │   └── ...
│   ├── arch/                   # 架构相关代码
│   │   ├── arch.h              # 📐 架构无关统一接口
│   │   ├── aarch64/            #   AArch64 实现
│   │   ├── riscv64/            #   RISC-V 64 实现
│   │   └── x86_64/             #   x86_64 实现
│   ├── driver/                 # 设备驱动
│   │   ├── include/            # 📐 驱动接口（ConsoleDriver 等）
│   │   ├── ns16550a/           #   NS16550A 串口驱动实现
│   │   ├── pl011/              #   PL011 串口驱动实现
│   │   └── ...
│   ├── task/                   # 任务管理
│   │   ├── include/            # 📐 调度器接口（SchedulerBase 等）
│   │   └── ...                 #   调度器实现
│   ├── libc/                   # 内核 C 标准库
│   └── libcxx/                 # 内核 C++ 运行时
├── tests/                      # 🧪 测试套件
│   ├── unit_test/              #   单元测试
│   ├── integration_test/       #   集成测试
│   └── system_test/            #   系统测试（QEMU 运行）
├── doc/                        # 📚 文档
│   ├── TODO_interface_refactor.md  # 接口重构计划
│   └── ...
├── cmake/                      # CMake 构建配置
├── 3rd/                        # 第三方依赖（Git Submodule）
└── tools/                      # 构建工具和模板
```

> 📐 标记的目录/文件是**接口定义**——这是你需要重点阅读的内容。

## 🎯 学习路线

建议按以下顺序学习和实现各模块：

### 阶段 1：基础设施（Boot）

| 模块 | 接口文件 | 难度 | 说明 |
|------|---------|:---:|------|
| Early Console | `src/arch/arch.h` 注释 | ⭐ | 最早期的输出，理解全局构造 |
| 串口驱动 | `console_driver.h` | ⭐⭐ | 实现 `PutChar`/`GetChar`，理解 MMIO |
| 设备树解析 | `kernel_fdt.hpp` | ⭐⭐ | 解析硬件信息，理解 FDT 格式 |
| ELF 解析 | `kernel_elf.hpp` | ⭐⭐ | 符号表解析，用于栈回溯 |

### 阶段 2：中断系统（Interrupt）

| 模块 | 接口文件 | 难度 | 说明 |
|------|---------|:---:|------|
| 中断基类 | `interrupt_base.h` | ⭐⭐ | 理解中断处理的统一抽象 |
| 中断控制器 | 各架构驱动头文件 | ⭐⭐⭐ | GIC/PLIC/PIC 硬件编程 |
| 时钟中断 | `arch.h → TimerInit` | ⭐⭐ | 定时器配置，tick 驱动 |

### 阶段 3：内存管理（Memory）

| 模块 | 接口文件 | 难度 | 说明 |
|------|---------|:---:|------|
| 虚拟内存 | `virtual_memory.hpp` | ⭐⭐⭐ | 页表管理、地址映射 |
| 物理内存 | 相关接口 | ⭐⭐⭐ | 帧分配器、伙伴系统 |

### 阶段 4：任务管理（Thread/Task）

| 模块 | 接口文件 | 难度 | 说明 |
|------|---------|:---:|------|
| 自旋锁 | `spinlock.hpp` | ⭐⭐ | 原子操作，多核同步 |
| 互斥锁 | `mutex.hpp` | ⭐⭐⭐ | 基于任务阻塞的锁 |
| 调度器 | `scheduler_base.hpp` | ⭐⭐⭐ | CFS/FIFO/RR 调度算法 |

### 阶段 5：系统调用（Syscall）

| 模块 | 接口文件 | 难度 | 说明 |
|------|---------|:---:|------|
| 系统调用 | `arch.h → SyscallInit` | ⭐⭐⭐ | 用户态/内核态切换 |

## 📦 第三方依赖

| 依赖 | 用途 |
|------|------|
| [google/googletest](https://github.com/google/googletest.git) | 测试框架 |
| [charlesnicholson/nanoprintf](https://github.com/charlesnicholson/nanoprintf.git) | printf 实现 |
| [MRNIU/cpu_io](https://github.com/MRNIU/cpu_io.git) | CPU I/O 操作 |
| [riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git) | RISC-V SBI 实现 |
| [MRNIU/opensbi_interface](https://github.com/MRNIU/opensbi_interface.git) | OpenSBI 接口 |
| [u-boot/u-boot](https://github.com/u-boot/u-boot.git) | 通用引导程序 |
| [OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git) | OP-TEE 操作系统 |
| [ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git) | ARM 可信固件 |
| [dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git) | 设备树编译器 |

## 📝 开发指南

### 🎨 代码风格

- **语言标准**: C23 / C++23
- **编码规范**: [Google C++ Style Guide](https://zh-google-styleguide.readthedocs.io/en/latest/google-cpp-styleguide/contents.html)
- **自动格式化**: `.clang-format` + `.clang-tidy`
- **注释规范**: Doxygen 风格，接口文件必须包含完整的契约文档

### 命名约定

| 类型 | 风格 | 示例 |
|------|------|------|
| 文件 | 小写下划线 | `kernel_log.hpp` |
| 类/结构体 | PascalCase | `TaskManager` |
| 函数 | PascalCase / snake_case | `ArchInit` / `sys_yield` |
| 变量 | snake_case | `per_cpu_data` |
| 宏 | SCREAMING_SNAKE | `SIMPLEKERNEL_DEBUG` |
| 常量 | kCamelCase | `kPageSize` |
| 内核 libc/libc++ 头文件 | `sk_` 前缀 | `sk_cstdio` |

### 📋 Git Commit 规范

```
<type>(<scope>): <subject>

type: feat|fix|docs|style|refactor|perf|test|build|revert
scope: 可选，影响的模块 (arch, driver, libc)
subject: 不超过50字符，不加句号
```

### 📚 文档

- **工具链**: [doc/0_工具链.md](./doc/0_工具链.md)
- **系统启动**: [doc/1_系统启动.md](./doc/1_系统启动.md)
- **调试输出**: [doc/2_调试输出.md](./doc/2_调试输出.md)
- **中断**: [doc/3_中断.md](./doc/3_中断.md)
- **Docker**: [doc/docker.md](./doc/docker.md)
- **接口重构计划**: [doc/TODO_interface_refactor.md](./doc/TODO_interface_refactor.md)

## 🤝 贡献指南

我们欢迎所有形式的贡献！

### 🎯 贡献方式

| 方式 | 说明 |
|------|------|
| 🐛 **报告问题** | 通过 [GitHub Issues](https://github.com/Simple-XX/SimpleKernel/issues) 报告 Bug |
| 📐 **改进接口** | 提出更好的接口抽象和文档改进建议 |
| 🧪 **补充测试** | 为现有接口编写更完整的测试用例 |
| 📖 **完善文档** | 改进 Doxygen 注释、添加使用示例 |
| 🔧 **提交实现** | 提交接口的参考实现或替代实现 |

### 🔧 代码贡献流程

1. Fork 本仓库
2. 创建功能分支: `git checkout -b feat/amazing-feature`
3. 遵循代码规范进行开发
4. 确保所有测试通过
5. 提交变更: `git commit -m 'feat(scope): add amazing feature'`
6. 创建 Pull Request

## 📄 许可证

本项目采用多重许可证：

- **代码许可** - [MIT License](./LICENSE)
- **反 996 许可** - [Anti 996 License](https://github.com/996icu/996.ICU/blob/master/LICENSE)

---

<div align="center">

**⭐ 如果这个项目对您有帮助，请给我们一个 Star！**

**🤖 让 AI 帮你写内核，让你专注于理解操作系统原理！**

[🌟 Star 项目](https://github.com/Simple-XX/SimpleKernel) • [🐛 报告问题](https://github.com/Simple-XX/SimpleKernel/issues) • [💬 参与讨论](https://github.com/Simple-XX/SimpleKernel/discussions)

</div>
||||||| bd1a0006
支持：i386, x86_64, riscv64
=======
## 关键词

- kernel
- x86, x86_64, riscv64
- C, C++, AT&T ASM
- cmake
- multiboot2, opensbi

## 简介

提供了各个阶段完成度不同的内核，你可以从自己喜欢的地方开始。

各分支内容：

1. boot: 系统的启动，从引导程序到内核入口
2. printf: 基本的字符输出，便于调试
3. parse_boot_info: 对引导程序传递信息的初步解析
4. pmm: 物理内存初始化
5. vmm: 虚拟内存初始化
6. heap: 堆管理
7. lib: C++ std/stl 的部分支持
8. intr: 中断管理
9. 进程: TODO
10. 文件系统: TODO
11. 设备管理: TODO
12. 系统调用: TODO
13. 用户模式: TODO

## 开发环境

- 基本工具

    交叉编译器 `x86_64-elf-gcc`, `riscv64-unknown-elf-gcc`, `arm-none-eabi-gcc`

    调试工具 `x86_64-elf-gdb`, `riscv64-unknown-elf-gdb`, `arm-none-eabi-gdb`

    构建工具 `cmake`

    模拟器 `bochs`, `qemu`

- For x86

    引导程序 `grub`
    
    制作内核镜像 `xorriso`
    
- For riscv64

    引导程序 `opensbi`
    
- For arm

    TODO

## 如何运行

```shell
git clone https://github.com/Simple-XX/SimpleKernel.git
cd SimpleKernel/
bash ./run.sh
```

运行截图

![](https://tva1.sinaimg.cn/large/00831rSTly1gdl6j8bxw7j317s0u0td9.jpg)

## 目录结构

- 原则

    整个工程按照功能模块划分子目录，每个子目录再划分头文件和源文件目录，以便架构清晰、易懂。

### 目录设计

- 原则

    目录的命名能准确描述模块的基本功能，建议用小写字母且不含下划线、点等特殊符号；

    目录必须放于相包含的父目录之下，并需要明确与其他目录间的耦合性。

- 示例

	kernel：系统内核部分；
	libs：依赖库；

### 依赖关系

- 原则

    新添加组件往往依赖于系统原有组件，必须以最小耦合的方式明确所直接依赖的组件。

### 头文件

#### 文件命名

头文件命名能准确描述文件所包含的模块内容，达到通俗、易懂的目的。

## CMake

## 测试

### 自动集成

每次 push 会使用 Github Action 进行测试，可以通过编译即可。

## 代码风格

- git commit 规范：

    [tools/Git Commit 规范.pdf](./tools/Git Commit 规范.pdf)

- 代码样式

    由 .clang-format 指定

## TODO

- 并发
- 文件系统
- 设备驱动

## 贡献者

[MRNIU](https://github.com/MRNIU)

[cy295957410](https://github.com/cy295957410)

[rakino](https://github.com/rakino)

[xiaoerlaigeid](https://github.com/xiaoerlaigeid)

[digmouse233](https://github.com/digmouse233)

[KehRoche](https://github.com/KehRoche)

## 贡献

Free to PR!

## 感谢

此项目参考了很多优秀的项目和资料。

[osdev](https://wiki.osdev.org)

[GRUB 在 Mac 上的安装](https://wiki.osdev.org/GRUB#Installing_GRUB_2_on_OS_X)

[multiboot](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html)

《程序员的自我修养--链接、装载与库》(俞甲子 石凡 潘爱民)

[JamesM's kernel development tutorials](http://www.jamesmolloy.co.uk/tutorial_html/1.-Environment%20setup.html)

[xOS](https://github.com/fengleicn/xOS)

[hurlex](http://wiki.0xffffff.org/posts/hurlex-8.html)

[howerj/os](https://github.com/howerj/os)

[cfenollosa/os-tutorial](https://github.com/cfenollosa/os-tutorial)

[omarrx024/xos](https://github.com/omarrx024/xos)

[PurpleBooth/project-title](https://gist.github.com/PurpleBooth/109311bb0361f32d87a2%23project-title)

[0xAX weblong](http://0xax.blogspot.com/search/label/asm)

[How-to-Make-a-Computer-Operating-System](https://github.com/SamyPesse/How-to-Make-a-Computer-Operating-System)

[coding-style](https://www.kernel.org/doc/Documentation/process/coding-style.rst)

[DNKernel](https://github.com/morimolymoly/DNKernel)

[c-algorithms](https://github.com/fragglet/c-algorithms)

[Linux内核中的中断栈与内核栈的补充说明](http://blog.chinaunix.net/uid-23769728-id-3077874.html)

[Linux进程管理 (1)进程的诞生](https://www.cnblogs.com/arnoldlu/p/8466928.html)

[SynestiaOS](https://github.com/SynestiaOS/SynestiaOS)

## 捐助者

- [digmouse233](https://github.com/digmouse233)
- l*e
- fslongjin

## 版权信息

此项目使用 MIT 许可证
>>>>>>> upstream/main
