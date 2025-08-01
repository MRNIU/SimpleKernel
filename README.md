[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-interrupt](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/interrupt)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

**现代化多架构内核操作系统 | 支持 x86_64、RISC-V 64 和 AArch64**

> 🚀 **当前分支状态**: interrupt - 已完成中断系统实现、多架构中断控制器支持和时钟中断处理

## 📖 目录

- [✨ 项目简介](#-项目简介)
- [🏗️ 支持架构](#️-支持架构)
- [🚀 快速开始](#-快速开始)
- [💻 核心特性](#-核心特性)
- [🎯 中断系统架构](#-中断系统架构)
- [⚡ 执行流程](#-执行流程)
- [📦 第三方依赖](#-第三方依赖)
- [📝 开发指南](#-开发指南)
- [🤝 贡献指南](#-贡献指南)
- [📄 许可证](#-许可证)

## ✨ 项目简介

SimpleKernel 是一个专为教育和研究设计的现代化操作系统内核，采用 C++ 编写，突出多架构支持和模块化架构设计。项目致力于为操作系统开发学习者和研究人员提供一个功能完整、代码清晰的实践平台。

### 🌟 核心亮点

- **🔧 现代 C++23 内核实现** - 充分利用现代 C++ 特性，提供类型安全和高性能
- **🌐 真正的多架构支持** - 原生支持 x86_64、RISC-V 64 和 AArch64 三大主流架构
- **⚡ 完整中断系统** - 实现了从底层硬件到高层处理的完整中断处理机制
- **🏗️ 工程化构建系统** - 基于 CMake 的现代化构建，支持交叉编译和多平台开发
- **🐳 容器化开发环境** - 预配置的 Docker 环境，一键启动开发
- **🔍 强大调试支持** - 集成栈回溯、符号解析和多级日志系统

## 🏗️ 支持架构

### 🏗️ 多架构支持矩阵

| 架构 | 引导链 | 串口输出 | 中断控制器 | 时钟中断 |
|:---:|:---:|:---:|:---:|:---:|
| **x86_64** | U-Boot | NS16550A | 8259A PIC | 8253/8254 Timer |
| **RISC-V 64** | U-Boot + OpenSBI | OpenSBI SBI Call | Direct 模式 | SBI Timer |
| **AArch64** | U-Boot + ATF + OP-TEE | PL011 UART | GICv3 | Generic Timer |

**架构特色：**
- **RISC-V**: 开放指令集，S 模式运行，完整的 CSR 寄存器抽象和基于 Direct 的中断处理
- **AArch64**: ARM 生态完整性，TrustZone 安全架构，GICv3 中断控制器驱动
- **x86_64**: 成熟的 PC 生态，8259A PIC 控制器，传统中断处理机制

## 🚀 快速开始

### 📋 系统要求

- **操作系统**: Linux (推荐 Ubuntu 24.04) 或 macOS
- **容器引擎**: Docker 20.10+
- **工具链**: 已包含在 Docker 镜像中（GCC 交叉编译器、CMake、QEMU 等）

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
make kernel

# 在 QEMU 模拟器中运行
make run
```

**支持的架构预设：**
- `build_riscv64` - RISC-V 64位架构
- `build_aarch64` - ARM 64位架构
- `build_x86_64` - x86 64位架构

### 🎯 VS Code 集成开发

项目已配置完整的 VS Code 开发环境：

```shell
# 在 VS Code 中打开项目
code ./SimpleKernel
```

- **一键编译**: 使用 `Ctrl+Shift+P` → `Tasks: Run Task` → 选择对应架构
- **调试支持**: 配置了 GDB 调试环境，支持源码级调试
- **代码补全**: 集成 C++ IntelliSense 和语法高亮

## 💻 核心特性

SimpleKernel 在当前 interrupt 分支实现了完整的中断处理系统，为现代 C++ 操作系统开发提供了坚实的技术底座。

### ⚡ 中断系统核心功能

#### 🔧 多架构中断控制器支持
- **x86_64 8259A PIC** - 传统可编程中断控制器，支持中断优先级和屏蔽
- **x86_64 8253/8254 Timer** - 精确的时钟中断生成和周期控制
- **RISC-V Direct 模式** - 基于 CSR 的直接中断处理，高效的中断响应
- **AArch64 GICv3** - 先进的通用中断控制器，支持消息信号中断和多核分发

#### 🎯 中断处理机制
- **统一中断注册** - 跨架构的中断处理函数注册接口
- **嵌套中断支持** - 中断深度跟踪和安全的嵌套中断处理
- **时钟中断调度** - 精确的系统时钟中断，为任务调度提供基础
- **中断上下文管理** - 完整的寄存器保存恢复和上下文切换

#### 🔍 CSR 寄存器抽象 (RISC-V)
- **寄存器封装** - 类型安全的 CSR 寄存器访问接口
- **状态监控** - 完整的寄存器状态打印和调试支持
- **特权级管理** - S 模式运行环境和权限控制
- **异常处理** - 中断和异常的统一处理框架

### 🏗️ 工程化构建系统

- **🔧 CMake 现代化构建** - 跨平台构建系统，支持多架构预设和交叉编译
- **📋 依赖管理** - Git Submodule 统一管理第三方组件，版本可控
- **🐳 容器化开发** - 预配置 Docker 环境，包含完整工具链和依赖
- **📖 详细文档** - 工具链安装指南：[doc/0_工具链.md](./doc/0_工具链.md)

### 📚 运行时支持库

#### 🛠️ C 标准库内核实现
为内核环境提供必要的 libc 函数实现：

- **内存操作** - `memcpy()`、`memmove()`、`memset()`、`memcmp()` 等高效内存操作
- **字符串处理** - `strcpy()`、`strcmp()`、`strlen()` 系列完整字符串函数
- **栈保护机制** - `__stack_chk_guard` 栈溢出检测和保护

#### ⚡ C++ 运行时环境
完整的内核 C++ 运行时支持：

- **🔄 对象生命周期** - `__cxa_atexit()`、`__cxa_finalize()` 全局对象构造析构管理
- **🔒 线程安全初始化** - `__cxa_guard_*` 静态局部变量线程安全机制
- **⚠️ 异常处理** - 基础异常捕获和 `throw/catch` 机制
- **📤 printf 支持** - 带颜色的格式化输出系统

### 🔍 高级调试与诊断

#### 📋 函数调用栈追踪
- **多架构栈回溯** - 支持 x86_64 (RBP)、RISC-V (FP)、AArch64 (X29) 帧指针链
- **符号解析集成** - 结合 ELF 符号表提供精确的函数名和地址映射
- **安全边界检查** - 限制在内核代码段范围，防止栈追踪越界

#### 📝 内核日志系统 (klog)
- **🌈 多级别彩色日志** - Debug/Info/Warn/Error 四级日志，ANSI 彩色输出
- **📍 源码精确定位** - 自动记录 `__func__`、`__LINE__` 调试信息
- **⚡ 高性能输出** - 优化的格式化输出，最小化性能影响

## 🎯 中断系统架构

### 💡 分支新增特性

本分支基于 boot 分支的完整基础架构，重点实现了中断处理系统。在此分支中，完成了多架构中断控制器驱动、中断注册机制、时钟中断处理等核心功能。

#### 🔧 RISC-V 64 架构增强
1. **CSR 寄存器抽象** - 完整的控制状态寄存器封装和类型安全访问
2. **寄存器状态打印** - 调试友好的寄存器状态输出和监控
3. **Direct 中断处理** - 基于 CSR 的高效直接中断处理机制
4. **中断注册函数** - 统一的中断处理函数注册和管理接口
5. **SBI 时钟中断** - OpenSBI 时钟中断服务集成

#### 🖥️ AArch64 架构增强
1. **中断注册函数** - ARM 架构中断处理函数注册机制
2. **Generic Timer** - ARM 通用定时器中断处理
3. **UART 中断** - PL011 串口中断处理支持
4. **GICv3 驱动** - 完整的通用中断控制器 v3 驱动实现

#### 💻 x86_64 架构增强
1. **CPU 抽象** - x86_64 处理器状态和功能抽象
2. **8259A PIC 控制器** - 传统可编程中断控制器完整驱动
3. **8253/8254 Timer** - 精确的定时器控制器驱动
4. **GDT 初始化** - 全局描述符表正确配置和管理
5. **中断处理流程** - 完整的中断入口、处理和退出机制
6. **中断注册函数** - x86 架构中断处理函数注册系统
7. **时钟中断** - 系统时钟中断处理和调度支持

#### 🚧 开发计划 (TODO)
- **RISC-V PLIC** - 平台级中断控制器支持，处理外部设备中断
- **x86_64 APIC** - 高级可编程中断控制器，支持多核和现代中断特性

## ⚡ 执行流程

### 🚀 系统启动流程

1. **引导阶段**
   - x86_64: U-Boot → Kernel
   - RISC-V: U-Boot → OpenSBI → Kernel (S-mode)
   - AArch64: U-Boot → ATF → OP-TEE → Kernel

2. **内核初始化**
   - C++ 运行时环境初始化
   - 硬件抽象层初始化
   - 中断控制器配置

3. **中断系统启动**
   - 中断控制器初始化
   - 中断向量表配置
   - 时钟中断启用

4. **主循环运行**
   - 系统服务就绪
   - 中断驱动的事件处理
   - 调试输出和状态监控

### 🔧 已支持的特性

- [x] **[BUILD]** 使用 CMake 的构建系统
- [x] **[BUILD]** 使用 gdb remote 调试
- [x] **[BUILD]** 第三方资源集成
- [x] **[COMMON]** C++ 全局对象的构造
- [x] **[COMMON]** C++ 静态局部对象构造
- [x] **[COMMON]** C 栈保护支持
- [x] **[COMMON]** printf 支持
- [x] **[COMMON]** 简单的 C++ 异常支持
- [x] **[COMMON]** 带颜色的字符串输出
- [x] **[INTERRUPT]** 多架构中断系统
- [x] **[INTERRUPT]** 时钟中断处理
- [x] **[INTERRUPT]** 中断注册机制
- [x] **[x86_64]** 基于 U-Boot 的引导
- [x] **[x86_64]** 基于 serial 的基本输出
- [x] **[x86_64]** 物理内存信息探测
- [x] **[x86_64]** 显示缓冲区探测
- [x] **[x86_64]** 调用栈回溯
- [x] **[x86_64]** 8259A PIC 中断控制器
- [x] **[x86_64]** 8253/8254 Timer 控制器
- [x] **[riscv64]** gp 寄存器的初始化
- [x] **[riscv64]** 基于 opensbi 的基本输出
- [x] **[riscv64]** device tree 硬件信息解析
- [x] **[riscv64]** ns16550a 串口驱动
- [x] **[riscv64]** 调用栈回溯(仅打印地址)
- [x] **[riscv64]** CSR 寄存器抽象
- [x] **[riscv64]** Direct 模式中断处理
- [x] **[aarch64]** GICv3 中断控制器驱动
- [x] **[aarch64]** Generic Timer 时钟中断
- [x] **[aarch64]** UART 中断处理
- [ ] **[aarch64]** 基于 U-Boot 的引导 (开发中)

## 📦 第三方依赖

- [google/googletest](https://github.com/google/googletest.git) - 测试框架
- [charlesnicholson/nanoprintf](https://github.com/charlesnicholson/nanoprintf.git) - printf 实现
- [MRNIU/cpu_io](https://github.com/MRNIU/cpu_io.git) - CPU I/O 操作
- [riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git) - RISC-V SBI 实现
- [MRNIU/opensbi_interface](https://github.com/MRNIU/opensbi_interface.git) - OpenSBI 接口
- [u-boot/u-boot](https://github.com/u-boot/u-boot.git) - 通用引导程序
- [OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git) - OP-TEE 操作系统
- [OP-TEE/optee_client](https://github.com/OP-TEE/optee_client.git) - OP-TEE 客户端
- [ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git) - ARM 可信固件
- [dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git) - 设备树编译器

## 📝 开发指南

### 🎨 代码风格规范
- **编码标准** - 严格遵循 [Google C++ 风格指南](https://zh-google-styleguide.readthedocs.io/en/latest/google-cpp-styleguide/contents.html)
- **自动格式化** - 预配置 `.clang-format`，使用 `clang-format` 自动格式化
- **命名约定** - 类名采用 PascalCase，函数和变量使用 snake_case
- **注释规范** - 使用 Doxygen 风格注释，支持自动文档生成

### 🚀 开发工作流
1. **Fork 项目** - 从主仓库创建个人分支
2. **本地开发** - 使用 Docker 环境进行开发和测试
3. **质量检查** - 运行静态分析和测试套件
4. **提交 PR** - 遵循提交信息规范，详细描述变更

### 📋 提交信息规范
```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型说明:**
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式调整
- `refactor`: 代码重构
- `test`: 测试相关
- `chore`: 构建工具或辅助工具变更

### 📚 文档自动部署
- **主分支部署** - GitHub Actions 自动将 main 分支文档部署到 [GitHub Pages](https://simple-xx.github.io/SimpleKernel/)
- **API 文档** - Doxygen 生成的完整 API 参考文档
- **开发文档** - 架构设计、开发指南和最佳实践

## 🤝 贡献指南

我们欢迎所有形式的贡献！无论是代码、文档、测试还是问题报告，都是推动项目发展的重要力量。

### 🎯 如何贡献

**🐛 报告问题**
- 使用 [GitHub Issues](https://github.com/Simple-XX/SimpleKernel/issues) 报告 Bug
- 详细描述问题重现步骤、环境信息和期望行为
- 附上相关日志和错误信息

**💡 功能建议**
- 通过 Issues 提出新功能建议
- 描述功能用途、实现思路和预期效果
- 讨论技术可行性和架构影响

**🔧 代码贡献**
1. Fork 本仓库到个人账户
2. 创建功能分支: `git checkout -b feature/amazing-feature`
3. 遵循代码规范进行开发
4. 添加必要的测试用例
5. 提交变更: `git commit -m 'feat: add amazing feature'`
6. 推送分支: `git push origin feature/amazing-feature`
7. 创建 Pull Request

### 📋 贡献者协议
- 确保代码质量和测试覆盖率
- 尊重现有架构和设计模式
- 积极参与代码评审和讨论

## 📄 许可证

本项目采用多重许可证：

- **代码许可** - [MIT License](./LICENSE)
- **反 996 许可** - [Anti 996 License](https://github.com/996icu/996.ICU/blob/master/LICENSE)

```
MIT License & Anti 996 License

Copyright (c) 2024 SimpleKernel Contributors

在遵循 MIT 协议的同时，本项目坚决反对 996 工作制度，
提倡健康的工作与生活平衡。
```

---

<div align="center">

**⭐ 如果这个项目对您有帮助，请给我们一个 Star！**

**🚀 让我们一起构建更好的操作系统内核！**

[🌟 Star 项目](https://github.com/Simple-XX/SimpleKernel) • [🐛 报告问题](https://github.com/Simple-XX/SimpleKernel/issues) • [💬 参与讨论](https://github.com/Simple-XX/SimpleKernel/discussions)

</div>
