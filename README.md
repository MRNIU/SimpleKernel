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

## 关键词

- kernel, own kernel
- x86_64, riscv64, aarch64
- osdev
- c++ bare metal
- u-boot, opensbi
- linux

## 快速开始

### 使用构建好的 Docker

```shell
# 拉取代码
git clone https://github.com/MRNIU/SimpleKernel.git
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
# build_riscv64/build_aarch64/build_x86_64/
cmake --preset build_riscv64
cd build_riscv64
# 编译内核
make kernel
# 在 qemu 中运行
make run
```

### 使用 vscode

提供了用于运行、调试的 vscode 相关配置，可以直接使用 vscode 运行内核或进行调试。


## 执行流

[common_bootflow](https://www.plantuml.com/plantuml/png/dL9TIyCm57tU_HKXFewDiR6NWJ8tHDGDXiKdaPAs5nVCHymIsVpr9d6bgnqexg6ZvvwFqzpCTuvPvwK0nvr0ijHIQaKMMZkIuRj7LI9iaLLe2HsFnjFXb08mxxJoia0BKEWzcTYANApuwzRTMZo02PQyv8OfHuhW97JIQnkVO_8ClSiKi4euz0RX1prAdmOHfXHU05L5WZCGaW9engKH-81MeQ37h8NmsCawfan6AIOYmwn9o8iwe2LCXz1MIiRLi3JcH9jONN4WSSL_o7TlkU15kT-tFPR6t0LkroJ6_LOW8bqbi-Mscn_Hl6jn7U3p1NRIv7yjaGVoUOT_bSdMczREuUJE3Aw-jpfBboLD0fOM5i5xBmsabu3McmXujELCy4yaotwVF7hbk4HegB5DuAtZturozj2CwfC8uz3iE0LMElx172PbyrQJ0U8po9jzp4Zym5G5Qbhjtv1IHaEiRLej3gea6ysLWmhRFIhiDfcZghmKNm00)

## 新增特性

本分支是 SimpleKernel 的首个分支。在本分支中，完成了构建系统的基础搭建、基本的文档部署与自动化测试，当然还有最重要的，有基于 u-boot 引导的 x86_64 内核与由 opensbi 启动的 riscv64 内核，可以在 qemu 上运行，并实现了简单的屏幕输出。

- riscv64

    1. 对 CSR 寄存器的抽象
    2. 寄存器状态打印
    3. 基于 Direct 的中断处理
    4. 中断注册函数
    5. 时钟中断

- aarch64

    1. 中断注册函数
    2. 时钟中断
    3. uart 中断
    4. gicv3 驱动

- X86_64

    1. cpu 抽象
    2. 8259A pic 控制器抽象
    3. 8253/8254 timer 控制器抽象
    4. gdt 初始化
    5. 中断处理流程
    6. 中断注册函数
    7. 时钟中断

- TODO

    riscv64 PLIC

    x86_64 APIC

## 已支持的特性

  - [x] [BUILD] 使用 CMake 的构建系统

  - [x] [BUILD] 使用 gdb remote 调试

  - [x] [BUILD] 第三方资源集成

  - [x] [COMMON] C++ 全局对象的构造

  - [x] [COMMON] C++ 静态局部对象构造

  - [x] [COMMON] C 栈保护支持

  - [x] [COMMON] printf 支持

  - [x] [COMMON] 简单的 C++ 异常支持

  - [x] [COMMON] 带颜色的字符串输出

  - [x] [x86_64] 基于 gnuefi 的 bootloader

  - [x] [x86_64] 基于 serial 的基本输出

  - [x] [x86_64] 物理内存信息探测

  - [x] [x86_64] 显示缓冲区探测

  - [x] [x86_64] 调用栈回溯

  - [x] [riscv64] gp 寄存器的初始化

  - [x] [riscv64] 基于 opensbi 的基本输出

  - [x] [riscv64] device tree 硬件信息解析

  - [x] [riscv64] ns16550a 串口驱动

  - [x] [riscv64] 调用栈回溯(仅打印地址)

  - [ ] [aarch64] 基于 gnuefi 的 bootloader(调试中)

## 使用的第三方资源

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

## 开发指南

代码风格：Google，已由 .clang-format 指定

命名规范：[Google 开源项目风格指南](https://zh-google-styleguide.readthedocs.io/en/latest/google-cpp-styleguide/contents.html)
