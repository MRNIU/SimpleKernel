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

- **🔧 Modern CMake Build** - Cross-platform build system with multi-architecture presets and cross-compilation support
- **📋 Dependency Management** - Unified third-party component management via Git Submodule with version control
- **🐳 Containerized Development** - Pre-configured Docker environment with complete toolchain and dependencies
- **📖 Detailed Documentation** - Toolchain installation guide: [doc/0_工具链.md](./doc/0_工具链.md), Docker usage: [doc/docker.md](./doc/docker.md)

### 📚 Runtime Support Libraries

#### 🛠️ C Standard Library Kernel Implementation
Provides essential libc function implementations for kernel environment:

- **Memory Operations** - Efficient memory operations like `memcpy()`, `memmove()`, `memset()`, `memcmp()`
- **String Processing** - Complete string functions including `strcpy()`, `strcmp()`, `strlen()` series
- **Type Conversion** - String to numeric conversions like `atoi()`, `strtol()`
- **Character Classification** - Character checking functions like `isalnum()`, `isdigit()`
- **Stack Protection Mechanism** - Stack overflow detection and protection via `__stack_chk_guard`

#### ⚡ C++ Runtime Environment
Complete kernel C++ runtime support:

- **🔄 Object Lifecycle** - Global object construction/destruction management via `__cxa_atexit()`, `__cxa_finalize()`
- **🔒 Thread-Safe Initialization** - Static local variable thread-safe mechanism via `__cxa_guard_*`
- **💾 Memory Management** - `operator new/delete` overloads adapted to kernel memory management
- **⚠️ Exception Handling** - Basic exception catching and `throw/catch` mechanism
- **📤 I/O Stream Support** - Custom iostream implementation supporting formatted output

### 🖥️ Multi-Architecture Hardware Abstraction

#### 🔧 RISC-V 64-bit Architecture
- **Boot Chain Integration** - U-Boot → OpenSBI → Kernel, S-mode privilege level execution
- **Register Initialization** - GP register proper configuration following RISC-V ABI standard
- **SBI System Calls** - System services based on OpenSBI ecall interface
- **FIT Image Support** - Flattened Image Tree format kernel packaging

#### 🖥️ x86_64 Architecture
- **Direct Boot** - U-Boot 64-bit long mode direct startup without additional firmware
- **NS16550A Serial** - Standard UART controller driver with broad compatibility
- **Unified FIT Packaging** - Same image format as other architectures

#### 📱 AArch64 Architecture
- **Complete Secure Boot** - U-Boot → ARM Trusted Firmware → OP-TEE → Kernel
- **TrustZone Integration** - ATF secure world framework supporting security services
- **PL011 UART** - ARM standard serial controller
- **DTB Hardware Discovery** - Complete device tree parsing and automatic hardware discovery

### 🔍 Advanced Debugging and Diagnostics

#### 📋 Function Call Stack Tracing
- **Multi-Architecture Stack Backtracing** - Support for x86_64 (RBP), RISC-V (FP), AArch64 (X29) frame pointer chains
- **Symbol Resolution Integration** - Precise function name and address mapping combined with ELF symbol tables
- **Safe Boundary Checking** - Limited to kernel code segment range preventing stack trace overflow

#### 📝 Kernel Logging System (klog)
- **🌈 Multi-Level Color Logging** - Debug/Info/Warn/Error four-level logging with ANSI color output
- **📟 Early Console** - Support for debug output during early kernel boot, covering static object initialization phase
- **🔒 Concurrency Safety** - Thread-safe logging based on SpinLock
- **📍 Precise Source Location** - Automatic recording of `__func__`, `__LINE__` debug information
- **⚡ High-Performance Output** - Optimized formatted output with minimal performance impact

#### ⚠️ Exception Handling and Protection
- **C++ Exception Mechanism** - Complete throw/catch exception catching and handling
- **System Safe Shutdown** - Safe system shutdown when exceptions occur, preventing data corruption
- **Stack Overflow Detection** - Compile-time stack protection, runtime stack overflow attack detection

### 🚀 Concurrency and Synchronization Support

#### 🔄 SMP (Symmetric Multi-Processing) Architecture
- **🚀 Multi-Core Boot Management** - Boot core sequence, secondary core SMP automatic initialization
- **💾 Per-CPU Data Structures** - Independent data storage per core, supporting up to 4 cores
- **⚡ Interrupt Nesting Management** - Interrupt depth tracking and nested interrupt handling

#### 🔐 Modern Synchronization Primitives
- **🌀 SpinLock** - Lock-free waiting synchronization mechanism for short critical sections
- **⚛️ Atomic Operation Support** - Atomic variables and memory model based on C++ `std::atomic`
- **🔍 Lock Debugging Features** - Integrated lock name tracking and deadlock detection mechanism
- **📊 Performance Monitoring** - Lock contention statistics and performance analysis support

### 🔌 Hardware Abstraction Layer and Drivers

#### 📡 Unified Serial Driver Interface
- **NS16550A Controller** - Standard UART for x86 and RISC-V platforms with interrupt and DMA support
- **PL011 Controller** - ARM platform dedicated serial with complete FIFO and flow control support
- **Unified Abstract Interface** - Cross-architecture serial operation API supporting baud rate, data bits, parity configuration
- **Advanced Features** - Asynchronous I/O, buffer management and error recovery mechanism

#### 🔍 System Information and Hardware Discovery
- **📋 Device Tree Parsing (DTB)** - Complete device tree binary parser supporting automatic hardware discovery and configuration
- **🔧 ELF Parser** - Kernel self ELF format parsing, symbol table and section information extraction
- **📊 System Parameter Collection** - Key system information like memory layout, CPU count, device address mapping
- **🌐 Cross-Architecture Compatibility** - Unified hardware information interface hiding architectural differences

#### 🏗️ Design Pattern and Architecture Support
- **🔒 Thread-Safe Singleton** - Template singleton implementation for global resource management
- **💎 RAII Resource Management** - Resource Acquisition Is Initialization ensuring proper resource release and exception safety
- **🎯 Factory Pattern** - Dynamic creation and management of drivers and services
- **📦 Modular Architecture** - Loosely coupled component design supporting dynamic loading and unloading

### 🧪 Enterprise-Grade Quality Assurance

#### 🔬 Three-Tier Testing Framework
- **🎯 Unit Testing** - Module-level functional verification based on Google Test
- **🔗 Integration Testing** - Cross-module interface and data flow testing
- **🌐 System Testing** - End-to-end complete system functionality testing
- **📊 Coverage Analysis** - Code coverage statistics and quality reports targeting >90%

#### 🛡️ Static and Dynamic Analysis
- **🔍 Static Analysis Tools** - Integrated Cppcheck, Clang-Tidy for compile-time issue detection
- **🧨 Dynamic Error Detection** - AddressSanitizer, UBSan runtime error capturing
- **💅 Code Formatting** - Clang-Format automatic formatting following Google C++ style
- **📏 Code Quality Gates** - CI/CD automated quality checks preventing merge without passing

#### 📚 Automated Documentation System
- **📖 API Documentation Generation** - Source code comment automatic documentation generation based on Doxygen
- **🚀 Automatic Deployment** - GitHub Actions automatic build and deployment to GitHub Pages
- **🔄 Real-time Updates** - Code changes automatically trigger documentation updates
- **🌍 Multi-language Support** - Chinese-English bilingual documentation maintenance

## 📦 Third-party Dependencies

- [google/googletest](https://github.com/google/googletest.git) - Testing framework
- [charlesnicholson/nanoprintf](https://github.com/charlesnicholson/nanoprintf.git) - printf implementation
- [MRNIU/cpu_io](https://github.com/MRNIU/cpu_io.git) - CPU I/O operations
- [riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git) - RISC-V SBI implementation
- [MRNIU/opensbi_interface](https://github.com/MRNIU/opensbi_interface.git) - OpenSBI interface
- [u-boot/u-boot](https://github.com/u-boot/u-boot.git) - Universal bootloader
- [OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git) - OP-TEE operating system
- [OP-TEE/optee_client](https://github.com/OP-TEE/optee_client.git) - OP-TEE client
- [ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git) - ARM Trusted Firmware
- [dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git) - Device Tree Compiler

## 📝 Development Guide

### 🎨 Code Style Standards
- **Coding Standards** - Strictly follow [Google C++ Style Guide](https://google.github.io/styleguide/cppguide.html)
- **Automatic Formatting** - Pre-configured `.clang-format`, use `clang-format` for automatic formatting
- **Naming Conventions** - Class names use PascalCase, functions and variables use snake_case
- **Comment Standards** - Use Doxygen-style comments supporting automatic documentation generation

### 🚀 Development Workflow
1. **Fork Project** - Create personal branch from main repository
2. **Local Development** - Use Docker environment for development and testing
3. **Quality Checks** - Run static analysis and test suites
4. **Submit PR** - Follow commit message standards with detailed change descriptions

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
