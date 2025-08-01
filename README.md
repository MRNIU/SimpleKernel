[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-boot](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/boot)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

**一个现代的多架构内核操作系统，支持 x86_64、RISC-V 和 AArch64 架构**

> boot branch - SimpleKernel 的首个分支，完成了构建系统基础搭建、文档部署与自动化测试

## 📖 目录

- [简介](#简介)
- [支持的架构](#支持的架构)
- [快速开始](#快速开始)
- [核心特性](#核心特性)
- [系统架构](#系统架构)
- [第三方依赖](#第三方依赖)
- [开发指南](#开发指南)

## 📋 简介

SimpleKernel 是一个基于 C++ 的现代操作系统内核，专注于多架构支持和模块化设计。项目采用现代化的构建系统和完善的测试框架，为操作系统开发学习和研究提供了良好的基础平台。

### 关键特性
- 🔧 现代 C++ 内核实现
- 🏗️ 支持多架构（x86_64、RISC-V、AArch64）
- 🚀 基于 CMake 的构建系统
- 🐳 Docker 容器化开发环境
- 🧪 完善的测试框架（单元测试、集成测试、系统测试）
- 📚 自动化文档生成与部署

## 🚀 快速开始

### 环境准备

```shell
# 拉取代码
git clone https://github.com/simple-xx/SimpleKernel.git
git submodule update --init --recursive
# 拉取 Docker Image
docker pull ptrnull233/simple_kernel:latest
# 运行 Docker
docker run --name SimpleKernel-container -itd -p 233:22 -v ./SimpleKernel:/root/ ptrnull233/simple_kernel:latest
# 进入 Docker
docker exec -it SimpleKernel-container /bin/zsh
```

### 编译并运行

```shell
cd SimpleKernel
# 选择架构：build_riscv64/build_aarch64/build_x86_64/
cmake --preset build_riscv64
cd build_riscv64
# 编译内核
make kernel
# 在 qemu 中运行
make run
```

### 使用 VS Code

提供了用于运行、调试的 VS Code 相关配置，可以直接使用 VS Code 运行内核或进行调试。

## 🏗️ 系统架构

### 执行流程

[common_bootflow](https://www.plantuml.com/plantuml/png/dL9TIyCm57tU_HKXFewDiR6NWJ8tHDGDXiKdaPAs5nVCHymIsVpr9d6bgnqexg6ZvvwFqzpCTuvPvwK0nvr0ijHIQaKMMZkIuRj7LI9iaLLe2HsFnjFXb08mxxJoia0BKEWzcTYANApuwzRTMZo02PQyv8OfHuhW97JIQnkVO_8ClSiKi4euz0RX1prAdmOHfXHU05L5WZCGaW9engKH-81MeQ37h8NmsCawfan6AIOYmwn9o8iwe2LCXz1MIiRLi3JcH9jONN4WSSL_o7TlkU15kT-tFPR6t0LkroJ6_LOW8bqbi-Mscn_Hl6jn7U3p1NRIv7yjaGVoUOT_bSdMczREuUJE3Aw-jpfBboLD0fOM5i5xBmsabu3McmXujELCy4yaotwVF7hbk4HegB5DuAtZturozj2CwfC8uz3iE0LMElx172PbyrQJ0U8po9jzp4Zym5G5Qbhjtv1IHaEiRLej3gea6ysLWmhRFIhiDfcZghmKNm00)

### 支持的架构

| 架构 | 引导方式 | 基本输出 | 硬件资源探测 |
| :---: | :---: | :---: | :---: |
| x86_64 | u-boot | 通过 serial 实现 | 由 u-boot 传递 |
| riscv64 | u-boot+opensbi | 通过 opensbi 提供的 ecall 实现 | 由 u-boot 传递的 dtb |
| aarch64 | u-boot+atf+optee | 通过 serial 实现 | 由 u-boot+atf 传递的 dtb |

## 💻 核心特性

基于对源代码的深入分析，SimpleKernel 在本分支中实现了一个功能完整的多架构内核基础框架，支持现代 C++ 开发和完善的系统服务。

### 🏗️ 构建与项目管理

- **现代化构建系统**：基于 CMake 的跨平台构建，支持多架构预设配置
- **工具链文档**：详见 [doc/0_工具链.md](./doc/0_工具链.md)
- **依赖管理**：使用 git submodule 管理第三方组件
- **容器化开发**：Docker 环境支持，详见 [doc/docker.md](./doc/docker.md)

### 📚 运行时支持库

#### C 标准库实现
提供了内核必需的 libc 函数，包括：

- **内存操作**：`memcpy()`、`memmove()`、`memset()`、`memcmp()`、`memchr()`
- **字符串处理**：`strcpy()`、`strncpy()`、`strcat()`、`strcmp()`、`strlen()` 系列
- **类型转换**：`atoi()`、`strtol()`、`strtoul()` 等字符串到数值转换
- **字符分类**：`isalnum()`、`isalpha()`、`isdigit()` 等字符检查函数
- **安全机制**：栈保护相关函数 `__stack_chk_guard`、`__stack_chk_fail()`

#### C++ 运行时支持
实现了内核 C++ 环境的核心组件：

- **对象生命周期管理**：`__cxa_atexit()`、`__cxa_finalize()` 支持全局对象构造析构
- **静态局部变量支持**：`__cxa_guard_acquire()`、`__cxa_guard_release()` 线程安全初始化
- **内存管理运算符**：`operator new()`、`operator delete()` 系列的空实现
- **异常处理**：基础的 `__cxa_rethrow()` 异常处理
- **I/O 流支持**：自定义 iostream 实现，支持格式化输出

### 🖥️ 多架构支持

#### RISC-V 64位架构
- **引导链**：u-boot → opensbi → kernel，S态运行环境
- **寄存器管理**：gp寄存器正确初始化
- **系统调用接口**：基于 opensbi ecall 的系统服务
- **内核打包**：FIT (Flattened Image Tree) 格式支持

#### x86_64 架构
- **引导方式**：u-boot 直接引导，64位长模式运行
- **串口输出**：ns16550a 兼容串口驱动
- **FIT 打包**：统一的内核镜像格式

#### AArch64 架构
- **安全引导链**：u-boot → ARM Trusted Firmware → OP-TEE → kernel
- **TrustZone 支持**：集成 ATF 安全世界框架
- **设备树支持**：完整的 DTB 解析和硬件发现

### 🔍 调试与诊断工具

#### 函数调用栈追踪
- **多架构栈回溯**：支持 x86_64 (rbp)、RISC-V (fp)、AArch64 (x29) 帧指针追踪
- **符号解析**：与 ELF 符号表结合，提供函数名和地址映射
- **安全边界检查**：限制在内核代码段范围内追踪

#### 内核日志系统 (klog)
- **多级别日志**：Debug、Info、Warn、Error 分级输出
- **ANSI 色彩支持**：终端彩色输出增强可读性
- **线程安全**：基于 spinlock 的并发安全日志
- **源码定位**：集成 `__func__`、`__LINE__` 自动定位

#### 异常处理
- **C++ 异常捕获**：基础的 throw/catch 机制
- **系统停机保护**：异常发生时安全停机，防止系统损坏

### 🚀 并发与同步

#### SMP (对称多处理器) 支持
- **多核启动**：支持主核引导，从核 SMP 初始化
- **Per-CPU 数据结构**：每核独立的数据存储，最大支持 4 核心
- **中断管理**：嵌套中断深度跟踪和管理

#### 同步原语
- **自旋锁 (SpinLock)**：适用于短临界区的无锁等待同步
- **原子操作**：基于 C++ std::atomic 的原子变量支持
- **锁调试**：集成锁名称和状态跟踪

### 🔌 硬件抽象与驱动

#### 串口驱动
- **NS16550A**：标准 UART 控制器，广泛用于 x86 和 RISC-V 平台
- **PL011**：ARM 平台标准串口控制器
- **统一接口**：抽象化串口操作，支持配置波特率、数据位等

#### 系统信息获取
- **设备树解析 (DTB)**：完整的设备树二进制解析器，支持硬件自发现
- **ELF 解析器**：内核自身 ELF 格式解析，支持符号表和节信息提取
- **基础信息收集**：内存布局、CPU 数量、设备地址等系统参数

#### 设计模式支持
- **单例模式**：线程安全的单例模板，用于全局资源管理
- **RAII**：资源获取即初始化，确保资源正确释放

### 🧪 质量保证

#### 测试框架
- **多层次测试**：单元测试、集成测试、系统测试三级测试体系
- **Google Test 集成**：使用 gtest 框架进行 C++ 测试
- **覆盖率统计**：测试覆盖率分析和报告

#### 代码质量工具
- **静态分析**：集成 cppcheck、clang-tidy 提前发现潜在问题
- **动态检测**：sanitize 工具支持运行时错误检测
- **代码格式化**：遵循 Google C++ 代码风格，自动格式化

#### 文档系统
- **API 文档**：基于 doxygen 的自动 API 文档生成
- **自动部署**：GitHub Actions 自动部署文档到 GitHub Pages

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

### 代码风格
- **代码风格**：Google C++ 风格指南
- **格式化工具**：已配置 `.clang-format`
- **命名规范**：遵循 [Google 开源项目风格指南](https://zh-google-styleguide.readthedocs.io/en/latest/google-cpp-styleguide/contents.html)

### 文档部署
GitHub Actions 会自动将文档部署到 https://simple-xx.github.io/SimpleKernel/ （仅限 main 分支）
