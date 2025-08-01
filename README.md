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
