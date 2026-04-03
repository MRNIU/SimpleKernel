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
tests/system/         # Unified system test kernel (QEMU, all test groups)
tests/standalone/     # Standalone test binaries (panic_test, oom_test, ...)
docs/design/         # Design docs (SAS architecture, subsystem designs, phase plans)
3rd/                  # Git submodules (opensbi, u-boot, optee, atf, dtc — firmware only)
```

## WHERE TO LOOK
- **Implementing a module** → Read trait definition in the module's `mod.rs` or dedicated trait file first
- **Adding a driver** → `src/device/` for examples, `Driver` trait for registration pattern
- **Adding a scheduler** → `src/task/scheduler/mod.rs` for `Scheduler` trait
- **Boot flow** → `src/main.rs`: `_start` → `bootstrap()` → logging → percpu → early_init → memory → paging → timer → interrupt → task → device → fs → SMP → schedule
- **System tests** → `tests/system/` for unified test kernel, `tests/standalone/` for isolated tests
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
| `src/memory/` | Virtual/physical memory, heap | page tables, frame allocator, `#[global_allocator]` |
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
| `tests/system/src/framework.rs` | TestRunner, TestCase, TestGroup | test framework |
| `tests/system/src/main.rs` | Test kernel entry: init → run tests → qemu_exit | system test binary |
| `tests/system/src/memory_tests.rs` | Heap allocation tests (Box, Vec, large) | memory test group |
| `tests/system/src/sync_tests.rs` | SpinLock tests (basic, modify, drop) | sync test group |
| `tests/system/src/device_tests.rs` | DeviceManager + VirtIO blk 验证 | device test group |
| `tests/system/src/fs_tests.rs` | VFS 路径解析 + RamFS CRUD + 多级目录 | fs test group |
| `tests/standalone/panic_test/` | Verifies panic handler triggers correctly | standalone test |
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
- **Formatting**: `rustfmt.toml` (100 char width), enforce via `cargo fmt`
- **Linting**: `cargo clippy -- -D warnings`
- **Doc comments**: `///` with `# Safety`, `# Errors`, `# Panics` sections for public APIs（节标题保留英文，内容用中文）
- **注释语言**: 所有注释和文档注释使用中文；`// SAFETY:` 前缀保留英文（Rust 社区惯例），其后说明用中文
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

# Unit tests (x86_64 host only)
cargo test

# System tests in QEMU
cargo xtask test --arch riscv64           # unified test kernel
cargo xtask test --arch riscv64 --all     # unified + all standalone
cargo xtask test --arch riscv64 --name panic-test  # specific standalone test
cargo xtask test --list                   # list available tests

# Format + lint check
cargo fmt --check && cargo clippy -- -D warnings

# Documentation
cargo doc --no-deps
```

## TESTING

三层测试体系：单元测试（host）、系统测试（QEMU 裸机）、独立测试（QEMU 隔离场景）。

### 单元测试（Unit Tests）

在 x86_64 宿主机上运行，测试与体系结构无关的纯逻辑代码。

```bash
cargo test                                        # 全部单元测试
cargo test -p memory_types                        # 单个 crate
cargo test alignment_basic -- --nocapture         # 单个测试（显示输出）
```

适用范围：`crates/` 下的地址运算、页表参数推导、调度算法、ELF 解析等。
在模块内用 `#[cfg(test)] mod tests { ... }` 编写，标准 `#[test]` 宏。

### 系统测试（System Tests）

一个 `#![no_std]` 裸机测试内核，在 QEMU 中完整引导后运行所有测试组。

```bash
cargo xtask test --arch riscv64                   # 运行统一测试内核
cargo xtask test --arch aarch64                   # aarch64 架构
cargo xtask test --arch riscv64 --all             # 统一测试 + 全部独立测试
cargo xtask test --list                           # 列出所有可用测试
```

测试框架位于 `tests/system/src/framework.rs`，核心类型：

| 类型 | 用途 |
|------|------|
| `TestCase { name, run: fn() }` | 单个测试用例 |
| `TestGroup { name, tests }` | 测试组（静态数组） |
| `TestRunner` | 收集组、依次执行、统计结果 |

现有测试组（在 `tests/system/src/main.rs` 中注册）：

| 组 | 文件 | 测试内容 |
|----|------|----------|
| memory | `memory_tests.rs` | 堆分配（Box、Vec、大块） |
| sync | `sync_tests.rs` | SpinLock 基本操作、RAII 语义 |
| device | `device_tests.rs` | DeviceManager、VirtIO 块设备读取 |
| fs | `fs_tests.rs` | VFS 路径解析、RamFS CRUD、多级目录 |

引导流程：`_start` → `kernel_init(InitLevel::Full)` → 注册测试组 → `runner.run()` → `exit_qemu(0/1)`。
断言失败 = panic = 测试内核立即终止（裸机环境无法捕获 panic）。

#### 添加系统测试

1. 在 `tests/system/src/` 新建 `xxx_tests.rs`，导出 `pub fn tests() -> &'static [TestCase]`
2. 在 `tests/system/src/main.rs` 中 `runner.add_group(TestGroup { name: "xxx", tests: xxx_tests::tests() })`

### 独立测试（Standalone Tests）

独立的裸机二进制，用于测试无法在统一测试内核中验证的场景（如 panic 行为、OOM 处理）。

```bash
cargo xtask test --arch riscv64 --name panic-test   # 运行指定独立测试
```

现有独立测试：`tests/standalone/panic_test/` — 验证 panic handler 正确触发。

#### 添加独立测试

1. 创建 `tests/standalone/my-test/`，包含 `Cargo.toml`（`name = "my-test"`）和 `src/main.rs`
2. `src/main.rs` 提供 `_start` 入口，按需调用 `kernel_init(InitLevel::...)` 选择初始化级别
3. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径
4. xtask 会自动扫描 `tests/standalone/*/Cargo.toml` 发现新测试

### 测试规范

- 每个测试函数必须有 `///` 文档注释
- 系统/独立测试二进制的 `Cargo.toml` 中设置 `test = false`（不使用标准测试 harness）
- 测试 crate 依赖 `simplekernel` lib，通过 `kernel_init()` 复用内核初始化流程
- CI 中系统测试会重复运行多次以验证稳定性（PR: 3 次，push: 10 次），每次超时 300 秒

## DESIGN REFERENCES
设计和实现新模块时，应参考以下成熟内核的对应实现，取其精华：
- **Linux** — 工业级参考，尤其是调度器（CFS）、VFS、内存管理（`vm_area_struct`）、信号处理
- **Zephyr** — 嵌入式/RTOS 视角，轻量级线程模型、设备驱动框架（device model + devicetree）、电源管理
- **Theseus** — Rust 类型系统深度利用，`MappedPages` RAII 映射管理、crate 级模块化、`DeadlockPrevention` trait 参数化同步原语
- **Redox** — Rust 微内核实践，scheme-based VFS、`syscall` crate 设计、reliability crate 拆分（注：SimpleKernel 不采用微内核的用户态驱动模型，仅参考其 API 设计）

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
- **Kernel encapsulation**: future APP code can ONLY access kernel through `src/syscall/` interfaces. These interfaces MUST guarantee safety — validate all inputs, return `KResult<T>` for recoverable errors. Kernel internals (`pub(crate)`) are not accessible to APPs.
- **Kernel-internal error policy**: the kernel is designed to be infallible. Internal errors (invariant violations, impossible states) MUST panic immediately — fail-fast, no silent error propagation. `Result` is for syscall boundaries; inside the kernel, use `.expect("descriptive reason")` or `panic!()`.
- **Error diagnostics**: panic/error messages MUST include the actual data that caused the failure, not just the reason. E.g., `panic!("invalid page-aligned address: {:#x}", addr)` instead of `panic!("invalid address")`. This applies to `.expect()`, `panic!()`, and `log::error!()`.
- Interface-driven: traits are contracts, `impl` blocks are implementations AI generates
- Boot chains differ: riscv64 (U-Boot SPL→OpenSBI→U-Boot), aarch64 (U-Boot→ATF→OP-TEE)
- Debug: use `cargo xtask debug` + GDB, QEMU logs in build output
- Design docs: `docs/design/` contains design documents written **before implementation**. They may be outdated — **always treat actual code as the source of truth**. When design docs conflict with code, trust the code and flag the discrepancy
- Phase plans: `docs/design/P0-P7` — all phases (P0-P7) implementation complete
