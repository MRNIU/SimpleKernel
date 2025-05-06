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

![common_bootflow](https://www.planttext.com?text=dLDRIyCm57xU-HM7-2WROsCl1MLkYAWR38TF8YLjWmis5xDKEd-zITQkhHqezg4b9zyjvwJplQN65Y87iDpc39TA22LnePJ5BViec4mPx1ZDc44o6Kzcena1e8LLiX09Cm29Ad5gChnOyRUTlJFi0DffyfHhAYqcJYbNWQ-CVq_m1GPNmM0LwZ0OkWS6X3mFVPaGU0KcCqSj0J4Oa2qNEcUFp4YMayfhaHUivrMvJCV1nbT6syOXJcg33Z5qeSiKbCjHgdMB6r1ziWDnoN_Gz-znpfEqBBiQIwtl7ROlukr-2-0hVIOrwQxlxwjnN-B6bSy7s0iT_pL4xC3d5VuLPhlUT6OEhJipl3vEDGgN9Upusd5W4JuKGiDnuQhr92Bqifpc_ClTwCjBV2gavO910mr7ZN3jFxVIcaEpTUf4X2vPjGiqjVoJMXQOpQe60mIAevzQ4A4_O8W29yrAlmNo7WqGsgWwnK6ck55SMiXODqThtIIPkqQwN_eR)

## New Features

This branch is the first branch of SImpleKernel. In this branch, the foundation of the build system is completed, basic documentation deployment and automated testing, and of course the most important, there is a uefi based x86_64 kernel and riscv64 kernel started by opensbi, which can run on qemu, and achieve simple screen output.

- Build system

  Reference [MRNIU/cmake-kernel](https://github.com/MRNIU/cmake-kernel) build system, a detailed explanation see [doc/build_system.md](./doc/build_system.md)

- Stack Trace Printing

  Traverse frame pointers and cross-reference with ELF symbol tables to reconstruct function call stacks.

- Basic C++ Exception Support

  Implements throw to trigger exceptions followed by system termination (no context-aware exception handling).

- klog Kernel Logging Module

  Supports ANSI escape codes for colored text output in ANSI-compatible terminals.

- RISC-V64 (U-Boot + OpenSBI)

  - Initialized by OpenSBI; kernel entry in Supervisor (S) mode.

  - GP (Global Pointer) register initialization.

  - printf implementation leveraging OpenSBI.

  - FIT (Flattened Image Tree) image packaging.

- AMD64 (U-Boot)

  - Initialized by U-Boot; kernel enters 64-bit mode directly.

  - FIT image packaging.

- AArch64 (U-Boot + Arm Trusted Firmware [ATF] + OP-TEE)

  - FIT image packaging.

  - Initialized by U-Boot in 64-bit mode.

  - ATF (Arm Trusted Firmware) framework integration.

- SMP Support

  Multi-core CPU coordination

- Spinlock

  Preemptive multi-core spinlock implementation (primarily for klog synchronization).

- Device Tree Blob (DTB) Parsing

  Hardware configuration decoding.

- ELF Parsing

  Executable/linkable format analysis.

- NS16550A

  UART driver for x86/RISC-V platforms.

- PL011

  UART driver for ARM platforms.


- Doxygen-based document generation and automatic deployment

  Making the action will document deployment to https://simple-xx.github.io/SimpleKernel/ (the main branch only)

- Third-party resource management based on CPM

  Use CPM's functionality in '3rd.cmake' to automatically download and integrate third-party resources

- Test

    Support unit testing, integration testing, system testing, the introduction of gtest as a test framework, while the test coverage statistics

- Code analysis

    Introduce cppcheck, clang-tidy, and sanitize tools to detect errors in advance

- Code formatting

    Use the llvm style

- docker

    Supports building with docker, see [doc/docker.md](./doc/docker.md)

## Supported Features

See "New Features" section

## 3rd

[google/googletest](https://github.com/google/googletest.git)

[eyalroz/printf](https://github.com/eyalroz/printf.git)

[MRNIU/cpu_io](https://github.com/MRNIU/cpu_io.git)

[riscv-software-src/opensbi](https://github.com/riscv-software-src/opensbi.git)

[MRNIU/opensbi_interface](https://github.com/MRNIU/opensbi_interface.git)

[u-boot/u-boot](https://github.com/u-boot/u-boot.git)

[OP-TEE/optee_os](https://github.com/OP-TEE/optee_os.git)

[OP-TEE/optee_client](https://github.com/OP-TEE/optee_client.git)

[ARM-software/arm-trusted-firmware](https://github.com/ARM-software/arm-trusted-firmware.git)

[dtc/dtc](https://git.kernel.org/pub/scm/utils/dtc/dtc.git)https://github.com/google/googletest)


## Development Guide
- Code Style: Google Style (enforced via .clang-format)
- Naming Convention: [Google Open Source Style Guide](https://google.github.io/styleguide/cppguide.html)
