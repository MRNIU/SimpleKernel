# Copyright The SimpleKernel Contributors

FROM ubuntu:26.04

ARG RUST_TOOLCHAIN=nightly-2026-03-24

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-euxo", "pipefail", "-c"]

RUN apt-get update && apt-get full-upgrade -y && \
    apt-get install --no-install-recommends --fix-missing -y \
    # 基础
    ca-certificates \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    file \
    rsync \
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
    shellcheck \
    unzip \
    sudo \
    locales \
    # QEMU（系统测试 + 运行）
    qemu-system-arm \
    qemu-system-misc \
    qemu-system-riscv \
    # 调试
    gdb-multiarch \
    # 固件构建基础依赖
    bc \
    bison \
    flex \
    gawk \
    cpio \
    libgnutls28-dev \
    libncurses-dev \
    libssl-dev \
    openssl \
    swig \
    uuid-dev \
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
    python3-cryptography \
    python3-dev \
    python3-pyelftools \
    python3-pip \
    python3-setuptools \
    python3-venv \
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
RUN install -m 0755 -d /etc/apt/keyrings && \
    curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/etc/apt/keyrings/githubcli-archive-keyring.gpg && \
    chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | tee /etc/apt/sources.list.d/github-cli.list >/dev/null && \
    apt-get update && apt-get install -y gh && \
    rm -rf /var/lib/apt/lists/*

# pre-commit（仓库提交前检查）
RUN python3 -m venv /opt/pre-commit && \
    /opt/pre-commit/bin/pip install --no-cache-dir --upgrade pip pre-commit && \
    ln -s /opt/pre-commit/bin/pre-commit /usr/local/bin/pre-commit && \
    pre-commit --version

# Mermaid CLI（Markdown 中的 Mermaid 图表 → PNG/SVG）
RUN npm install -g npm@latest && \
    hash -r && \
    npm --version && \
    npm install -g @mermaid-js/mermaid-cli && \
    mmdc --version

# Rust 工具链（与 rust-toolchain.toml 保持一致）
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal \
        --default-toolchain "${RUST_TOOLCHAIN}" \
        --component rust-src \
        --component rustfmt \
        --component clippy \
        --component llvm-tools \
        --target riscv64gc-unknown-none-elf \
        --target aarch64-unknown-none && \
    chmod -R a+w "$RUSTUP_HOME" "$CARGO_HOME" && \
    rustup show && \
    rustc --version && \
    cargo --version

# Cargo 审计与文档工具
RUN cargo install --locked cargo-deny && \
    cargo install --locked mdbook && \
    chmod -R a+w "$CARGO_HOME"

# Dev Container 交互用户。构建阶段仍使用 root 安装工具，运行阶段使用 dev。
RUN useradd -m -s /usr/bin/zsh dev && \
    echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev && \
    chmod 0440 /etc/sudoers.d/dev && \
    mkdir -p /workspace && \
    chown dev:dev /workspace

# 默认 shell 设为 zsh
ENV SHELL=/usr/bin/zsh

WORKDIR /workspace
