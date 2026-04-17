# AGENTS.md — SimpleKernel

## OVERVIEW
Interface-driven OS kernel for AI-assisted learning. Rust (`no_std`, `no_main`), freestanding, nightly toolchain. Two architectures: riscv64, aarch64. Traits define contracts (doc comments with `# Safety`/`# Errors`/`# Panics`), AI generates `impl` blocks, tests verify compliance.

> **架构模型**：SimpleKernel 采用单地址空间（SAS）架构——所有代码运行在同一特权级和地址空间中，不存在用户态/内核态分离。隔离通过 Rust 类型系统 + crate 可见性规则实现（Theseus 式）。详见 `docs/design/SAS-架构设计.md`。

## STRUCTURE
```
src/                  # Kernel source — lib.rs (modules) + main.rs (entry)
src/arch/             # Per-architecture code (riscv64/, aarch64/)
src/boot.rs           # kernel_init() — staged init for kernel & test binaries
crates/               # Workspace crates (memory, sync, per_cpu, paging, ...)
xtask/                # Build tool (cargo xtask run/build/debug/test/firmware)
tests/                # Standalone QEMU test binaries (each runs in isolated QEMU instance)
docs/design/         # Design docs (SAS architecture, subsystem designs, phase plans)
3rd/                  # Git submodules (opensbi, u-boot, optee, atf, dtc — firmware only)
```

## WHERE TO LOOK
- **Implementing a module** → Read trait definition in the module's `mod.rs` or dedicated trait file first
- **Adding a driver** → `src/device/` for examples, `Driver` trait for registration pattern
- **Adding a scheduler** → `src/task/scheduler/mod.rs` for `Scheduler` trait
- **Boot flow** → `src/main.rs`: `_start` → `bootstrap()` → logging → percpu → early_init → memory → paging → timer → interrupt → task → device → fs → SMP → schedule
- **帧生命周期** → `crates/frame_allocator/`: `AllocatedFrames` RAII 所有权（Drop 归还 buddy）
- **帧所有权** → `crates/paging/src/mapping.rs`: `OwnedPages` 持有 `AllocatedFrames`，Drop 恢复默认权限并自动归还帧；内核段通过 `mem::forget` 永久持有
- **System tests** → `tests/` for isolated QEMU tests, each binary in its own QEMU instance
- **Error handling** → `KResult<T> = Result<T, ErrorCode>` in `src/error.rs`
- **Logging** → `log::info!()` / `log::debug!()` via `log` crate, backend in `src/logging.rs`
- **Design overview** → `docs/design/00-概述.md` (master plan — written pre-implementation, may be outdated; code is source of truth)
- **Phase details** → `docs/design/P0-P7` (implementation plans, all phases complete — may diverge from actual code)

## CODE MAP
| Module | Purpose | Key Files |
|--------|---------|-----------|
| `src/main.rs` | `#![no_std]` `#![no_main]` entry, `_start` | main entry point |
| `src/arch/` | Arch-agnostic dispatch via `cfg` | `mod.rs` + `{riscv64,aarch64}/` |
| `src/arch/{arch}/init.rs` | ArchInit, ArchInitSMP | per-arch boot sequence |
| `src/arch/{arch}/console.rs" | Early console (SBI / PL011) | UART output |
| `src/arch/{arch}/interrupt.rs` | PLIC/GIC + trap dispatch | interrupt handling |
| `src/arch/{arch}/timer.rs` | Timer init + tick handler | timer subsystem |
| `src/arch/{arch}/context.rs` | TrapContext, InitTaskContext | `#[repr(C)]` structs |
| `src/arch/{arch}/backtrace.rs` | Stack unwinding | debug support |
| `src/memory/` | 内存子系统门面（re-export crates） | init, map_mmio |
| `src/task/` | TaskManager, TCB, schedulers | CFS/FIFO/RR, clone/exit/wait/sleep/signal |
| `src/task/scheduler/` | `Scheduler` trait + implementations | scheduling algorithms |
| `src/device/` | DeviceManager, Hal, PlatformBus | 设备枚举/注册框架 |
| `src/device/hal.rs` | `virtio-drivers::Hal` trait 实现 | DMA 分配 + MMIO 映射 |
| `src/device/virtio.rs` | VirtIO 块设备探测 + 全局引用 | MmioTransport + VirtIOBlk |
| `src/fs/` | VFS, RamFS, FatFS adapter, FD table | 文件系统层 |
| `src/fs/vfs.rs` | `FileSystem` trait + InodeId/DirEntry/FileType | VFS 抽象 |
| `src/fs/ramfs.rs` | 内存文件系统（BTreeMap 存储） | RamFS 实现 |
| `src/fs/fd_table.rs` | Per-task 文件描述符表 | Fd newtype + alloc/get/close |
| `src/fs/fatfs_adapter.rs` | VirtIO blk → fatfs crate I/O 适配 | 扇区对齐 read-modify-write |
| `src/syscall/` | 类型安全的集中式 API 网关（SAS 模式，不经过 trap） | POSIX 兼容 syscall 编号 |
| `src/sync/spinlock.rs` | SpinLock (interrupt-aware, lock levels) | custom implementation |
| `src/error.rs` | `ErrorCode`, `KResult<T>` | error handling |
| `src/logging.rs` | `log` crate backend + ANSI colors | kernel logging |
| `src/config.rs` | Kernel constants (`MAX_CORE_COUNT`, etc.) | configuration |
| `src/per_cpu.rs` | Per-CPU data + CORE_COUNT | SMP support |
| `src/fdt.rs` | Device tree parser (`fdt` crate wrapper) | hardware discovery |
| `src/elf.rs` | ELF symbol table parser | backtrace support |
| `src/panic.rs` | Panic handler + observer pattern | error recovery |
| `src/lang_items.rs` | `#[panic_handler]` (gated on `lang_items` feature) | Rust runtime |
| `src/boot.rs` | `kernel_init(InitLevel)` + `kernel_init_smp()` | staged init for kernel & tests |
| `tests/test_harness/` | `test_main!` 宏（启动 + 测试 + 退出 QEMU） | test infrastructure |
| `tests/*/` | 独立 QEMU 测试二进制 | standalone tests |
| `xtask/src/test.rs` | `cargo xtask test` orchestration | test runner |

## CONVENTIONS

> Full Rust coding conventions: `docs/design/00-概述.md` §9

### Git
- **Commit 格式**: `<type>(<scope>): <subject>` — type: feat/fix/refactor/test/docs/chore
- **Sign-off 必须**: 每条 commit 必须使用 `git commit --signoff`（DCO 签署），**不可省略**
- **Subagent 派发时**：给 subagent 的 commit 指令中也必须包含 `--signoff`

### Rust
- **Language**: Rust nightly, `#![no_std]`, `#![no_main]`, edition 2024
- **Naming**: `snake_case` functions/methods, `PascalCase` types/traits/enums, `SCREAMING_SNAKE_CASE` constants
- **函数命名惯例**: 返回 `bool` 用 `is_`/`has_`/`can_` 前缀；getter 用名词不加 `get_`/`read_`（如 `len()`）；setter 用 `set_` 前缀；动作用动宾结构（动词在前，如 `flush_tlb()`、`disable_irq()`）
- **Formatting**: `rustfmt.toml` (100 char width), enforce via `cargo fmt`
- **Linting**: `cargo clippy -- -D warnings`
- **Doc comments**: `///` with `# Safety`, `# Errors`, `# Panics` sections for public APIs（节标题保留英文，内容用中文）
- **注释语言**: 所有注释和文档注释使用中文；`// SAFETY:` 前缀保留英文（Rust 社区惯例），其后说明用中文
- **注释位置**: 注释写在代码上方，不写在行尾（`// SAFETY:` 除外——紧跟 `unsafe` 块上方）
- **Error handling**: Syscall boundary uses `Result<T, ErrorCode>` + `?`; kernel internals use `.expect("reason with data")` or `panic!()` — the kernel must not silently proceed on internal errors
- **Unsafe**: Every `unsafe` block MUST have `// SAFETY:` comment explaining invariants; minimize scope
- **Singletons**: `spin::Once<T>` with `call_once()` / `get()`
- **Sync**: Custom `SpinLock<T>` (interrupt-aware), NOT `spin::Mutex` for kernel mutual exclusion
- **Assembly**: `.S` files compiled via `cc` crate in `build.rs`; `#[repr(C)]` for ABI-compatible structs
- **Attributes**: Rust 2024 edition syntax — `#[unsafe(no_mangle)]` (not `#[no_mangle]`)
- **规范引用**: 涉及体系结构、硬件规范的代码，模块文档注释须附上官方文档链接并精确到章节（如 `[Arm ARM §D8.3](https://...)`）

## ANTI-PATTERNS

- **NO** `.unwrap()` — use `.expect("reason with relevant data")` or `?`
- **NO** vague panic/error messages — always include the data that caused the failure (addresses, indices, sizes, etc.)
- **NO** `unsafe` without `// SAFETY:` comment
- **NO** modifying trait definitions to embed implementation (traits = contracts)
- **NO** `spin::Mutex` for kernel mutual exclusion (doesn't disable interrupts)
- **NO** `static mut` — use `SyncUnsafeCell` or `spin::Once<T>`
- **NO** empty `unsafe {}` blocks to bypass borrow checker
- **NO** suppressing warnings with `#[allow(...)]` without justification——使用 `#[expect(..., reason = "...")]` 代替
- **NO** heap allocation in interrupt context (`Box`, `Vec`, `String`, `format!`)——use `heapless` containers or stack buffers
- **NO** ASCII-art 分隔线注释（`// ─── Title ───`、`// === Title ===` 等）——用空行和 doc comment 分组
- **NO** `.map_err(|_| ...)` 丢弃原始错误——使用 `.map_err(|e| ...)` 保留原始错误信息（写入日志或嵌入新错误类型），便于调试
- **NO** 文档/注释中引用本地路径（`ref/Theseus/`、`/home/...`、`~/...`）——使用上游 URL（GitHub 链接等），保证仓库内所有内容对任意 clone 通用

## UNIQUE STYLES
- `spin::Once<T>` with named statics: `TASK_MANAGER.call_once(|| ...)`, `TASK_MANAGER.get().unwrap()`
- `SpinLockGuard<'_, T>` RAII locking (disables/restores interrupts on acquire/release)
- `KResult<T> = Result<T, ErrorCode>` project-wide type alias
- Per-architecture code selected via `#[cfg(target_arch = "...")]`
- Cargo workspace: root package (kernel) + `xtask` (build tool)

## COMMANDS
```bash
# Build kernel
cargo xtask build --arch riscv64
cargo xtask build --arch aarch64

# Run in QEMU (via xtask — handles FIT image + TFTP + QEMU)
cargo xtask run --arch riscv64
cargo xtask run --arch aarch64

# Debug (GDB on localhost:1234)
cargo xtask debug --arch riscv64

# Unit tests (host, pure logic only)
cargo test -p memory_types -p config -p page_table_entry -p arch

# System tests in QEMU
cargo xtask test --arch riscv64                        # all standalone tests
cargo xtask test --arch riscv64 --name panic-test      # specific standalone test
cargo xtask test --list                                # list available tests

# Format + lint check
cargo fmt --check && cargo clippy -- -D warnings

# Documentation
cargo doc --no-deps
```

**QEMU 超时**：在 QEMU 中运行内核或测试时经常出现卡死或无限循环打印日志的情况。所有通过 Bash 工具执行的 QEMU 相关命令（`cargo xtask run`、`cargo xtask test`）**必须设置 30 秒超时**（`timeout: 30000`）。超时后应 `pkill -f qemu-system` 清理残留进程。

## TESTING

两层测试体系 + 冒烟测试：纯逻辑单元测试（宿主机）、独立 QEMU 系统测试、冒烟测试（内核启动时自动运行）。

### 纯逻辑单元测试（Host Unit Tests）

在宿主机上运行，测试不涉及硬件的纯计算逻辑。

```bash
cargo test -p memory_types -p config -p page_table_entry -p arch  # 全部纯逻辑测试
cargo test -p memory_types                        # 单个 crate
cargo test -p config -- page_size --nocapture     # 单个测试（显示输出）
```

适用范围：地址运算、PTE 编解码、常量验证等。
在模块内用 `#[cfg(test)] mod tests { ... }` 编写，标准 `#[test]` 宏。

### 独立 QEMU 系统测试（Standalone Tests）

每个测试是独立的 `#![no_std]` 裸机二进制，启动独立 QEMU 实例，拥有干净的内核环境。

```bash
cargo xtask test --arch riscv64                        # 全部独立测试
cargo xtask test --arch riscv64 --name frame-alloc-test # 指定测试
cargo xtask test --list                                # 列出可用测试
```

测试基础设施位于 `tests/test_harness/`，核心是 `test_main!` 宏：

```rust
// 普通测试
test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

// should_panic 测试（期望 panic 则通过）
test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_test, should_panic);
```

`test_main!` 负责：`_start` 入口 → `kernel_init(level)` → 调用测试函数 → `exit_qemu(0)`。
断言失败 = panic = QEMU 非零退出 = 测试失败。

#### 添加独立测试

**在已有模块包中添加（推荐）：**
1. 在对应包的 `src/` 下创建新文件（如 `tests/paging-test/src/my_new_test.rs`）
2. 在该包的 `Cargo.toml` 中添加 `[[bin]]` 条目，指定 `name` 和 `path`，设置 `test = false`
3. `src/` 文件中使用 `test_harness::test_main!` 宏
4. should_panic 测试使用 `test_main!(level, fn, should_panic)` 变体
5. xtask 自动扫描 `[[bin]]` 条目发现新测试

**创建新模块包：**
1. 创建 `tests/my-test/`，包含 `Cargo.toml`（至少一个 `[[bin]]` 条目）和对应源文件
2. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径
3. xtask 自动扫描 `tests/*/Cargo.toml` 中的 `[[bin]]` 条目发现新测试

### 冒烟测试（Boot Smoke Tests）

内核 `boot.rs::kernel_init()` 各阶段完成后自动运行关键断言（SpinLock 基本操作、堆分配、帧分配），确保基础设施正常。这些测试在每次内核启动（包括独立测试二进制启动）时自动执行，无需手动触发。

### 测试规范

- 每个测试函数必须有 `///` 文档注释
- 独立测试二进制的 `Cargo.toml` 中设置 `test = false`（不使用标准测试 harness）
- 测试 crate 依赖 `simplekernel` lib，通过 `kernel_init()` 复用内核初始化流程
- CI 中系统测试会重复运行多次以验证稳定性（PR: 3 次，push: 10 次），每个测试超时 120 秒

## DESIGN REFERENCES
设计和实现新模块时，应参考以下成熟内核和论文，取其精华。完整参考文献见 `docs/design/references.md`。

### 参考内核
- **[Linux](https://github.com/torvalds/linux)** — 工业级参考，尤其是调度器（CFS）、VFS、内存管理（`vm_area_struct`）、信号处理
- **[Zephyr](https://github.com/zephyrproject-rtos/zephyr)** — 嵌入式/RTOS 视角，轻量级线程模型、设备驱动框架（device model + devicetree）、电源管理
- **[Theseus](https://github.com/theseus-os/Theseus)** — Rust 类型系统深度利用，`MappedPages` RAII 映射管理、crate 级模块化、`#![forbid(unsafe_code)]` APP 隔离
- **[Redox](https://github.com/redox-os/redox)** — Rust 微内核实践，scheme-based VFS、`syscall` crate 设计、reliability crate 拆分（注：SimpleKernel 不采用微内核的用户态驱动模型，仅参考其 API 设计）
- **[Tock](https://github.com/tock/tock)** — 嵌入式 Rust 内核，`unsafe trait` capability 模式、Grant 内存模型
- **[Asterinas](https://github.com/asterinas/asterinas)** — Framekernel 架构（framework 可 unsafe + services 纯 safe Rust），Linux ABI 兼容
- **[rCore](https://github.com/rcore-os/rCore-Tutorial-v3)** — 清华大学 RISC-V 教学 Rust 内核，启动流程和页表实现参考

### 关键论文
- [Theseus OSDI'20](https://www.usenix.org/system/files/osdi20-boos.pdf) — intralingual OS：Rust 编译器即保护环
- [RedLeaf OSDI'20](https://www.usenix.org/system/files/osdi20-narayanan_vikram.pdf) — 语言域隔离 + 跨域故障恢复
- [Tock SOSP'17](https://www.cs.virginia.edu/~bjc8c/papers/levy17tock.pdf) — Rust 嵌入式内核 capability 模式
- [SPIN SOSP'95](https://cseweb.ucsd.edu/~savage/papers/Sosp95.pdf) — 语言安全内核扩展（Modula-3），SAS 隔离的早期实践
- [Singularity MSR'05-'07](https://www.microsoft.com/en-us/research/project/singularity/) — SIP 软件隔离进程，量化 SAS 性能优势
- [Opal TOCS'94](https://homes.cs.washington.edu/~levy/opal.pdf) — SAS 保护模型理论基础
- [Mungi SPE'98](https://trustworthy.systems/publications/papers/Heiser_EVRL_98.abstract) — SAS + capability 保护
- [RustBelt POPL'18](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf) — Rust 安全模型形式化证明
- [Asterinas Framekernel ATC'25](https://www.usenix.org/conference/atc25/presentation/peng-yuke) — 内核内特权分离，TCB 14%
- [Rust for Linux ACSAC'24](https://mars-research.github.io/doc/2024-acsac-rfl.pdf) — Rust 消除 91% 驱动安全漏洞的量化分析

## CURRENT PHASE
> **⚠ 临时节——审计结束后清理**
>
> 项目当前处于全项目深度审计阶段。以下内容仅在审计期间有效，审计完成后须移除。

### 审计相关文件
- **Roadmap**（全局计划、排查 checklist、协作流程）: `docs/audit/review-roadmap.md`
- **审计进度**（跨对话上下文传递）: `docs/audit/audit-progress.md`
- **Session Prompt**（输出格式参考）: `docs/audit/review-session-prompt.md`
- **ADR 目录**（架构决策记录）: `docs/decisions/`
- **ADR 模板**: `docs/templates/adr-template.md`

### 审计工作流

当用户发起审计任务（如"审计 R2 crates/sync/"、"继续审计"等）时，按以下流程执行：

**启动阶段：**
1. Read 整个 `docs/audit/review-roadmap.md`（定位审查范围和标准排查流程）
2. Read `docs/audit/audit-progress.md`（获取历史上下文和当前进度）
3. 如果用户说"继续审计"且未指定目标，从 audit-progress.md 的"下一个目标"继续
4. Read 排查目标的所有源文件

**排查阶段（按 Roadmap 中"标准排查流程"执行）：**
1. 代码审查（trait → impl → unsafe → error handling → 全局状态 → pub 接口）
2. Rust 范式审查（所有权 / 生命周期 / typestate / RAII / 零成本 / 错误处理）
3. 并发安全审查（Send/Sync / 多核竞态 / 中断重入 / 锁序 / atomic ordering）
4. 依赖与版本检查（crate 版本 / nightly 特性 / 可替代 crate 评估）
5. 参考内核对比（Linux / Theseus / Redox / Zephyr / µFork）
6. 接口契约审查（doc comment 完整性）

**输出阶段（按 `review-session-prompt.md` 中的格式输出）：**
- 模块概述、问题清单、依赖与版本、Crate 替代评估、参考内核对比、设计讨论点、建议的代码修改、文档产出建议

**停止条件：**
- 完成审查报告后**停下来等待用户反馈**，不要直接开始修改代码
- 代码修改在讨论完设计问题后进行

**结束阶段：**
- 对话结束时，将以下内容写入 `docs/audit/audit-progress.md`：
  - 更新"当前状态"（当前 Phase + 下一个目标）
  - 更新"上次对话摘要"（已完成、关键问题、未决设计问题、下一步）
  - 追加"已完成的目标"记录

### 审计约束
- 设计讨论点：列出备选方案和客观优缺点，**不要推荐某个方案**，标注 ADR 待决
- Crate 替代：列出候选和优缺点，**不要自行替换**，需经讨论后决定
- ADR 状态：AI 生成的 ADR 状态**必须为"提议"**，只有项目作者 review 后才可改为"已接受"

## COLLABORATION STYLE
> **⚠ 临时节——审计结束后清理**

- 项目作者是 C/C++ 背景，正在学习 Rust。在编写或审阅代码时：
  - 遇到 Rust 特有的语法、惯用法、设计模式时，主动用 C/C++ 类比解释
  - 指出 Rust 写法与 C/C++ 的关键差异（所有权、生命周期、trait vs 虚函数、enum vs union+tag 等）
  - 不要假设用户熟悉 Rust 高级特性（typestate、GAT、`PhantomData` 等），使用时需简要说明

## NOTES
- **SAS architecture**: single address space, no user/kernel split. Isolation via Rust type system + crate visibility (`pub(crate)`). Syscall layer (`src/syscall/`) is the only public cross-module API gateway — direct function calls, no trap (ecall/svc).
- **Kernel encapsulation** (three-layer isolation, ref: [Theseus OSDI'20], [Tock SOSP'17], [SPIN SOSP'95]):
  1. APP crate 必须标记 `#![forbid(unsafe_code)]` — 编译器强制禁止 unsafe，APP 无法绕过类型系统
  2. 内核内部接口使用 `pub(crate)` — APP crate 无法访问内核内部符号
  3. `src/syscall/` 是唯一 `pub` 跨 crate 接口 — APP 的一切内核访问必须经过此网关
  - Syscall 接口使用 Rust 类型（非裸 `usize`），编译器保证类型安全；运行时只验证编译器无法保证的部分（如 Fd 是否有效）
  - 未来如需 APP 权限分级，可引入 Tock 式 capability token（`unsafe trait` 作为编译期访问控制），当前不需要
- **Kernel-internal error policy** (bug = panic, expected error = Result):
  - 不变量违反、不可达路径、逻辑错误 → `panic!()` / `.expect()` — 内核 bug 必须立即暴露，fail-fast
  - 资源耗尽（OOM）、外部设备失败 → `Result<T, ErrorCode>` — 向上返回，由 syscall 层决定如何报给 APP
  - 判断标准：**"这不应该发生" → panic；"这可能发生" → Result**
- **Error diagnostics**: panic/error messages MUST include the actual data that caused the failure, not just the reason. E.g., `panic!("invalid page-aligned address: {:#x}", addr)` instead of `panic!("invalid address")`. This applies to `.expect()`, `panic!()`, and `log::error!()`.
- Interface-driven: traits are contracts, `impl` blocks are implementations AI generates
- Boot chains differ: riscv64 (U-Boot SPL→OpenSBI→U-Boot), aarch64 (U-Boot→ATF→OP-TEE)
- Debug: use `cargo xtask debug` + GDB, QEMU logs in build output
- Design docs: `docs/design/` contains design documents written **before implementation**. They may be outdated — **always treat actual code as the source of truth**. When design docs conflict with code, trust the code and flag the discrepancy
- Phase plans: `docs/design/P0-P7` — all phases (P0-P7) implementation complete
