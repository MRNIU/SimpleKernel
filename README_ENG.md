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

## Supported Features

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

Code Style: Google, specified by .clang-format

Naming Convention: [Google Open Source Project Style Guide](https://google.github.io/styleguide/cppguide.html)
