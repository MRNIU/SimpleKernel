# 合并测试包（一个模块一个包） Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 22 个独立测试包按模块合并为 10 个包，每个包用 `[[bin]]` 管理多个二进制，减少 workspace 碎片。

**Architecture:** 同模块的测试源文件合并到一个 Cargo 包目录下，每个二进制对应一个 `[[bin]]` 条目。xtask 的测试发现逻辑从"每包一个二进制"改为"扫描 `[[bin]]` 条目"，构建命令从 `-p pkg` 改为 `-p pkg --bin name`。

**Tech Stack:** Cargo workspace, `[[bin]]` multi-binary packages, xtask (xshell)

---

## File Structure

合并后的 `tests/` 目录：

```
tests/
├── test_harness/          # 不变
├── memory-types-test/     # 合并 5 个包
│   ├── Cargo.toml         # 5 个 [[bin]]
│   └── src/
│       ├── main.rs                          # 原 memory-types-test
│       ├── align_up_overflow_panic.rs       # 原 memory-types-align-up-overflow-panic-test
│       ├── frame_2m_unaligned_panic.rs      # 原 memory-types-frame-2m-unaligned-panic-test
│       ├── pa_overflow_panic.rs             # 原 memory-types-pa-overflow-panic-test
│       └── va_canonical_panic.rs            # 原 memory-types-va-canonical-panic-test
├── paging-test/           # 合并 6 个包
│   ├── Cargo.toml         # 6 个 [[bin]]
│   └── src/
│       ├── main.rs                          # 原 paging-basic-test（重命名为 paging-basic-test 二进制）
│       ├── table.rs                         # 原 paging-table-test
│       ├── mapping.rs                       # 原 paging-mapping-test
│       ├── conflict_panic.rs               # 原 paging-conflict-panic-test
│       ├── equal_range_panic.rs            # 原 paging-equal-range-panic-test
│       └── reversed_range_panic.rs         # 原 paging-reversed-range-panic-test
├── sync-test/             # 合并 4 个包
│   ├── Cargo.toml         # 4 个 [[bin]]
│   └── src/
│       ├── main.rs                          # 原 sync-spinlock-test（重命名为 sync-spinlock-test 二进制）
│       ├── lockstack.rs                     # 原 sync-lockstack-test
│       ├── lockstack_pop_mismatch_panic.rs  # 原 sync-lockstack-pop-mismatch-test
│       └── recursive_lock_panic.rs          # 原 sync-recursive-lock-test
├── frame-test/            # 合并 2 个包
│   ├── Cargo.toml         # 2 个 [[bin]]
│   └── src/
│       ├── main.rs                          # 原 frame-alloc-test（重命名为 frame-alloc-test 二进制）
│       └── mapped_drop_panic.rs             # 原 frame-mapped-drop-test
├── panic_test/            # 不变
├── pte-test/              # 不变
├── heap-test/             # 不变
├── device-test/           # 不变
├── fs-test/               # 不变
└── vma-test/              # 不变
```

二进制名保持不变（如 `memory-types-test`、`memory-types-align-up-overflow-panic-test`），确保 `cargo xtask test --name xxx` 向后兼容。

---

### Task 1: 合并 memory-types 测试包（5→1）

**Files:**
- Create: `tests/memory-types-test/src/align_up_overflow_panic.rs`
- Create: `tests/memory-types-test/src/frame_2m_unaligned_panic.rs`
- Create: `tests/memory-types-test/src/pa_overflow_panic.rs`
- Create: `tests/memory-types-test/src/va_canonical_panic.rs`
- Modify: `tests/memory-types-test/Cargo.toml`
- Delete: `tests/memory-types-align-up-overflow-panic-test/` (entire directory)
- Delete: `tests/memory-types-frame-2m-unaligned-panic-test/` (entire directory)
- Delete: `tests/memory-types-pa-overflow-panic-test/` (entire directory)
- Delete: `tests/memory-types-va-canonical-panic-test/` (entire directory)
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 将 4 个 panic 测试的源文件移到 memory-types-test/src/ 下**

```bash
cp tests/memory-types-align-up-overflow-panic-test/src/main.rs tests/memory-types-test/src/align_up_overflow_panic.rs
cp tests/memory-types-frame-2m-unaligned-panic-test/src/main.rs tests/memory-types-test/src/frame_2m_unaligned_panic.rs
cp tests/memory-types-pa-overflow-panic-test/src/main.rs tests/memory-types-test/src/pa_overflow_panic.rs
cp tests/memory-types-va-canonical-panic-test/src/main.rs tests/memory-types-test/src/va_canonical_panic.rs
```

- [ ] **Step 2: 更新 memory-types-test/Cargo.toml，添加 `[[bin]]` 和新增依赖**

合并后的 Cargo.toml 需要包含所有 panic 测试的额外依赖（`config`、`arch`、`heapless`）：

```toml
[package]
name = "memory-types-test"
description = "memory_types 地址、帧/页号编解码及边界 panic 测试"
edition.workspace = true

[[bin]]
name = "memory-types-test"
path = "src/main.rs"
test = false

[[bin]]
name = "memory-types-align-up-overflow-panic-test"
path = "src/align_up_overflow_panic.rs"
test = false

[[bin]]
name = "memory-types-frame-2m-unaligned-panic-test"
path = "src/frame_2m_unaligned_panic.rs"
test = false

[[bin]]
name = "memory-types-pa-overflow-panic-test"
path = "src/pa_overflow_panic.rs"
test = false

[[bin]]
name = "memory-types-va-canonical-panic-test"
path = "src/va_canonical_panic.rs"
test = false

[dependencies]
test_harness = { path = "../test_harness" }
simplekernel = { path = "../..", default-features = false }
memory_types = { path = "../../crates/memory_types" }
config = { path = "../../crates/config" }
arch = { path = "../../crates/arch" }
log.workspace = true
heapless.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: 从 workspace members 中移除旧包路径**

编辑根 `Cargo.toml`，从 `[workspace] members` 中删除：
```
"tests/memory-types-align-up-overflow-panic-test",
"tests/memory-types-pa-overflow-panic-test",
"tests/memory-types-va-canonical-panic-test",
"tests/memory-types-frame-2m-unaligned-panic-test",
```

- [ ] **Step 4: 删除旧的测试目录**

```bash
rm -rf tests/memory-types-align-up-overflow-panic-test
rm -rf tests/memory-types-frame-2m-unaligned-panic-test
rm -rf tests/memory-types-pa-overflow-panic-test
rm -rf tests/memory-types-va-canonical-panic-test
```

- [ ] **Step 5: 验证编译**

```bash
cargo xtask build --arch riscv64
```

Expected: 编译成功，无错误。

- [ ] **Step 6: Commit**

```bash
git add -A tests/memory-types-test/ Cargo.toml
git commit --signoff -m "refactor(tests): 合并 memory-types 测试包（5→1），使用 [[bin]] 管理多二进制"
```

---

### Task 2: 合并 paging 测试包（6→1）

**Files:**
- Create: `tests/paging-test/Cargo.toml`
- Create: `tests/paging-test/src/basic.rs` (原 paging-basic-test/src/main.rs)
- Create: `tests/paging-test/src/table.rs` (原 paging-table-test/src/main.rs)
- Create: `tests/paging-test/src/mapping.rs` (原 paging-mapping-test/src/main.rs)
- Create: `tests/paging-test/src/conflict_panic.rs` (原 paging-conflict-panic-test/src/main.rs)
- Create: `tests/paging-test/src/equal_range_panic.rs` (原 paging-equal-range-panic-test/src/main.rs)
- Create: `tests/paging-test/src/reversed_range_panic.rs` (原 paging-reversed-range-panic-test/src/main.rs)
- Delete: `tests/paging-basic-test/` (entire directory)
- Delete: `tests/paging-table-test/` (entire directory)
- Delete: `tests/paging-mapping-test/` (entire directory)
- Delete: `tests/paging-conflict-panic-test/` (entire directory)
- Delete: `tests/paging-equal-range-panic-test/` (entire directory)
- Delete: `tests/paging-reversed-range-panic-test/` (entire directory)
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 创建 paging-test 目录并移入所有源文件**

```bash
mkdir -p tests/paging-test/src
cp tests/paging-basic-test/src/main.rs tests/paging-test/src/basic.rs
cp tests/paging-table-test/src/main.rs tests/paging-test/src/table.rs
cp tests/paging-mapping-test/src/main.rs tests/paging-test/src/mapping.rs
cp tests/paging-conflict-panic-test/src/main.rs tests/paging-test/src/conflict_panic.rs
cp tests/paging-equal-range-panic-test/src/main.rs tests/paging-test/src/equal_range_panic.rs
cp tests/paging-reversed-range-panic-test/src/main.rs tests/paging-test/src/reversed_range_panic.rs
```

注意：paging-test 没有传统 `main.rs`（所有二进制都指定了 `path`），这是合法的 Cargo 配置。

- [ ] **Step 2: 创建 paging-test/Cargo.toml**

合并所有 6 个包的依赖：

```toml
[package]
name = "paging-test"
description = "页表参数、map/unmap/映射生命周期及边界 panic 测试"
edition.workspace = true

[[bin]]
name = "paging-basic-test"
path = "src/basic.rs"
test = false

[[bin]]
name = "paging-table-test"
path = "src/table.rs"
test = false

[[bin]]
name = "paging-mapping-test"
path = "src/mapping.rs"
test = false

[[bin]]
name = "paging-conflict-panic-test"
path = "src/conflict_panic.rs"
test = false

[[bin]]
name = "paging-equal-range-panic-test"
path = "src/equal_range_panic.rs"
test = false

[[bin]]
name = "paging-reversed-range-panic-test"
path = "src/reversed_range_panic.rs"
test = false

[dependencies]
test_harness = { path = "../test_harness" }
simplekernel = { path = "../..", default-features = false }
paging = { path = "../../crates/paging" }
frame_allocator = { path = "../../crates/frame_allocator" }
memory_types = { path = "../../crates/memory_types" }
config = { path = "../../crates/config" }
log.workspace = true
heapless.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: 更新 workspace members**

编辑根 `Cargo.toml`，替换 6 个旧路径为 1 个新路径：

删除：
```
"tests/paging-basic-test",
"tests/paging-table-test",
"tests/paging-mapping-test",
"tests/paging-equal-range-panic-test",
"tests/paging-reversed-range-panic-test",
"tests/paging-conflict-panic-test",
```

添加：
```
"tests/paging-test",
```

- [ ] **Step 4: 删除旧目录**

```bash
rm -rf tests/paging-basic-test tests/paging-table-test tests/paging-mapping-test
rm -rf tests/paging-conflict-panic-test tests/paging-equal-range-panic-test tests/paging-reversed-range-panic-test
```

- [ ] **Step 5: 验证编译**

```bash
cargo xtask build --arch riscv64
```

Expected: 编译成功。

- [ ] **Step 6: Commit**

```bash
git add -A tests/paging-test/ Cargo.toml
git commit --signoff -m "refactor(tests): 合并 paging 测试包（6→1），使用 [[bin]] 管理多二进制"
```

---

### Task 3: 合并 sync 测试包（4→1）

**Files:**
- Create: `tests/sync-test/Cargo.toml`
- Create: `tests/sync-test/src/spinlock.rs` (原 sync-spinlock-test/src/main.rs)
- Create: `tests/sync-test/src/lockstack.rs` (原 sync-lockstack-test/src/main.rs)
- Create: `tests/sync-test/src/lockstack_pop_mismatch_panic.rs` (原 sync-lockstack-pop-mismatch-test/src/main.rs)
- Create: `tests/sync-test/src/recursive_lock_panic.rs` (原 sync-recursive-lock-test/src/main.rs)
- Delete: `tests/sync-spinlock-test/` (entire directory)
- Delete: `tests/sync-lockstack-test/` (entire directory)
- Delete: `tests/sync-lockstack-pop-mismatch-test/` (entire directory)
- Delete: `tests/sync-recursive-lock-test/` (entire directory)
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 创建 sync-test 目录并移入源文件**

```bash
mkdir -p tests/sync-test/src
cp tests/sync-spinlock-test/src/main.rs tests/sync-test/src/spinlock.rs
cp tests/sync-lockstack-test/src/main.rs tests/sync-test/src/lockstack.rs
cp tests/sync-lockstack-pop-mismatch-test/src/main.rs tests/sync-test/src/lockstack_pop_mismatch_panic.rs
cp tests/sync-recursive-lock-test/src/main.rs tests/sync-test/src/recursive_lock_panic.rs
```

- [ ] **Step 2: 创建 sync-test/Cargo.toml**

```toml
[package]
name = "sync-test"
description = "SpinLock、LockStack 及边界 panic 测试"
edition.workspace = true

[[bin]]
name = "sync-spinlock-test"
path = "src/spinlock.rs"
test = false

[[bin]]
name = "sync-lockstack-test"
path = "src/lockstack.rs"
test = false

[[bin]]
name = "sync-lockstack-pop-mismatch-test"
path = "src/lockstack_pop_mismatch_panic.rs"
test = false

[[bin]]
name = "sync-recursive-lock-test"
path = "src/recursive_lock_panic.rs"
test = false

[dependencies]
test_harness = { path = "../test_harness" }
simplekernel = { path = "../..", default-features = false }
sync = { path = "../../crates/sync" }
log.workspace = true
heapless.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: 更新 workspace members**

删除：
```
"tests/sync-spinlock-test",
"tests/sync-recursive-lock-test",
"tests/sync-lockstack-test",
"tests/sync-lockstack-pop-mismatch-test",
```

添加：
```
"tests/sync-test",
```

- [ ] **Step 4: 删除旧目录**

```bash
rm -rf tests/sync-spinlock-test tests/sync-lockstack-test
rm -rf tests/sync-lockstack-pop-mismatch-test tests/sync-recursive-lock-test
```

- [ ] **Step 5: 验证编译**

```bash
cargo xtask build --arch riscv64
```

- [ ] **Step 6: Commit**

```bash
git add -A tests/sync-test/ Cargo.toml
git commit --signoff -m "refactor(tests): 合并 sync 测试包（4→1），使用 [[bin]] 管理多二进制"
```

---

### Task 4: 合并 frame 测试包（2→1）

**Files:**
- Create: `tests/frame-test/Cargo.toml`
- Create: `tests/frame-test/src/alloc.rs` (原 frame-alloc-test/src/main.rs)
- Create: `tests/frame-test/src/mapped_drop_panic.rs` (原 frame-mapped-drop-test/src/main.rs)
- Delete: `tests/frame-alloc-test/` (entire directory)
- Delete: `tests/frame-mapped-drop-test/` (entire directory)
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: 创建 frame-test 目录并移入源文件**

```bash
mkdir -p tests/frame-test/src
cp tests/frame-alloc-test/src/main.rs tests/frame-test/src/alloc.rs
cp tests/frame-mapped-drop-test/src/main.rs tests/frame-test/src/mapped_drop_panic.rs
```

- [ ] **Step 2: 创建 frame-test/Cargo.toml**

```toml
[package]
name = "frame-test"
description = "帧分配器 typestate 生命周期及 MappedFrames drop panic 测试"
edition.workspace = true

[[bin]]
name = "frame-alloc-test"
path = "src/alloc.rs"
test = false

[[bin]]
name = "frame-mapped-drop-test"
path = "src/mapped_drop_panic.rs"
test = false

[dependencies]
test_harness = { path = "../test_harness" }
simplekernel = { path = "../..", default-features = false }
frame_allocator = { path = "../../crates/frame_allocator" }
memory_types = { path = "../../crates/memory_types" }
config = { path = "../../crates/config" }
log.workspace = true
heapless.workspace = true

[target.'cfg(target_arch = "riscv64")'.dependencies]
sbi-rt.workspace = true

[target.'cfg(target_arch = "aarch64")'.dependencies]
aarch64-cpu.workspace = true

[lints]
workspace = true
```

- [ ] **Step 3: 更新 workspace members**

删除：
```
"tests/frame-alloc-test",
"tests/frame-mapped-drop-test",
```

添加：
```
"tests/frame-test",
```

- [ ] **Step 4: 删除旧目录**

```bash
rm -rf tests/frame-alloc-test tests/frame-mapped-drop-test
```

- [ ] **Step 5: 验证编译**

```bash
cargo xtask build --arch riscv64
```

- [ ] **Step 6: Commit**

```bash
git add -A tests/frame-test/ Cargo.toml
git commit --signoff -m "refactor(tests): 合并 frame 测试包（2→1），使用 [[bin]] 管理多二进制"
```

---

### Task 5: 更新 xtask 测试发现逻辑（包→二进制）

**Files:**
- Modify: `xtask/src/test.rs`
- Modify: `xtask/src/build.rs`

xtask 当前逻辑：扫描 `tests/*/Cargo.toml`，取 `[package] name`，用 `-p name` 构建。
合并后一个包有多个二进制，需要改为：扫描 `[[bin]] name`，用 `-p pkg --bin name` 构建。

- [ ] **Step 1: 修改 `test_packages()` → `test_binaries()`（xtask/src/test.rs）**

将函数重命名为 `test_binaries`，返回 `Vec<(String, String)>`（`(package_name, binary_name)` 对）。扫描逻辑改为：对每个 `tests/*/Cargo.toml`，解析所有 `[[bin]] name` 条目（跳过 `test_harness`），将每个 `(package, bin)` 对推入结果。

```rust
/// 表示一个测试二进制——所属包名 + 二进制名。
pub struct TestBinary {
    pub package: String,
    pub bin_name: String,
}

/// 收集 `tests/` 下所有测试二进制（扫描 `[[bin]]` 条目，跳过库 crate）。
pub fn test_binaries(project_root: &Path) -> Vec<TestBinary> {
    let mut binaries = Vec::new();
    let tests_dir = project_root.join("tests");
    if !tests_dir.exists() {
        return binaries;
    }
    let Ok(entries) = std::fs::read_dir(&tests_dir) else {
        return binaries;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let cargo_toml = dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&cargo_toml) else {
            continue;
        };
        let Some(pkg_name) = parse_package_name(&content) else {
            continue;
        };
        // 跳过库 crate（test_harness）
        if !has_bin_section(&content) {
            continue;
        }
        for bin_name in parse_bin_names(&content) {
            binaries.push(TestBinary {
                package: pkg_name.clone(),
                bin_name,
            });
        }
    }
    binaries
}

/// 从 Cargo.toml 内容中解析 [package] name
fn parse_package_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name") && !trimmed.starts_with("# ") {
            if let Some(start) = trimmed.find('"') {
                let rest = &trimmed[start + 1..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }
    None
}

/// 检查 Cargo.toml 是否有 [[bin]] 节
fn has_bin_section(content: &str) -> bool {
    content.lines().any(|l| l.trim() == "[[bin]]")
}

/// 从 Cargo.toml 中解析所有 [[bin]] 的 name 字段。
///
/// 简单状态机：遇到 `[[bin]]` 后，下一个 `name = "xxx"` 行即该 bin 的名字。
fn parse_bin_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_bin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bin]]" {
            in_bin = true;
            continue;
        }
        if in_bin && trimmed.starts_with("name") {
            if let Some(start) = trimmed.find('"') {
                let rest = &trimmed[start + 1..];
                if let Some(end) = rest.find('"') {
                    names.push(rest[..end].to_string());
                    in_bin = false;
                }
            }
        }
        // 遇到其他 section 头时退出 [[bin]] 状态
        if trimmed.starts_with('[') && trimmed != "[[bin]]" {
            in_bin = false;
        }
    }
    names
}
```

注意：`parse_package_name` 有一个陷阱——它可能匹配到 `[[bin]]` 节下的 `name` 而非 `[package]` 下的。需要确保它只解析 `[package]` 节下的第一个 `name`。实现中可以在遇到第一个 `name` 后立即返回（当前的 `read_package_name` 就是这样做的），但前提是 `[package]` 在 `[[bin]]` 之前——这在所有现有 Cargo.toml 中都成立。保持原有的 `read_package_name` 函数不变，只新增 `parse_bin_names`。

- [ ] **Step 2: 修改 `build_binary()` 支持 `--bin` 参数（xtask/src/build.rs）**

当前 `build_binary` 接受 `package: Option<&str>`，ELF 路径用 `package` 推断。

改为接受 `package: Option<&str>` 和 `bin_name: Option<&str>`：

```rust
pub fn build_binary(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: Option<&str>,
    bin_name: Option<&str>,
    release: bool,
) -> Result<PathBuf> {
    let label = bin_name.or(package).unwrap_or("kernel");
    println!("[xtask] Building '{}' for {}...", label, arch.as_str());
    let target = arch.target_triple();
    let mut build_cmd = cmd!(sh, "cargo build {BUILD_STD_ARGS...} --target {target}");
    if let Some(pkg) = package {
        build_cmd = build_cmd.args(["-p", pkg]);
    }
    if let Some(bin) = bin_name {
        build_cmd = build_cmd.args(["--bin", bin]);
    }
    if release {
        build_cmd = build_cmd.arg("--release");
    }
    build_cmd.run()?;

    let profile_dir = if release { "release" } else { "debug" };
    let binary_name = bin_name.or(package).unwrap_or("simplekernel");
    let elf_path = project_root
        .join("target")
        .join(target)
        .join(profile_dir)
        .join(binary_name);
    if !elf_path.exists() {
        return Err(format!("ELF not found at {}", elf_path.display()).into());
    }
    Ok(elf_path)
}
```

- [ ] **Step 3: 更新所有 `build_binary` 调用点**

搜索 `xtask/src/` 中所有调用 `build::build_binary` 的位置，添加 `None` 作为新参数 `bin_name`（非测试场景）。

在 `test.rs` 中的测试调用改为传递 `Some(package)` 和 `Some(bin_name)`。

`run_test` 函数签名改为接受 `TestBinary`（或 `package` + `bin_name`）：

```rust
pub fn run_test(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    bin_name: &str,
    env: &QemuEnv,
    release: bool,
) -> Result<bool> {
    let kernel_elf_path = build::build_binary(sh, project_root, arch, Some(package), Some(bin_name), release)?;
    // ... 其余不变，但日志用 bin_name ...
```

`build_test_with_fit` 同理：

```rust
fn build_test_with_fit(
    sh: &Shell,
    project_root: &Path,
    arch: Arch,
    package: &str,
    bin_name: &str,
    env: &QemuEnv,
    release: bool,
) -> Result<(PathBuf, PathBuf)> {
    let kernel_elf_path = build::build_binary(sh, project_root, arch, Some(package), Some(bin_name), release)?;
    // ...
    let test_boot_dir = per_test_boot_dir(&env.boot_dir, bin_name)?;
    // ...
```

`run_all_tests` 改为调用 `test_binaries()` 并遍历 `TestBinary`：

```rust
pub fn run_all_tests(...) -> Result<bool> {
    let bins = test_binaries(project_root);
    // ...
    for tb in &bins {
        let (elf, boot_dir) = build_test_with_fit(sh, project_root, arch, &tb.package, &tb.bin_name, env, release)?;
        prepared.push((tb.bin_name.clone(), elf, boot_dir));
    }
    // ...
```

`list_tests` 改为显示二进制名：

```rust
pub fn list_tests(project_root: &Path) {
    println!("Available tests:");
    for tb in test_binaries(project_root) {
        println!("  {}", tb.bin_name);
    }
}
```

- [ ] **Step 4: 更新 main.rs 中 `--name` 参数处理**

`--name` 现在传递的是二进制名，需要通过 `test_binaries()` 查找对应的 `(package, bin_name)` 对：

在 `xtask/src/main.rs` 中（约 152 行）：

```rust
if let Some(name) = &args.name {
    // 查找二进制名对应的包
    let bins = test::test_binaries(&project_root);
    let tb = bins.iter().find(|b| b.bin_name == *name)
        .unwrap_or_else(|| {
            eprintln!("[xtask] Unknown test binary: '{name}'");
            eprintln!("[xtask] Use --list to see available tests.");
            std::process::exit(1);
        });
    test::run_test(&sh, &project_root, args.arch, &tb.package, &tb.bin_name, &qemu_env, args.release)?;
```

- [ ] **Step 5: 更新 `build.rs` 中非测试调用点（kernel build/run/debug）**

搜索 `xtask/src/` 中 `build::build_binary(` 的所有调用，在 `package` 参数后加 `None`（对内核构建而言不需要 `--bin`）。这些调用通常在 `xtask/src/main.rs` 的 run/build/debug 分支中。

- [ ] **Step 6: 验证编译和列表**

```bash
cd xtask && cargo build && cd ..
cargo xtask test --list
```

Expected: 列出所有 22 个二进制名（与之前相同），包括所有 should_panic 和普通测试。

- [ ] **Step 7: Commit**

```bash
git add xtask/src/test.rs xtask/src/build.rs xtask/src/main.rs
git commit --signoff -m "refactor(xtask): 测试发现改为扫描 [[bin]]，支持多二进制包"
```

---

### Task 6: 更新 CLAUDE.md 测试文档

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: 更新 STRUCTURE 和 CODE MAP 中的测试描述**

在 CLAUDE.md 的 STRUCTURE 节中，`tests/` 描述不需要改动（已是通用描述）。

在 CODE MAP 中，确认 `tests/*/` 行仍然准确。

- [ ] **Step 2: 更新 TESTING 节中"添加独立测试"步骤**

当前步骤说"创建 `tests/my-test/`"，需要补充：如果测试属于现有模块，应在现有包中添加 `[[bin]]` 条目而非创建新包。

替换 `#### 添加独立测试` 部分为：

```markdown
#### 添加独立测试

**在已有模块包中添加（推荐）：**
1. 在对应包的 `src/` 下创建新文件（如 `tests/paging-test/src/my_new_test.rs`）
2. 在该包的 `Cargo.toml` 中添加 `[[bin]]` 条目，指定 `name` 和 `path`，设置 `test = false`
3. should_panic 测试使用 `test_main!(level, fn, should_panic)` 变体
4. xtask 自动扫描 `[[bin]]` 条目发现新测试

**创建新模块包：**
1. 创建 `tests/my-test/`，包含 `Cargo.toml`（至少一个 `[[bin]]` 条目）和对应源文件
2. 在根 `Cargo.toml` 的 `[workspace] members` 中添加路径
3. xtask 自动扫描 `tests/*/Cargo.toml` 中的 `[[bin]]` 条目发现新测试
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit --signoff -m "docs: 更新 CLAUDE.md 测试文档，反映多二进制包结构"
```

---

### Task 7: 端到端验证

- [ ] **Step 1: 验证测试列表完整**

```bash
cargo xtask test --list
```

Expected: 22 个测试二进制名全部列出，名字与合并前完全一致。

- [ ] **Step 2: 运行全量测试（riscv64）**

```bash
cargo xtask test --arch riscv64
```

Expected: 全部通过。

- [ ] **Step 3: 运行全量测试（aarch64）**

```bash
cargo xtask test --arch aarch64
```

Expected: 全部通过。

- [ ] **Step 4: 验证 `--name` 筛选**

```bash
cargo xtask test --arch riscv64 --name memory-types-test
cargo xtask test --arch riscv64 --name memory-types-align-up-overflow-panic-test
cargo xtask test --arch riscv64 --name sync-spinlock-test
cargo xtask test --arch riscv64 --name paging-table-test
```

Expected: 每个指定测试单独运行并通过。

- [ ] **Step 5: 验证 clippy 和 fmt**

```bash
cargo fmt --check
cargo clippy -- -D warnings
```
