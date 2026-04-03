# Copyright The SimpleKernel Contributors

FROM ubuntu:latest

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get upgrade -y && \
    apt-get install --no-install-recommends --fix-missing -y \
    # 基础
    ca-certificates \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    # Shell 与开发体验
    zsh \
    vim \
    less \
    tree \
    htop \
    jq \
    ripgrep \
    fd-find \
    bat \
    unzip \
    sudo \
    locales \
    # QEMU（系统测试 + 运行）
    qemu-system-arm \
    qemu-system-misc \
    # 调试
    gdb-multiarch \
    # 设备树编译器
    device-tree-compiler \
    # U-Boot 镜像工具（mkimage）
    u-boot-tools \
    # FAT 文件系统工具（mkfs.fat，用于 rootfs 镜像）
    dosfstools \
    # RISC-V 交叉编译工具链（OpenSBI + U-Boot 固件构建）
    gcc-riscv64-linux-gnu \
    binutils-riscv64-linux-gnu \
    # AArch64 交叉编译工具链（ATF + U-Boot 固件构建）
    gcc-aarch64-linux-gnu \
    binutils-aarch64-linux-gnu \
    # ARM 32-bit 交叉编译工具链（OP-TEE 32-bit TA 构建）
    gcc-arm-linux-gnueabihf \
    binutils-arm-linux-gnueabihf \
    # Python（固件构建脚本依赖）
    python3 \
    python3-pip \
    python-is-python3 \
    # 文档生成
    graphviz \
    # Node.js（Mermaid CLI 图表渲染依赖）
    nodejs \
    npm \
    && \
    apt-get autoremove -y && apt-get clean -y && \
    rm -rf /var/lib/apt/lists/*

# 设置 locale（避免工具输出乱码）
RUN sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && locale-gen
ENV LANG=en_US.UTF-8

# GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
    apt-get update && apt-get install -y gh && \
    rm -rf /var/lib/apt/lists/*

# Mermaid CLI（Markdown 中的 Mermaid 图表 → PNG/SVG）
RUN npm install -g @mermaid-js/mermaid-cli

# Rust 工具链（具体版本由 rust-toolchain.toml 控制）
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain none && \
    rustup show

# Cargo 审计与文档工具
RUN cargo install --locked cargo-deny && \
    cargo install --locked mdbook

# 默认 shell 设为 zsh
ENV SHELL=/usr/bin/zsh
