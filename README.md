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

本分支是 SimpleKernel 的首个分支。在本分支中，完成了构建系统的基础搭建、基本的文档部署与自动化测试，当然还有最重要的，有基于 u-boot 引导的 x86_64 内核与由 opensbi 启动的 riscv64 内核，可以在 qemu 上运行，并实现了简单的屏幕输出。

### 🔧 构建系统

见 [doc/0_工具链.md](./doc/0_工具链.md)

### 📚 标准库支持

#### libc 支持

提供了完整的 libc 函数支持，包括：

- **内存操作**：`memcpy()`, `memmove()`, `memset()`, `memcmp()`, `memchr()`
- **字符串操作**：`strcpy()`, `strncpy()`, `strcat()`, `strcmp()`, `strlen()` 等
- **字符串转换**：`atoi()`, `atol()`, `strtol()`, `strtoul()` 等
- **字符分类**：`isalnum()`, `isalpha()`, `isdigit()` 等
- **栈保护**：`__stack_chk_guard`, `__stack_chk_fail()`

#### libc++ 支持

提供了基础的 C++ 运行时支持：

- **对象构造/析构**：`__cxa_atexit()`, `__cxa_finalize()`
- **静态局部变量**：`__cxa_guard_acquire()`, `__cxa_guard_release()`
- **内存管理**：`operator new()`, `operator delete()` 系列
- **异常处理**：`__cxa_rethrow()` 简单异常处理

### 🖥️ 架构特定实现

#### RISC-V 64位支持
- 基于 u-boot+opensbi 引导
- S 态运行环境
- gp 寄存器初始化
- 基于 opensbi 的输出实现
- FIT 打包内核

#### x86_64 支持
- 基于 u-boot 引导
- 64位运行环境
- FIT 打包内核

#### AArch64 支持
- 基于 u-boot+arm-trusted-firmware+optee
- 64位运行环境
- ATF 框架集成
- FIT 打包内核

### 🔍 调试与诊断

- **函数调用栈打印**：逐层回溯帧指针后与 ELF 信息进行对比
- **基础 C++ 异常支持**：通过 throw 抛出异常后停机
- **klog 内核日志模块**：基于 ANSI 转义码的彩色输出

### 🚀 多核与同步

- **SMP 支持**：多核处理器支持
- **spinlock**：适用于多核抢占的自旋锁，主要用于 klog 模块

### 🔌 硬件驱动

- **串口驱动**：ns16550a 和 pl011 串口驱动
- **DTB 解析**：设备树解析支持
- **ELF 解析**：可执行文件格式解析

### 🧪 开发工具支持

- **测试框架**：支持单元测试、集成测试、系统测试，基于 gtest 框架
- **代码分析**：集成 cppcheck、clang-tidy、sanitize 工具
- **代码格式化**：使用 Google 代码风格
- **Docker 支持**：容器化开发环境，详见 [doc/docker.md](./doc/docker.md)
- **文档生成**：基于 doxygen 的自动文档生成与部署

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
