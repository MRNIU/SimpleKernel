# 系统测试框架设计

## 概述

为 SimpleKernel 添加 QEMU 环境下的系统测试能力。采用混合方案：统一测试内核覆盖常规测试，独立二进制覆盖破坏性测试。测试代码作为独立 crate，依赖内核 lib 部分，替换入口点。

## 决策记录

| 维度 | 决定 | 理由 |
|------|------|------|
| 组织方式 | 混合：统一测试内核 + 独立二进制 | 兼顾效率和隔离性 |
| 覆盖范围 | 全量（内存/任务/设备/FS/panic/OOM/syscall） | 第一期即完整覆盖 |
| 代码关系 | 测试 crate 依赖内核 lib，替换 main.rs | 复用内核代码，测试逻辑独立 |
| 执行调度 | 分组：组内顺序，需要并发的组用多线程 | 平衡测试能力和可控性 |
| 输出格式 | 类 `cargo test` 风格 + `qemu-exit` 退出码 | 可读性好，CI 友好 |
| 触发方式 | `cargo xtask test --arch` + `--name` 指定独立测试 | 统一入口，灵活选择 |
| 内核拆分 | `src/lib.rs` + `src/main.rs` 同 crate 内拆分 | 最小改动，自然支持 |

## 1. 内核 crate 拆分

### 当前状态

`src/main.rs` 既声明所有模块又包含 `_start` 入口，无法被外部 crate 依赖。

### 拆分方案

在当前 crate 内拆分为 `lib.rs` + `main.rs`：

- **`src/lib.rs`**：`#![no_std]` 顶层属性，声明并 re-export 所有内核子模块（`arch`、`memory`、`task`、`device`、`fs`、`sync`、`error`、`logging`、`config`、`per_cpu`、`fdt`、`elf`、`panic` 等）
- **`src/main.rs`**：仅保留 `#![no_main]`、`_start` 入口函数和启动流程，通过 `use simplekernel::*` 引用 lib
- **`Cargo.toml`**：Rust 自然支持同 crate 的 `[lib]` + `[[bin]]`，无需额外配置

### 公共初始化接口

在 `src/lib.rs` 中暴露分级初始化函数，避免每个测试 binary 复刻启动序列：

```rust
/// 内核初始化级别
pub enum InitLevel {
    /// 日志 + per_cpu + early_init + 内存子系统
    Memory,
    /// Memory + MMIO 映射 + 页表激活 + 定时器 + 中断
    Interrupt,
    /// Interrupt + 任务子系统 + SMP 唤醒（完整初始化）
    Full,
}

/// 初始化内核子系统到指定级别
///
/// # Safety
/// 必须在 bare-metal 环境调用，且每个级别只能调用一次。
/// `dtb_addr` 必须指向有效的设备树 blob。
pub unsafe fn kernel_init(dtb_addr: usize, level: InitLevel) { ... }
```

启动汇编（`.S` 文件）和 linker script 通过内核 crate 的 `build.rs` 编译，测试 crate 的 `build.rs` 引用相同的 linker script 路径（不复制）。

## 2. 目录结构

```
src/
  lib.rs              # re-export 所有模块 + kernel_init() + InitLevel
  main.rs             # _start → kernel_init(Full) → idle loop
  boot.rs             # 启动序列逻辑（kernel_init 实现）

tests/
  system/
    Cargo.toml        # dep: simplekernel = { path = "../.." }
    build.rs          # 引用内核的 linker script（不复制）
    src/
      main.rs         # _start → kernel_init(Full) → runner.run() → qemu_exit
      framework.rs    # TestRunner, TestCase, 输出格式化
      memory.rs       # 内存测试组
      sync.rs         # 同步原语测试组
      task.rs         # 任务测试组
      device.rs       # 设备测试组
      fs.rs           # 文件系统测试组
      syscall.rs      # 系统调用测试组
      smp.rs          # SMP 测试组

  standalone/
    panic_test/       # 验证 panic handler 行为
      Cargo.toml
      build.rs
      src/main.rs
    oom_test/         # 耗尽内存后的错误处理
      Cargo.toml
      build.rs
      src/main.rs
    stack_overflow/   # 栈溢出检测
      Cargo.toml
      build.rs
      src/main.rs
```

### Workspace 配置

```toml
[workspace]
members = [
    ".",
    "crates/*",
    "xtask",
    "tests/system",
    "tests/standalone/panic_test",
    "tests/standalone/oom_test",
    "tests/standalone/stack_overflow",
]
default-members = ["."]  # cargo build 默认只编译内核
```

测试 crate 不在 `default-members` 中，避免 `cargo build` 时目标架构冲突。编译由 `xtask test` 显式触发。

## 3. 测试框架（`tests/system/src/framework.rs`）

作为统一测试内核的内部模块，不单独成 crate。

### 核心类型

```rust
/// 单个测试用例
pub struct TestCase {
    pub name: &'static str,
    pub run: fn(),
}

/// 顺序测试组
pub struct TestGroup {
    pub name: &'static str,
    pub tests: &'static [TestCase],
}

/// 并发测试线程
pub struct ThreadTestCase {
    pub name: &'static str,
    pub run: fn(usize),  // 参数为线程 ID
}

/// 并发测试组
pub struct ConcurrentTestGroup {
    pub name: &'static str,
    pub threads: &'static [ThreadTestCase],
    pub thread_count: usize,
    pub verify: fn(),  // 所有线程结束后的验证
}

/// 测试结果
pub struct TestResult {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<(&'static str, &'static str)>,  // (group, test_name)
}

/// 测试运行器
pub struct TestRunner {
    groups: Vec<TestGroup>,
    concurrent_groups: Vec<ConcurrentTestGroup>,
}
```

### 输出格式

```
=== SimpleKernel System Tests ===

running 5 tests in group "memory"
test page_map_unmap ... ok
test heap_alloc_free ... ok
test heap_grow ... ok
test frame_alloc_dealloc ... ok
test large_allocation ... ok
group result: ok. 5 passed; 0 failed

running 3 tests in group "sync" (concurrent)
test spinlock_contention (4 threads) ... ok
test kmutex_fairness (4 threads) ... ok
test rwlock_readers (4 threads) ... FAILED
group result: FAILED. 2 passed; 1 failed

================================
test result: FAILED. 18 passed; 1 failed
```

### 失败捕获

测试函数通过 `assert!` / `assert_eq!` 系列宏断言。在 bare-metal 环境中 panic 会触发 panic handler。

框架通过以下机制处理测试中的 panic：

1. Runner 在调用每个测试前，将全局 `CURRENT_TEST: Option<&'static str>` 设为当前测试名
2. 测试 binary 的 panic handler 检查 `CURRENT_TEST`：
   - 若有值：记录测试名到 `FAILED_TESTS` 列表，输出 `test {name} ... FAILED (panic: {message})`，然后**终止当前测试组**（跳到下一组的入口），因为 bare-metal 环境下单个 panic 后的栈状态不可信
   - 若无值（非测试代码 panic）：按正常 panic handler 处理（输出 backtrace + `qemu-exit(1)`）
3. 组间跳转通过 `setjmp`/`longjmp` 风格的汇编实现（runner 在每组开始前保存寄存器上下文，panic handler longjmp 回到 runner）
4. 每组最多因 panic 跳过一次，runner 继续执行后续组

## 4. 测试分组与依赖

### 分层原则

测试组按依赖关系排序——先测底层，再测上层：

```
1. memory    （顺序）  — 页表、帧分配、堆
2. sync      （顺序）  — SpinLock、KMutex 基本正确性
3. smp       （顺序）  — 多核启动、per-cpu 数据
4. sync-mt   （并发）  — SpinLock/KMutex 多线程竞争（依赖 task 正常工作）
5. task      （顺序）  — 线程创建/销毁、sleep、yield（不用 spawn 测 spawn）
6. task-mt   （并发）  — clone/wait、信号、调度器公平性
7. device    （顺序）  — VirtIO 块设备读写
8. fs        （顺序）  — VFS/RamFS/FatFS
9. syscall   （顺序）  — ecall/svc 路径
```

**关键设计**：测试 `task` 子系统本身时用顺序模式（不依赖 `task::spawn`），只有在 task 测试通过后，后续组才用并发模式。

### 组间 Checkpoint

每个组结束后执行健全性检查：
- 帧分配器：已分配帧数无异常增长
- 堆：无严重碎片化
- 无死锁（所有锁已释放）

异常时输出警告但不终止（bare-metal 无法重置状态），帮助定位跨组影响。

## 5. 独立二进制测试

### 共同特征

- 各自是 `#![no_std]` + `#![no_main]` 的完整 binary
- 依赖 `simplekernel` lib，调用 `kernel_init()` 到所需级别
- 通过 `qemu-exit` 退出
- 共享内核的 linker script（`build.rs` 引用）

### 判定逻辑

独立测试的通过/失败判定可以是**反转**的：

| 测试 | 期望行为 | 判定方式 |
|------|----------|----------|
| `panic_test` | 触发 panic，输出 backtrace，然后正常退出 | 输出包含 `PANIC_TEST_TRIGGERED`，退出码 0 |
| `oom_test` | 分配失败返回 `ErrorCode`，不 panic | 退出码 0 |
| `stack_overflow` | 检测到栈溢出并安全处理 | 输出包含 `STACK_OVERFLOW_DETECTED`，退出码 0 |

`xtask test` 根据测试名称决定判定逻辑（可在测试 crate 的 `Cargo.toml` 中用 metadata 声明）。

## 6. xtask 集成

### 新增子命令

```bash
# 运行统一测试内核
cargo xtask test --arch riscv64

# 运行指定独立测试
cargo xtask test --arch riscv64 --name panic_test

# 运行全部（统一 + 所有独立）
cargo xtask test --arch riscv64 --all

# 列出可用测试
cargo xtask test --list
```

### 执行流程

```
cargo xtask test --arch riscv64
  1. 编译目标测试 binary（cargo build -p system-test --target riscv64gc-unknown-none-elf）
  2. 生成 FIT 镜像（复用现有 generate_fit_image）
  3. 设置 TFTP（复用现有 setup_tftp）
  4. 启动 QEMU（复用现有 launch_qemu，替换内核 binary 路径）
  5. 捕获串口输出
  6. 等待 QEMU 退出（qemu-exit 触发）
  7. 检查退出码 + 解析输出
  8. 打印结果摘要
```

### 超时处理

- 默认 300 秒超时（与 CI 一致）
- 超时视为测试失败，kill QEMU 进程
- `--timeout` 参数可自定义

## 7. CI 集成

```yaml
- name: System tests
  run: cargo xtask test --arch ${{ matrix.arch }} --all
  timeout-minutes: 10
```

替换现有的 `cargo xtask run` + grep 逻辑，直接依赖 `xtask test` 的退出码。

## 8. 迁移路径

### 从现有烟雾测试迁移

`src/smoke_test.rs` 中的现有测试逐步迁移到 `tests/system/`：

| 现有烟雾测试 | 目标测试组 |
|-------------|-----------|
| Phase 2: SpinLock | `sync` 组 |
| Phase 2: ELF parser | `memory` 组 |
| Phase 3: Heap/Box | `memory` 组 |
| Phase 4: SMP | `smp` 组 |
| P5a: Lock contention | `sync-mt` 并发组 |
| P5b: Sleep/clone/wait | `task-mt` 并发组 |
| P5b: Signal | `task-mt` 并发组 |

迁移完成后，删除 `src/smoke_test.rs` 及 `main.rs` 中的 smoke test 调用。

## 9. 未涵盖 / 后续考虑

- **代码覆盖率**：bare-metal 环境下收集覆盖率较复杂，不在第一期范围
- **性能基准测试**：可作为后续测试组扩展
- **模糊测试**：syscall 接口的 fuzz 测试可在后续引入
- **测试过滤**：`cargo xtask test --arch riscv64 --group memory` 只跑指定组，可在第一期后添加
