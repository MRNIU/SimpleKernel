[![codecov](https://codecov.io/gh/Simple-XX/SimpleKernel/graph/badge.svg?token=J7NKK3SBNJ)](https://codecov.io/gh/Simple-XX/SimpleKernel)
![workflow](https://github.com/Simple-XX/SimpleKernel/actions/workflows/workflow.yml/badge.svg)
![commit-activity](https://img.shields.io/github/commit-activity/t/Simple-XX/SimpleKernel)
![last-commit-boot](https://img.shields.io/github/last-commit/Simple-XX/SimpleKernel/boot)
![MIT License](https://img.shields.io/github/license/mashape/apistatus.svg)
[![LICENSE](https://img.shields.io/badge/license-Anti%20996-blue.svg)](https://github.com/996icu/996.ICU/blob/master/LICENSE)
[![996.icu](https://img.shields.io/badge/link-996.icu-red.svg)](https://996.icu)

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

boot branch

## Key Words

- kernel, own kernel
- x86_64/ riscv64/ aarch64
- osdev
- c++ bare metal
- u-boot, opensbi
- linux


## Quick Start

### Using Pre-built Docker

```shell
# Clone repository
git clone https://github.com/MRNIU/SimpleKernel.git
git submodule update --init --recursive
# Pull Docker Image
docker pull ptrnull233/simple_kernel:latest
# Run Docker
docker run --name SimpleKernel-container -itd -p 233:22 -v ./SimpleKernel:/root/ ptrnull233/simple_kernel:latest
# Enter Docker
docker exec -it SimpleKernel-container /bin/zsh
```

### Build and Run

```shell
cd SimpleKernel
# build_riscv64/build_aarch64/build_x86_64/
cmake --preset build_riscv64
cd build_riscv64
# Build kernel
make kernel
# Run in QEMU
make run
```

### Using VSCode

Pre-configured VSCode settings for running and debugging the kernel are provided.

## Execution Flow

[common_bootflow](https://www.plantuml.com/plantuml/png/dL9TIyCm57tU_HKXFewDiR6NWJ8tHDGDXiKdaPAs5nVCHymIsVpr9d6bgnqexg6ZvvwFqzpCTuvPvwK0nvr0ijHIQaKMMZkIuRj7LI9iaLLe2HsFnjFXb08mxxJoia0BKEWzcTYANApuwzRTMZo02PQyv8OfHuhW97JIQnkVO_8ClSiKi4euz0RX1prAdmOHfXHU05L5WZCGaW9engKH-81MeQ37h8NmsCawfan6AIOYmwn9o8iwe2LCXz1MIiRLi3JcH9jONN4WSSL_o7TlkU15kT-tFPR6t0LkroJ6_LOW8bqbi-Mscn_Hl6jn7U3p1NRIv7yjaGVoUOT_bSdMczREuUJE3Aw-jpfBboLD0fOM5i5xBmsabu3McmXujELCy4yaotwVF7hbk4HegB5DuAtZturozj2CwfC8uz3iE0LMElx172PbyrQJ0U8po9jzp4Zym5G5Qbhjtv1IHaEiRLej3gea6ysLWmhRFIhiDfcZghmKNm00)

## New Features

This branch is the first branch of SimpleKernel. In this branch, we have completed the foundation of the build system, basic documentation deployment, and automated testing. Most importantly, we have implemented a u-boot based x86_64 kernel and a RISC-V64 kernel started by OpenSBI, both of which can run on QEMU and achieve simple screen output.

- Build system

  References the [MRNIU/cmake-kernel](https://github.com/MRNIU/cmake-kernel) build system. For detailed explanation, see [doc/build_system.md](./doc/build_system.md)

- Stack Trace Printing

  Traverse frame pointers and cross-reference with ELF symbol tables to reconstruct function call stacks.

- Basic C++ Exception Support

  Implements throw to trigger exceptions followed by system termination (no context-aware exception handling).

- klog Kernel Logging Module

  Supports ANSI escape codes for colored text output in ANSI-compatible terminals.

- RISC-V64 (U-Boot + OpenSBI)

  1. Initialized by OpenSBI; kernel entry in Supervisor (S) mode.
  2. GP (Global Pointer) register initialization.
  3. printf implementation leveraging OpenSBI.
  4. FIT (Flattened Image Tree) image packaging.

- x86_64 (U-Boot)

  1. Initialized by U-Boot; kernel enters 64-bit mode directly.
  2. FIT image packaging.

- AArch64 (U-Boot + Arm Trusted Firmware + OP-TEE)

  1. FIT image packaging.
  2. Initialized by U-Boot in 64-bit mode.
  3. ATF (Arm Trusted Firmware) framework integration.

- SMP Support

  Multi-core CPU coordination and synchronization

- Spinlock

  Preemptive multi-core spinlock implementation (primarily for klog synchronization)

- Device Tree Blob (DTB) Parsing

  Hardware configuration decoding and interpretation

- ELF Parsing

  Executable and Linkable Format analysis

- NS16550A UART Driver

  Serial communication driver for x86/RISC-V platforms

- PL011 UART Driver

  Serial communication driver for ARM platforms

- Doxygen-based Documentation Generation and Automatic Deployment

  GitHub Actions automatically deploys documentation to https://simple-xx.github.io/SimpleKernel/ (main branch only)

- Third-party Resource Management based on Git Submodules

  Uses git submodule to integrate third-party resources

- Testing Framework

  Supports unit testing, integration testing, and system testing using gtest framework with test coverage statistics

- Code Analysis

  Integrates cppcheck, clang-tidy, and sanitize tools for early error detection

- Code Formatting

  Uses Google style formatting

- Docker Support

  Supports building with Docker, see [doc/docker.md](./doc/docker.md)

## Supported Features

See New Features

## Third-party Dependencies

[google/googletest](https://github.com/google/googletest.git)

[charlesnicholson/nanoprintf](https://github.com/charlesnicholson/nanoprintf.git)

[MRNIU/cpu_io](https://github.com/MRNIU/cpu_io.git)

[riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git)

[MRNIU/opensbi_interface](https://github.com/MRNIU/opensbi_interface.git)

[u-boot/u-boot](https://github.com/u-boot/u-boot.git)

[OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git)

[OP-TEE/optee_client](https://github.com/OP-TEE/optee_client.git)

[ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git)

[dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git)

## Development Guide

- **Code Style**: Google Style (enforced via .clang-format)
- **Naming Convention**: [Google Open Source Style Guide](https://google.github.io/styleguide/cppguide.html)
