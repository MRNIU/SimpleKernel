# 参考文献

> 本文件收录 SimpleKernel 设计过程中参考的论文和开源项目。
> 以实际代码为准——这些文献提供设计灵感，不代表 SimpleKernel 完全采纳其方案。

---

## 1. 单地址空间操作系统（SASOS）

| 标记 | 引用 | 链接 |
|------|------|------|
| [Opal'94] | Chase et al. "Sharing and Protection in a Single-Address-Space Operating System." ACM TOCS, 1994. | [PDF](https://homes.cs.washington.edu/~levy/opal.pdf) |
| [Opal'92] | Chase et al. "Opal: A Single Address Space System for 64-bit Architectures." ACM SIGOPS OSR, 1992. | [ACM DL](https://dl.acm.org/doi/10.1145/142111.964562) |
| [Mungi'98] | Heiser et al. "The Mungi Single-Address-Space Operating System." SPE, 1998. | [Trustworthy Systems](https://trustworthy.systems/publications/papers/Heiser_EVRL_98.abstract) |
| [Angel'92] | Wilkinson et al. "Angel: A Proposed Multiprocessor Operating System Kernel." EWPC, 1992. | [PDF](https://www.doc.ic.ac.uk/~phjk/Publications/AngelAProposed..EWPC92.pdf) |
| [Nemesis'96] | Leslie et al. "The Design and Implementation of an Operating System to Support Distributed Multimedia Applications." IEEE JSAC, 1996. | [Wikipedia](https://en.wikipedia.org/wiki/Nemesis_(operating_system)) |
| [Pilot'80] | Redell et al. "Pilot: An Operating System for a Personal Computer." CACM, 1980. | [ACM DL](https://dl.acm.org/doi/10.1145/358818.358822) |

## 2. 语言级操作系统隔离

### Rust 内核

| 标记 | 引用 | 链接 |
|------|------|------|
| [Theseus'20] | Boos et al. "Theseus: an Experiment in Operating System Structure and State Management." OSDI'20. | [PDF](https://www.usenix.org/system/files/osdi20-boos.pdf) |
| [Theseus'17] | Boos, Zhong. "Theseus: a State Spill-free Operating System." PLOS'17. | [PDF](https://www.yecl.org/publications/boos2017plos.pdf) |
| [RedLeaf'20] | Narayanan et al. "RedLeaf: Isolation and Communication in a Safe Operating System." OSDI'20. | [PDF](https://www.usenix.org/system/files/osdi20-narayanan_vikram.pdf) |
| [Tock'17] | Levy et al. "Multiprogramming a 64 kB Computer Safely and Efficiently." SOSP'17. | [PDF](https://www.cs.virginia.edu/~bjc8c/papers/levy17tock.pdf) |

### 托管语言内核

| 标记 | 引用 | 链接 |
|------|------|------|
| [Singularity'07] | Hunt, Larus. "Singularity: Rethinking the Software Stack." ACM SIGOPS OSR, 2007. | [MSR](https://www.microsoft.com/en-us/research/project/singularity/) |
| [Singularity'06] | Hunt et al. "Deconstructing Process Isolation." MSR-TR-2006-43. | [MSR](https://www.microsoft.com/en-us/research/publication/deconstructing-process-isolation/) |
| [SPIN'95] | Bershad et al. "Extensibility, Safety and Performance in the SPIN Operating System." SOSP'95. | [PDF](https://cseweb.ucsd.edu/~savage/papers/Sosp95.pdf) |
| [J-Kernel'99] | von Eicken et al. "J-Kernel: A Capability-Based Operating System for Java." Springer, 1999. | [PDF](https://www.cs.cornell.edu/info/people/chichao/sip99.pdf) |
| [KaffeOS'00] | Back et al. "Processes in KaffeOS: Isolation, Resource Management, and Sharing in Java." OSDI'00. | [PDF](https://www.usenix.org/legacy/event/osdi00/full_papers/back/back.pdf) |
| [Verve'10] | Yang, Hawblitzel. "Safe to the Last Instruction: Automated Verification of a Type-Safe Operating System." PLDI'10. | [PDF](https://www.cs.cmu.edu/~jyang2/papers/pldi117-yang.pdf) |
| [Biscuit'18] | Cutler et al. "The Benefits and Costs of Writing a POSIX Kernel in a High-Level Language." OSDI'18. | [PDF](https://www.usenix.org/system/files/osdi18-cutler.pdf) |

## 3. Rust 操作系统项目

| 项目 | 特点 | 链接 |
|------|------|------|
| Theseus | SAS、intralingual 设计、crate 级模块化 | [GitHub](https://github.com/theseus-os/Theseus) / 本地 `ref/Theseus/` |
| Redox | Rust 微内核、scheme VFS、syscall crate | [Website](https://www.redox-os.org/) / [GitHub](https://github.com/redox-os/redox) |
| Asterinas | Framekernel、Linux ABI 兼容、OSTD 安全抽象 | [GitHub](https://github.com/asterinas/asterinas) |
| Tock | 嵌入式 Rust、capability token、Grant 模型 | [Website](https://tockos.org/) / [GitHub](https://github.com/tock/tock) |
| rCore | 清华大学、RISC-V 教学内核 | [GitHub](https://github.com/rcore-os/rCore-Tutorial-v3) |
| Hubris | Oxide 生产级 RTOS、编译期任务定义 | [GitHub](https://github.com/oxidecomputer/hubris) |
| Kerla | Linux ABI 兼容单体内核（已停维） | [GitHub](https://github.com/nuta/kerla) |
| Starina | 现代 Rust 微内核 | [GitHub](https://github.com/starina-os/starina) |

## 4. Rust 安全与内核安全

| 标记 | 引用 | 链接 |
|------|------|------|
| [RustBelt'18] | Jung et al. "RustBelt: Securing the Foundations of the Rust Programming Language." POPL'18. | [PDF](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf) |
| [SafeRust'21] | Jung et al. "Safe Systems Programming in Rust: The Promise and the Challenge." CACM, 2021. | [PDF](https://iris-project.org/pdfs/2021-rustbelt-cacm-final.pdf) |
| [OwnershipTypes'02] | Boyapati et al. "Ownership Types for Safe Programming: Preventing Data Races and Deadlocks." OOPSLA'02. | [PDF](https://web.eecs.umich.edu/~bchandra/publications/oopsla02.pdf) |
| [RFL'24] | Li et al. "Rust for Linux: Understanding the Security Impact of Rust in the Linux Kernel." ACSAC'24. | [PDF](https://mars-research.github.io/doc/2024-acsac-rfl.pdf) |
| [RFL-Empirical'24] | Li et al. "An Empirical Study of Rust-for-Linux." ATC'24. | [PDF](https://www.usenix.org/system/files/atc24-li-hongyu.pdf) |
| [Asterinas-FK'25] | Peng et al. "Asterinas: A Linux ABI-Compatible, Rust-Based Framekernel OS." ATC'25. | [USENIX](https://www.usenix.org/conference/atc25/presentation/peng-yuke) |
| [Converos'25] | Tang et al. "Converos: Practical Model Checking for Verifying Rust OS Kernel Concurrency." ATC'25. | [PDF](https://www.usenix.org/system/files/atc25-tang.pdf) |

## 5. 内核隔离与分解

| 标记 | 引用 | 链接 |
|------|------|------|
| [LCD'16] | Jacobsen et al. "Lightweight Capability Domains: Towards Decomposing the Linux Kernel." ACM SIGOPS OSR, 2016. | [ACM DL](https://dl.acm.org/doi/10.1145/2883591.2883601) |
| [lwC'16] | Litton et al. "Light-Weight Contexts: An OS Abstraction for Safety and Performance." OSDI'16. | [PDF](https://www.usenix.org/system/files/conference/osdi16/osdi16-litton.pdf) |
| [Occlum'20] | Shen et al. "Occlum: Secure and Efficient Multitasking Inside a Single Enclave of Intel SGX." ASPLOS'20. | [GitHub](https://github.com/occlum/occlum) |
