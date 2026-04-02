# 单地址空间（SAS）架构设计

> 日期：2026-04-01
> 状态：已批准，待实施
> 分支：feat/rust-SAS

## 1. 架构定义

SimpleKernel 采用单地址空间（SAS）架构。所有代码——内核基础设施、驱动、任务——运行在同一特权级、同一地址空间中。不存在用户态/内核态分离。

### 1.1 隔离模型

采用 Theseus 式 crate 边界隔离。每个子系统是独立 crate，通过 `pub` trait 暴露窄接口。`unsafe` 代码集中在少数底层 crate（`paging`、`sync`、`interrupt_state`）中并严格审计。

### 1.2 安全保证来源

| 传统 OS | SimpleKernel SAS |
|---------|-----------------|
| MMU 页表隔离用户/内核 | 单一页表，`MappedPages` 仿射类型防止非法映射操作 |
| 特权级切换 (ecall/svc) | Rust 可见性 (`pub(crate)`) + syscall 契约层 |
| copy_from_user / copy_to_user | 不需要——所有数据在同一地址空间直接可达 |
| ASID / TLB flush on context switch | 不需要——共享页表，上下文切换仅保存 callee-saved 寄存器 |

### 1.3 显式放弃

- 不支持运行不受信任的用户态二进制程序
- 不提供硬件强制的进程间内存隔离
- 不实现 U-mode / EL0 代码执行

### 1.4 POSIX 兼容路径

保留集中式 syscall 分发层（`src/syscall/`），编号对齐 Linux ABI。POSIX API 通过该层对内核线程提供标准语义（`open`、`read`、`write`、`mmap`...），但不经过 trap——调用者直接以 Rust 函数调用方式进入。

### 1.5 参考内核

| OS | 借鉴点 |
|----|--------|
| Theseus | crate 级隔离、`MappedPages` 仿射类型、单地址空间 |
| Tock | Grant 机制思想（未采用，但驱动隔离可参考） |
| RedLeaf | 跨域所有权转移理念（未采用，实现成本过高） |
| μFork | 能力机制理念（未采用，架构差异大） |

## 2. Syscall 层改造

### 2.1 移除 trap 入口路径

- 删除 `src/arch/riscv64/syscall.rs`
- 删除 `src/arch/aarch64/syscall.rs`
- 两个架构的 `mod.rs` 中移除 `mod syscall`
- trap handler 中 ecall/svc 异常分支改为 `panic!("ecall/svc 在 SAS 模式下不应触发")`

### 2.2 签名改造

`dispatch` 函数删除。每个 syscall 改为独立的类型安全公开函数：

```rust
// src/syscall/io.rs
pub fn write(fd: usize, buf: *const u8, len: usize) -> isize

// src/syscall/process.rs
pub fn exit(code: i32) -> !
pub fn yield_now()
pub fn clone(entry: fn(usize), arg: usize) -> isize
pub fn waitpid(pid: usize) -> isize
pub fn nanosleep(ms: u64)
pub fn kill(pid: usize, sig: u32) -> isize
```

### 2.3 SyscallNumber 枚举保留

用于日志、审计、统计及 POSIX 合规追踪。不再作为运行时分发键——分发在编译期由函数调用直接完成。

### 2.4 消费者迁移

所有通过 ecall/svc 调用 syscall 的地方（冒烟测试等），改为直接调用 `syscall::process::exit(0)` 等函数。

## 3. Crate 可见性收紧

### 3.1 原则

- 各子系统的内部实现函数标记为 `pub(crate)`
- 只有被 `src/syscall/` 调用的接口才是 `pub`
- `src/syscall/` 本身的函数对整个 kernel crate 可见（`pub`）

### 3.2 调用路径

```
调用者 ──→ syscall::process::exit() ──→ task::exit()
              ↑ pub                        ↑ pub(crate)
              唯一合法入口                   外部不可直接调用
```

### 3.3 例外

boot 路径中的初始化函数（`task::init()`、`memory::init()`）不经过 syscall 层——它们是启动序列的一部分，不是运行时 API。这些保持 `pub`。

### 3.4 scope 控制

此次只收紧已有 7 个 syscall 对应的函数。尚未实现的子系统（fs、device）在开发时直接按新规则设计。

## 4. 受影响文件清单

### 4.1 删除

- `src/arch/riscv64/syscall.rs`
- `src/arch/aarch64/syscall.rs`

### 4.2 修改

| 文件 | 变更 |
|------|------|
| `src/arch/riscv64/mod.rs` | 移除 `mod syscall` |
| `src/arch/aarch64/mod.rs` | 移除 `mod syscall` |
| `src/arch/riscv64/interrupt.rs` | ecall 分支改为 panic |
| `src/arch/aarch64/interrupt.rs` | svc 分支改为 panic |
| `src/syscall/mod.rs` | 删除 `dispatch` 函数，改为 re-export 子模块的类型安全函数 |
| `src/syscall/io.rs` | `sys_write` 签名改为类型安全 |
| `src/syscall/process.rs` | `sys_exit` 等签名改为类型安全 |
| `src/task/mod.rs` + `sched.rs` | `exit`、`yield_now`、`schedule` 等收紧为 `pub(crate)` |
| `src/smoke_test.rs` | ecall/svc 调用改为 `syscall::` 函数调用 |
| `src/main.rs` | boot 路径例外，其余改为经过 syscall 层 |

### 4.3 新增

- `docs/design/SAS-架构设计.md`（本文档）

### 4.4 不修改

- `crates/` 下的所有 crate——已独立，可见性无需调整
- `src/memory/`、`src/boot.rs`——初始化路径，不受影响

### 4.5 估计改动量

~200-300 行（删除 > 新增）

## 5. 测试策略

验证标准：迁移后内核行为与迁移前完全一致。

| 验证项 | 方式 |
|--------|------|
| ecall/svc 不再被调用 | trap handler 中 panic 分支——意外触发会立即崩溃 |
| syscall 函数可正常调用 | `smoke_test.rs` 改为直接调用后正常运行 |
| crate 可见性正确 | `cargo build` 编译通过——绕过 syscall 调用 `pub(crate)` 函数会编译失败 |
| 单元测试 | `cargo test` 通过（x86_64 host） |
| 系统测试 | `cargo xtask test --arch riscv64` 通过 |
| lint | `cargo fmt --check && cargo clippy -- -D warnings` 通过 |

不新增测试——纯重构，行为不变，现有测试覆盖即可。
