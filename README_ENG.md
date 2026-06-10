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

Use the project Dev Container instead of installing the Rust nightly toolchain,
cross-compilers, QEMU, or firmware build dependencies on the host.

```bash
devcontainer up --workspace-folder .
docker exec -w /workspace simplekernel-devcontainer cargo xtask build --arch riscv64
docker exec -w /workspace simplekernel-devcontainer cargo xtask run --arch riscv64 --timeout 30
docker exec -w /workspace simplekernel-devcontainer cargo xtask test --arch riscv64 --timeout 30
docker exec -w /workspace simplekernel-devcontainer cargo fmt --all -- --check
```

QEMU run and test commands default to `--timeout 30`. Use a larger timeout only
when the scenario explicitly needs it.
