# Copyright The SimpleKernel Contributors

FROM ubuntu:26.04

ARG RUST_TOOLCHAIN=nightly-2026-09-22
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8
SHELL ["/bin/bash", "-euxo", "pipefail", "-c"]

# 仅提供基础编译环境；QEMU、固件、调试和审计工具按任务安装。
RUN apt-get update && apt-get install --no-install-recommends -y \
    ca-certificates curl git build-essential sudo && \
    rm -rf /var/lib/apt/lists/*

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

# Dev Container 使用普通用户；CI container job 默认以 root 运行。
RUN useradd -m -s /bin/bash dev && \
    echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev && \
    chmod 0440 /etc/sudoers.d/dev && \
    mkdir -p /workspace && chown dev:dev /workspace

WORKDIR /workspace
