[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-interrupt](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/interrupt)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

interrupt branch

> 🚀 **Current Branch Status**: boot - Build system setup, multi-architecture support, automated testing and documentation deployment completed

## 📖 Table of Contents

- [✨ Project Overview](#-project-overview)
- [🏗️ Supported Architectures](#️-supported-architectures)
- [🚀 Quick Start](#-quick-start)
- [💻 Core Features](#-core-features)
- [🎯 System Architecture](#-system-architecture)
- [📦 Third-party Dependencies](#-third-party-dependencies)
- [📝 Development Guide](#-development-guide)
- [🤝 Contributing](#-contributing)
- [📄 License](#-license)

## ✨ Project Overview

SimpleKernel is a modern operating system kernel designed for education and research, written in C++ with emphasis on multi-architecture support and modular design. The project aims to provide a feature-complete, clean-code practice platform for OS development learners and researchers.

### 🌟 Core Highlights

- **🔧 Modern C++17 Kernel Implementation** - Leveraging modern C++ features for type safety and high performance
- **🌐 True Multi-Architecture Support** - Native support for x86_64, RISC-V 64 and AArch64 mainstream architectures
- **🏗️ Engineering Build System** - CMake-based modern build with cross-compilation and multi-platform development support
- **🐳 Containerized Development Environment** - Pre-configured Docker environment for one-click development startup
- **🧪 Complete Testing Framework** - Three-tier testing architecture ensuring code quality: unit tests, integration tests, system tests
- **📚 Automated Documentation** - Doxygen-based API documentation auto-generation and deployment
- **🔍 Powerful Debugging Support** - Integrated stack backtrace, symbol resolution and multi-level logging system


## 🚀 Quick Start

### 📋 System Requirements

- **Operating System**: Linux (Ubuntu 20.04+ recommended) or macOS
- **Container Engine**: Docker 20.10+
- **Toolchain**: Included in Docker image (GCC cross-compilers, CMake, QEMU, etc.)

### 🛠️ Environment Setup

**Option 1: Using Docker (Recommended)**

```shell
# 1. Clone the project
git clone https://github.com/simple-xx/SimpleKernel.git
cd SimpleKernel
git submodule update --init --recursive

# 2. Start development environment
docker pull ptrnull233/simple_kernel:latest
docker run --name SimpleKernel-dev -itd -p 233:22 \
  -v $(pwd):/root/SimpleKernel ptrnull233/simple_kernel:latest

# 3. Enter development container
docker exec -it SimpleKernel-dev /bin/zsh
```

**Option 2: Local Environment**

Refer to [Toolchain Documentation](./doc/0_工具链.md) for local development environment setup.

### ⚡ Build and Run

```shell
cd SimpleKernel

# Select target architecture (RISC-V 64 example)
cmake --preset build_riscv64
cd build_riscv64

# Build kernel
make kernel

# Run in QEMU emulator
make run
```

**Supported Architecture Presets:**
- `build_riscv64` - RISC-V 64-bit architecture
- `build_aarch64` - ARM 64-bit architecture
- `build_x86_64` - x86 64-bit architecture

### 🎯 VS Code Integrated Development

The project has configured complete VS Code development environment:

```shell
# Open project in VS Code
code ./SimpleKernel
```

- **One-click Build**: Use `Ctrl+Shift+P` → `Tasks: Run Task` → Select corresponding architecture
- **Debug Support**: Configured GDB debugging environment with source-level debugging support
- **Code Completion**: Integrated C++ IntelliSense and syntax highlighting

## 🎯 System Architecture

- riscv64

    1. CSR register abstraction
    2. Register status printing
    3. Direct-based interrupt handling
    4. Interrupt registration functions
    5. Timer interrupts

- aarch64

    1. Interrupt registration functions
    2. Timer interrupts
    3. UART interrupts
    4. GICv3 driver

- x86_64

    1. CPU abstraction
    2. 8259A PIC controller abstraction
    3. 8253/8254 timer controller abstraction
    4. GDT initialization
    5. Interrupt handling flow
    6. Interrupt registration functions
    7. Timer interrupts

- TODO

    riscv64 PLIC

    x86_64 APIC

### 📋 Commit Message Standards
```
<type>(<scope>): <subject>

  - [x] [BUILD] CMake-based build system

  - [x] [BUILD] GDB remote debugging

  - [x] [BUILD] Third-party resource integration

  - [x] [COMMON] C++ global object construction

  - [x] [COMMON] C++ static local object construction

  - [x] [COMMON] C stack protection support

  - [x] [COMMON] printf support

  - [x] [COMMON] Simple C++ exception support

  - [x] [COMMON] Colored string output

  - [x] [x86_64] gnuefi-based bootloader

  - [x] [x86_64] Serial-based basic output

  - [x] [x86_64] Physical memory information detection

  - [x] [x86_64] Display buffer detection

  - [x] [x86_64] Call stack backtrace

  - [x] [riscv64] GP register initialization

  - [x] [riscv64] OpenSBI-based basic output

  - [x] [riscv64] Device tree hardware information parsing

  - [x] [riscv64] ns16550a serial driver

  - [x] [riscv64] Call stack backtrace (address printing only)

  - [ ] [aarch64] gnuefi-based bootloader (debugging)

## Used Third-party Resources

**Type Descriptions:**
- `feat`: New features
- `fix`: Bug fixes
- `docs`: Documentation updates
- `style`: Code formatting adjustments
- `refactor`: Code refactoring
- `test`: Test-related
- `chore`: Build tools or auxiliary tool changes

### 📚 Automatic Documentation Deployment
- **Main Branch Deployment** - GitHub Actions automatically deploys main branch documentation to [GitHub Pages](https://simple-xx.github.io/SimpleKernel/)
- **API Documentation** - Complete API reference documentation generated by Doxygen
- **Development Documentation** - Architecture design, development guides and best practices

## 🤝 Contributing

We welcome all forms of contributions! Whether code, documentation, testing or issue reports, all are important forces driving project development.

### 🎯 How to Contribute

**🐛 Report Issues**
- Use [GitHub Issues](https://github.com/Simple-XX/SimpleKernel/issues) to report bugs
- Describe problem reproduction steps, environment information and expected behavior in detail
- Attach relevant logs and error information

**💡 Feature Suggestions**
- Propose new feature suggestions through Issues
- Describe feature purpose, implementation ideas and expected effects
- Discuss technical feasibility and architectural impact

**🔧 Code Contributions**
1. Fork this repository to personal account
2. Create feature branch: `git checkout -b feature/amazing-feature`
3. Follow code standards for development
4. Add necessary test cases
5. Commit changes: `git commit -m 'feat: add amazing feature'`
6. Push branch: `git push origin feature/amazing-feature`
7. Create Pull Request

### 📋 Contributor Agreement
- Ensure code quality and test coverage
- Respect existing architecture and design patterns
- Actively participate in code review and discussions

## 📄 License

This project adopts multiple licenses:

Code Style: Google, specified by .clang-format

Naming Convention: [Google Open Source Project Style Guide](https://google.github.io/styleguide/cppguide.html)
