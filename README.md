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

## 关键词

- kernel, own kernel
- x86_64/ riscv64/ aarch64
- osdev
- c++ bare metal
- u-boot, opensbi
- linux

## 快速开始

### 使用构建好的 Docker

```shell
# 拉取代码
git pull https://github.com/MRNIU/SimpleKernel.git
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

提供了用于运行、调试的  vscode 相关配置，可以直接使用 vscode 运行内核或进行调试。


## 执行流

![common_bootflow](https://www.planttext.com?text=dLDRIyCm57xU-HM7-2WROsCl1MLkYAWR38TF8YLjWmis5xDKEd-zITQkhHqezg4b9zyjvwJplQN65Y87iDpc39TA22LnePJ5BViec4mPx1ZDc44o6Kzcena1e8LLiX09Cm29Ad5gChnOyRUTlJFi0DffyfHhAYqcJYbNWQ-CVq_m1GPNmM0LwZ0OkWS6X3mFVPaGU0KcCqSj0J4Oa2qNEcUFp4YMayfhaHUivrMvJCV1nbT6syOXJcg33Z5qeSiKbCjHgdMB6r1ziWDnoN_Gz-znpfEqBBiQIwtl7ROlukr-2-0hVIOrwQxlxwjnN-B6bSy7s0iT_pL4xC3d5VuLPhlUT6OEhJipl3vEDGgN9Upusd5W4JuKGiDnuQhr92Bqifpc_ClTwCjBV2gavO910mr7ZN3jFxVIcaEpTUf4X2vPjGiqjVoJMXQOpQe60mIAevzQ4A4_O8W29yrAlmNo7WqGsgWwnK6ck55SMiXODqThtIIPkqQwN_eR)

## 新增特性

本分支是 SImpleKernel 的首个分支。在本分支中，完成了构建系统的基础搭建、基本的文档部署与自动化测试，当然还有最重要的，有基于 uefi 的 x86_64 内核与由 opensbi 启动的 riscv64 内核，可以在 qemu 上运行，并实现了简单的屏幕输出。

||x86_64|riscv64|aarch64|
| :-----------------------: | :-------------------------------: | :---------------------------------------------: | :-------------------: |
|引导|u-boot|u-boot+opensbi|u-boot+atf+optee|
|基本输出|通过 serial 实现|通过 opensbi 提供的 ecall 实现|通过 serial 实现|
|硬件资源探测|由 u-boot 传递|由 u-boot 传递的 dtb|由 u-boot+atf 传递的 dtb|

- 构建系统

  参考 [MRNIU/cmake-kernel](https://github.com/MRNIU/cmake-kernel) 的构建系统，详细解释见 [doc/build_system.md](./doc/build_system.md)

- libc 支持

  |     函数/变量名      |                       用途                       |      |
  | :------------------: | :----------------------------------------------: | :--: |
  | `__stack_chk_guard`  |                      栈保护                      |      |
  | `__stack_chk_fail()` |               栈保护检查失败后调用               |      |
  |      `memcpy()`      |                    复制内存块                    |      |
  |     `memmove()`      |          复制内存块，可以处理重叠区域。          |      |
  |      `memset()`      |                    设置内存块                    |      |
  |      `memcmp()`      |                    比较内存块                    |      |
  |      `memchr()`      |                在内存块中查找字符                |      |
  |      `strcpy()`      |                    复制字符串                    |      |
  |     `strncpy()`      |               复制指定长度的字符串               |      |
  |      `strcat()`      |                    连接字符串                    |      |
  |      `strcmp()`      |                    比较字符串                    |      |
  |     `strncmp()`      |               比较指定长度的字符串               |      |
  |      `strlen()`      |                  获取字符串长度                  |      |
  |     `strnlen()`      |                获取指定字符串长度                |      |
  |      `strchr()`      |           查找字符在字符串中的首次出现           |      |
  |     `strrchr()`      |         反向查找字符在字符串中的首次出现         |      |
  |     `strtoull()`     |      将字符串按指定进制转换为无符号长长整数      |      |
  |     `strtoul()`      |       将字符串按指定进制转换为无符号长整数       |      |
  |     `strtoll()`      |         将字符串按指定进制转换为长长整数         |      |
  |      `strtol()`      |          将字符串按指定进制转换为长整数          |      |
  |      `atoll()`       |              将字符串转换为长长整数              |      |
  |       `atol()`       |               将字符串转换为长整数               |      |
  |       `atoi()`       |                将字符串转换为整数                |      |
  |     `isalnum()`      |             检查字符是否为字母或数字             |      |
  |     `isalpha()`      |                检查字符是否为字母                |      |
  |     `isblank()`      |      检查字符是否为空白字符（空格或制表符）      |      |
  |     `iscntrl()`      |              检查字符是否为控制字符              |      |
  |     `isdigit()`      |         检查字符是否为十进制数字（0-9）          |      |
  |     `isgraph()`      |      检查字符是否为可打印字符（不包括空格）      |      |
  |     `islower()`      |              检查字符是否为小写字母              |      |
  |     `isprint()`      |       检查字符是否为可打印字符（包括空格）       |      |
  |     `ispunct()`      |              检查字符是否为标点符号              |      |
  |     `isspace()`      | 检查字符是否为空白字符（空格、制表符、换行符等） |      |
  |     `isupper()`      |              检查字符是否为大写字母              |      |
  |     `isxdigit()`     |   检查字符是否为十六进制数字（0-9、a-f、A-F）    |      |
  |     `tolower()`      |                 将字符转换为小写                 |      |
  |     `toupper()`      |                 将字符转换为大写                 |      |

- libc++ 支持

    |       函数/变量名       |                 用途                 |      |
    | :---------------------: | :----------------------------------: | :--: |
    |    `__cxa_atexit()`     |             注册析构函数             |      |
    |   `__cxa_finalize()`    |             调用析构函数             |      |
    | `__cxa_guard_acquire()` |         静态局部变量初始化锁         |      |
    | `__cxa_guard_release()` |        静态局部变量初始化完成        |      |
    |  `__cxa_guard_abort()`  |        静态局部变量初始化出错        |      |
    |    `__cxa_rethrow()`    |         用于简单处理 `throw`         |      |
    |    `operator new()`     | 运算符重载，空实现，用于全局对象支持 |      |
    |   `operator new[]()`    | 运算符重载，空实现，用于全局对象支持 |      |
    |    `operator new()`     | 运算符重载，空实现，用于全局对象支持 |      |
    |   `operator new[]()`    | 运算符重载，空实现，用于全局对象支持 |      |
    |   `operator delete()`   | 运算符重载，空实现，用于全局对象支持 |      |
    |   `operator delete()`   | 运算符重载，空实现，用于全局对象支持 |      |
    |  `operator delete[]()`  | 运算符重载，空实现，用于全局对象支持 |      |
    |  `operator delete[]()`  | 运算符重载，空实现，用于全局对象支持 |      |

- 打印函数调用栈

  逐层回溯帧指针后与 elf 信息进行对比，实现对函数调用栈的打印

- 基础 c++ 异常 支持

  通过 throw 抛出异常后停机，没有上下文相关的处理

- klog 内核日志模块

  基于 ANSI 转义码，在支持 ANSI 转义码的终端中可以显示有颜色的字符串

- 基于 u-boot+opensbi 引导的 riscv64 内核

  1. 由 opensbi 进行初始化，直接跳转到内核地址，进入内核逻辑时为 S 态
  2. gp 寄存器的初始化
  3. 基于 opensbi 的 printf
  4. 使用 FIT 打包的内核

- 基于 u-boot 引导的 amd64 内核

  1. 由 u-boot 进行初始化，进入内核时为 64 位状态
  2. 使用 FIT 打包的内核

- 基于 u-boot+arm-trusted-firmware+optee 的 aarch64 内核

  1. 使用 FIT 打包的内核
  2. 由 u-boot 进行初始化，进入内核时为 64 位状态
  3. 使用 atf 框架

- SMP 支持

  多核支持

- spinlock

  适用于多核抢占的自旋锁，主要用于 klog 模块

- dtb 解析

- elf 解析

- ns16550a 串口驱动

- pl011 串口驱动

- 基于 doxygen 的文档生成与自动部署

  github action 会将文档部署到 https://simple-xx.github.io/SimpleKernel/ (仅 main 分支)

- 基于 git submodule 的第三方资源管理

  使用 git submodule 集成第三方资源

- 测试

    支持 单元测试、集成测试、系统测试，引入 gtest 作为测试框架，同时统计了测试覆盖率

- 代码分析

    引入 cppcheck、clang-tidy、sanitize 工具提前发现错误

- 代码格式化

    使用 google 风格

- docker

    支持使用 docker 构建，详细使用方法见 [doc/docker.md](./doc/docker.md)

## 已支持特性

见 新增特性

## 使用的第三方资源

[google/googletest](https://github.com/google/googletest.git)

[eyalroz/printf](https://github.com/eyalroz/printf.git)

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

命名规范：[Gooele 开源项目风格指南](https://zh-google-styleguide.readthedocs.io/en/latest/google-cpp-styleguide/contents.html)
