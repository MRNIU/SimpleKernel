<!-- Copyright The SimpleKernel Contributors -->

[English](./README_ENG.md) | [中文](./README.md)

# SimpleKernel

SimpleKernel is an interface-driven OS kernel for AI-assisted learning. This
file is a concise English entry point; the maintained project details live in
the Chinese README and repository documentation. The current codebase is written
in Rust (`no_std`, `no_main`, nightly toolchain) and targets RISC-V 64 and
AArch64 through the repository `cargo xtask` workflow.

The Chinese README is the maintained project entry point:

- [中文 README](./README.md)
- [Dev Container setup](./docs/docker.md)
- [Contribution guide](./CONTRIBUTING.md)
- [Documentation index](./docs/AGENTS.md)

## Quick Start

Choose a local environment or the optional Dev Container / Docker environment.
Prepare the dependencies described in [CONTRIBUTING](CONTRIBUTING.md#环境与命令),
then run these commands at the repository root in your chosen environment.

```bash
cargo xtask build --arch riscv64
cargo xtask run --arch riscv64 --timeout 30
cargo xtask test --arch riscv64 --name frame-test/alloc --timeout 30
```

If using a container, check that it mounts this checkout. For command details,
QEMU timeouts and cleanup, see the [xtask guide](xtask/AGENTS.md); choose validation by change
scope in [CONTRIBUTING](CONTRIBUTING.md). The kernel uses a single address space (SAS);
restricting all application access to the syscall gateway remains a goal, not a completed guarantee.
