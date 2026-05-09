<!-- Copyright The SimpleKernel Contributors -->

# R1 原语层审计修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实施 R1 审计报告中所有已确认的修复：`span` trait bound 优化 + `ExactSizeIterator`、`memory_types` 地址算术校验 + 方法式转换、`page_frame.rs` lint 修复、`build_common` rerun-if-changed 修复、README 补全。

**Architecture:** 自底向上——先修 `span`（零依赖），再修 `memory_types`（依赖 span），再修 `build_common`（独立），最后更新下游调用方和文档。每个 Task 产出一个可编译可测试的 commit。

**Tech Stack:** Rust nightly, `#![no_std]`, edition 2024, `cargo test`（宿主机单元测试）

**Conventions:** 所有 `git commit` 必须带 `--signoff`。`#[allow(...)]` 一律改为 `#[expect(..., reason = "...")]`。

---

### Task 1: `span` — 放宽 `split_at`/`contiguous_with` 的 trait bound

**Files:**
- Modify: `crates/span/src/lib.rs`

当前 `split_at` 和 `contiguous_with` 位于 `impl<A: Copy + Ord + Add<usize, Output = A>>` 块中，但它们不使用 `Add`。将它们移到仅需 `Copy + Ord` 的 impl 块。

- [ ] **Step 1: 将 `split_at` 和 `contiguous_with` 移到 `Copy + Ord` 块**

将 `crates/span/src/lib.rs` 中 `impl<A: Copy + Ord + core::ops::Add<usize, Output = A>> Span<A>` 块内的 `split_at` 和 `contiguous_with` 方法，移到 `impl<A: Copy + Ord> Span<A>` 块的末尾（`is_empty` 之后）。

注意：`split_at` 的签名中 `mid: A` 只用了 `Ord` 比较，不需要 `Add`。

移动后 `impl<A: Copy + Ord + Add<usize, Output = A>>` 块中仅剩 `merge` 和 `iter`。

- [ ] **Step 2: 运行测试验证**

```bash
cargo test -p span
```

Expected: 全部 PASS（9 个测试）。

- [ ] **Step 3: Commit**

```bash
git add crates/span/src/lib.rs
git commit --signoff -m "refactor(span): 放宽 split_at/contiguous_with 的 trait bound 至 Copy + Ord"
```

---

### Task 2: `span` — 为 `SpanIter` 实现 `size_hint` 和 `ExactSizeIterator`

**Files:**
- Modify: `crates/span/src/lib.rs`

- [ ] **Step 1: 为测试用 `Val` 类型实现 `Sub` trait（已有），添加新测试**

在 `crates/span/src/lib.rs` 的 `#[cfg(test)] mod tests` 末尾添加：

```rust
    #[test]
    fn iter_exact_size() {
        let span = Span::new(Val(10), Val(15));
        let iter = span.iter();
        assert_eq!(iter.len(), 5);
        assert_eq!(iter.size_hint(), (5, Some(5)));
    }

    #[test]
    fn iter_exact_size_empty() {
        let span = Span::new(Val(3), Val(3));
        let iter = span.iter();
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p span iter_exact_size
```

Expected: FAIL — `SpanIter` 未实现 `ExactSizeIterator`，`len()` 方法不存在。

- [ ] **Step 3: 为 `SpanIter` 添加 `size_hint` 和 `ExactSizeIterator`**

在 `crates/span/src/lib.rs` 中，将现有的 `Iterator for SpanIter` impl 替换为（注意增加 `Sub` bound）：

```rust
impl<A: Copy + Ord + core::ops::Add<usize, Output = A> + core::ops::Sub<A, Output = usize>>
    Iterator for SpanIter<A>
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let item = self.current;
            self.current = self.current + 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.current;
        (remaining, Some(remaining))
    }
}

impl<A: Copy + Ord + core::ops::Add<usize, Output = A> + core::ops::Sub<A, Output = usize>>
    ExactSizeIterator for SpanIter<A>
{
}
```

同时需要更新 `iter()` 方法的 impl 块 bound，因为 `SpanIter` 的 `Iterator` 现在需要 `Sub`。`iter()` 目前在 `Add<usize, Output = A>` 块中，需要加上 `Sub<A, Output = usize>`：

```rust
impl<A: Copy + Ord + core::ops::Add<usize, Output = A> + core::ops::Sub<A, Output = usize>>
    Span<A>
{
    /// 逐元素迭代（步长为 1）。
    #[inline]
    pub fn iter(self) -> SpanIter<A> {
        SpanIter {
            current: self.start,
            end: self.end,
        }
    }
}
```

注意 `merge` 方法不需要 `Sub`，所以将 `merge` 保留在原来的 `Add<usize>` 块中，`iter` 移到新的 `Add + Sub` 块中。

- [ ] **Step 4: 运行测试验证通过**

```bash
cargo test -p span
```

Expected: 全部 PASS（11 个测试）。

- [ ] **Step 5: 验证下游编译**

```bash
cargo check -p memory_types
```

Expected: 编译通过（`memory_types` 的 `Frame`/`Page` 已实现 `Sub`）。

- [ ] **Step 6: Commit**

```bash
git add crates/span/src/lib.rs
git commit --signoff -m "feat(span): 为 SpanIter 实现 ExactSizeIterator + size_hint"
```

---

### Task 3: `memory_types` — 地址算术运算添加有效性校验

**Files:**
- Modify: `crates/memory_types/src/lib.rs`

当前 `PhysAddr + usize` 和 `VirtAddr + usize` 使用 `Self(checked_result)` 绕过了构造时的范围/规范化校验。改为 `Self::new(checked_result)` 以确保结果始终有效。

- [ ] **Step 1: 添加测试——验证算术结果超出范围时 panic**

在 `crates/memory_types/src/addr.rs` 的 `#[cfg(test)] mod tests` 末尾添加：

```rust
    /// PhysAddr 加法结果超出 PA_BITS 范围应 panic。
    #[test]
    #[should_panic(expected = "PA_BITS")]
    fn phys_add_overflow_pa_bits() {
        // PA_BITS 在测试环境中为 48，构造一个接近上限的地址
        let near_max = PhysAddr::new((1usize << config::PA_BITS) - 2);
        let _ = near_max + 4; // 结果超出 PA_BITS
    }

    /// VirtAddr 加法结果不满足规范化应 panic。
    #[test]
    #[should_panic(expected = "非规范虚拟地址")]
    fn virt_add_breaks_canonical() {
        // VA_BITS 在测试环境中为 39，构造一个接近规范边界的低段地址
        let near_boundary = VirtAddr::new((1usize << (config::VA_BITS - 1)) - 2);
        let _ = near_boundary + 4; // 结果跨入 canonical hole
    }
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p memory_types phys_add_overflow_pa_bits -- --nocapture
cargo test -p memory_types virt_add_breaks_canonical -- --nocapture
```

Expected: FAIL — 当前 `Add` impl 不校验范围，不会 panic。

- [ ] **Step 3: 修改 `@common` 宏中的算术运算**

在 `crates/memory_types/src/lib.rs` 的 `impl_usize_newtype!(@common $name)` 中，将所有 `Self(self.0.checked_add/sub(...).expect(...))` 改为 `Self::new(self.0.checked_add/sub(...).expect(...))`。

具体修改 4 处：

`Add<usize>`:
```rust
Self::new(self.0.checked_add(rhs).expect(concat!(stringify!($name), ": 加法溢出")))
```

`AddAssign<usize>`:
```rust
self.0 = Self::new(self.0.checked_add(rhs).expect(concat!(stringify!($name), ": 加法溢出"))).0;
```

注意 `AddAssign` 中 `self.0 = ...` 需要取 `.0` 因为 `Self::new()` 返回的是 `Self`。改为：
```rust
*self = Self::new(self.0.checked_add(rhs).expect(concat!(stringify!($name), ": 加法溢出")));
```

`Sub<usize>`:
```rust
Self::new(self.0.checked_sub(rhs).expect(concat!(stringify!($name), ": 减法下溢")))
```

`SubAssign<usize>`:
```rust
*self = Self::new(self.0.checked_sub(rhs).expect(concat!(stringify!($name), ": 减法下溢")));
```

不修改 `Sub<$name>` —— 它返回 `usize`，不需要校验。

- [ ] **Step 4: 运行全部测试**

```bash
cargo test -p memory_types
```

Expected: 全部 PASS。新增的两个 `should_panic` 测试通过。

- [ ] **Step 5: Commit**

```bash
git add crates/memory_types/src/lib.rs crates/memory_types/src/addr.rs
git commit --signoff -m "fix(memory_types): 地址算术运算后校验结果在有效范围内"
```

---

### Task 4: `memory_types` — `phys_to_virt`/`virt_to_phys` 转为方法式 + 启用校验

**Files:**
- Modify: `crates/memory_types/src/addr.rs`
- Modify: `crates/memory_types/src/lib.rs`（更新 re-export）
- Modify: `crates/memory/src/lib.rs`（更新 re-export）
- Modify: `src/device/hal.rs`（更新调用方）
- Modify: `crates/frame_allocator/src/transitions.rs`（更新调用方）
- Modify: `crates/memory_types/README.md`（更新文档）

将自由函数 `phys_to_virt(pa)` / `virt_to_phys(va)` 改为方法 `pa.to_virt()` / `va.to_phys()`，同时使用 `Self::new()` 替代直接构造以启用校验。

- [ ] **Step 1: 修改 `addr.rs` — 添加方法，保留旧函数为 deprecated**

在 `crates/memory_types/src/addr.rs` 中：

1) 在 `impl PhysAddr` 块（`page_number` 方法所在的那个）中添加：

```rust
    /// 转换为虚拟地址（SAS identity mapping，VA == PA）。
    ///
    /// 仅适用于线性映射区域，非线性映射地址须通过页表查询。
    ///
    /// # Panics
    /// 结果不在规范虚拟地址范围内时 panic。
    #[inline]
    pub fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.as_usize())
    }
```

2) 在 `impl VirtAddr` 块（`as_ptr`/`as_mut_ptr` 所在的那个）中添加：

```rust
    /// 转换为物理地址（SAS identity mapping，VA == PA）。
    ///
    /// 仅适用于线性映射区域，非线性映射地址须通过页表查询。
    ///
    /// # Panics
    /// 结果不在物理地址有效范围内时 panic。
    #[inline]
    pub fn to_phys(self) -> PhysAddr {
        PhysAddr::new(self.as_usize())
    }
```

3) 删除旧的自由函数 `pub fn phys_to_virt` 和 `pub fn virt_to_phys`（连同它们上方的文档注释）。

- [ ] **Step 2: 更新 `lib.rs` re-export**

在 `crates/memory_types/src/lib.rs` 中，将：
```rust
pub use addr::{PhysAddr, VirtAddr, phys_to_virt, virt_to_phys};
```
改为：
```rust
pub use addr::{PhysAddr, VirtAddr};
```

- [ ] **Step 3: 更新测试**

在 `crates/memory_types/src/addr.rs` 的测试中，将所有 `phys_to_virt(pa)` 改为 `pa.to_virt()`，`virt_to_phys(va)` 改为 `va.to_phys()`。

具体修改 `phys_virt_roundtrip` 和 `phys_virt_zero` 两个测试：

```rust
    /// to_virt / to_phys 互逆：任意物理地址经往返转换后应恢复原值。
    #[test]
    fn phys_virt_roundtrip() {
        let pa = PhysAddr::new(0x8020_0000);
        let va = pa.to_virt();
        assert_eq!(va.as_usize(), pa.as_usize());
        assert_eq!(va.to_phys(), pa);
    }

    /// 零地址的物理-虚拟转换。
    #[test]
    fn phys_virt_zero() {
        let pa = PhysAddr::new(0);
        let va = pa.to_virt();
        assert_eq!(va.as_usize(), 0);
        assert_eq!(va.to_phys(), pa);
    }
```

- [ ] **Step 4: 运行 memory_types 测试**

```bash
cargo test -p memory_types
```

Expected: 全部 PASS。

- [ ] **Step 5: 更新下游调用方 — `crates/memory/src/lib.rs`**

删除 re-export 行：
```rust
pub use memory_types::{phys_to_virt, virt_to_phys};
```

- [ ] **Step 6: 更新下游调用方 — `crates/frame_allocator/src/transitions.rs`**

将：
```rust
let ptr = memory_types::phys_to_virt(free.start_paddr()).as_mut_ptr::<u8>();
```
改为：
```rust
let ptr = free.start_paddr().to_virt().as_mut_ptr::<u8>();
```

- [ ] **Step 7: 更新下游调用方 — `src/device/hal.rs`**

将所有 `memory_types::phys_to_virt(...)` 改为 `(...).to_virt()`，`memory_types::virt_to_phys(...)` 改为 `(...).to_phys()`。

具体三处：

1) `share` 函数中：
```rust
let vaddr = memory_types::phys_to_virt(paddr);
```
改为：
```rust
let vaddr = paddr.to_virt();
```

2) `mmio_phys_to_virt` 函数中：
```rust
let vaddr = memory_types::phys_to_virt(PhysAddr::new(paddr as usize));
```
改为：
```rust
let vaddr = PhysAddr::new(paddr as usize).to_virt();
```

3) `unshare` 函数中：
```rust
memory_types::virt_to_phys(vaddr).as_usize() as u64
```
改为：
```rust
vaddr.to_phys().as_usize() as u64
```

- [ ] **Step 8: 验证编译**

```bash
cargo check -p memory_types -p frame_allocator -p memory -p simplekernel --target targets/riscv64-none.json
```

Expected: 编译通过。如果有其他编译错误，根据错误信息修复遗漏的调用方。

- [ ] **Step 9: 运行单元测试**

```bash
cargo test -p memory_types
```

Expected: 全部 PASS。

- [ ] **Step 10: Commit**

```bash
git add crates/memory_types/src/addr.rs crates/memory_types/src/lib.rs \
       crates/memory/src/lib.rs crates/frame_allocator/src/transitions.rs \
       src/device/hal.rs
git commit --signoff -m "refactor(memory_types): phys_to_virt/virt_to_phys 改为方法式 + 启用校验"
```

---

### Task 5: `memory_types/page_frame.rs` — `#[allow]` 改为 `#[expect]`

**Files:**
- Modify: `crates/memory_types/src/page_frame.rs`

- [ ] **Step 1: 替换所有 `#[allow(clippy::...)]` 为 `#[expect]`**

在 `crates/memory_types/src/page_frame.rs` 中，将以下 4 处 `#[allow]`（宏展开后生成 8 处）：

```rust
#[allow(clippy::suspicious_arithmetic_impl)]
```
改为：
```rust
#[expect(clippy::suspicious_arithmetic_impl, reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug")]
```

以及：
```rust
#[allow(clippy::suspicious_op_assign_impl)]
```
改为：
```rust
#[expect(clippy::suspicious_op_assign_impl, reason = "页号算术按 NUM_4K_PAGES_SHIFT 位移缩放，非 bug")]
```

共 5 处（`Add`、`AddAssign`、`Sub`、`SubAssign`、`Sub<$name>` 中的前 4 个有 allow，第 5 个 `Sub<$name>` 也有一个）。

- [ ] **Step 2: 运行测试 + clippy**

```bash
cargo test -p memory_types
cargo clippy -p memory_types -- -D warnings
```

Expected: 全部 PASS，无 clippy 警告。

- [ ] **Step 3: Commit**

```bash
git add crates/memory_types/src/page_frame.rs
git commit --signoff -m "refactor(memory_types): #[allow(clippy::...)] 改为 #[expect] 并附注原因"
```

---

### Task 6: `build_common` — 修复 `rerun-if-changed` 粒度

**Files:**
- Modify: `crates/build_common/src/lib.rs`

- [ ] **Step 1: 为每个发现的 .S 文件和 link.ld 添加 rerun-if-changed**

在 `crates/build_common/src/lib.rs` 的 `setup_kernel_build` 函数中，在发现 `.S` 文件的循环内添加：

```rust
    if path.extension().is_some_and(|ext| ext == "S") {
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(&path);
        has_asm = true;
    }
```

在循环之后、链接器参数之前，添加对 `link.ld` 的监视：

```rust
    let linker_script = arch_dir.join("link.ld");
    println!("cargo:rerun-if-changed={}", linker_script.display());
```

保留原有的目录级 `rerun-if-changed`（作为兜底，捕获新增文件）。

- [ ] **Step 2: 验证编译**

```bash
cargo check -p build_common
```

Expected: 编译通过。

- [ ] **Step 3: Commit**

```bash
git add crates/build_common/src/lib.rs
git commit --signoff -m "fix(build_common): 补充逐文件 rerun-if-changed 确保增量编译正确"
```

---

### Task 7: `span` README

**Files:**
- Create: `crates/span/README.md`

- [ ] **Step 1: 创建 README**

按模板创建 `crates/span/README.md`，内容覆盖：
- 所属层级：R1 原语层
- 依赖方向：无依赖；被 `memory_types` 依赖
- 架构设计：零依赖泛型半开区间，可独立发布
- 公共 API 表：`Span<A>`、`SpanIter<A>`
- 设计决策：`merge` 仅支持相邻区间（内核分配器语义）
- 测试覆盖情况

- [ ] **Step 2: Commit**

```bash
git add crates/span/README.md
git commit --signoff -m "docs(span): 添加 README"
```

---

### Task 8: `build_common` README

**Files:**
- Create: `crates/build_common/README.md`

- [ ] **Step 1: 创建 README**

按模板创建 `crates/build_common/README.md`，内容覆盖：
- 所属层级：R1 原语层（构建基础设施）
- 依赖方向：依赖 `cc`；被根 crate `build.rs` 和测试 crate 的 `build.rs` 使用
- 架构设计：抽取汇编编译 + 链接器参数设置的公共逻辑
- 公共 API：`setup_kernel_build(arch_dir: &Path)`
- 支持的架构：riscv64、aarch64

- [ ] **Step 2: Commit**

```bash
git add crates/build_common/README.md
git commit --signoff -m "docs(build_common): 添加 README"
```

---

### Task 9: 更新 `memory_types` README

**Files:**
- Modify: `crates/memory_types/README.md`

- [ ] **Step 1: 更新 README 中的函数调用示例和说明**

将 README 中所有 `phys_to_virt(pa)` 改为 `pa.to_virt()`，`virt_to_phys(va)` 改为 `va.to_phys()`。更新以下区域：

1) "概览" 段落中关于 `phys_to_virt()` / `virt_to_phys()` 的描述
2) "核心类型" 段落（如有提及）
3) "类型关系" 图中的标注
4) "使用示例" 中的代码示例
5) "注意事项" 第 3 节

同时从 "核心类型" 代码块中移除自由函数的提及，改为方法式说明。

- [ ] **Step 2: Commit**

```bash
git add crates/memory_types/README.md
git commit --signoff -m "docs(memory_types): 更新 README 反映方法式地址转换 API"
```

---

### Task 10: 更新 `config` README

**Files:**
- Modify: `crates/config/README.md`

- [ ] **Step 1: 更新 identity mapping 说明中的函数引用**

将 `phys_to_virt()` / `virt_to_phys()` 改为 `PhysAddr::to_virt()` / `VirtAddr::to_phys()`。

- [ ] **Step 2: Commit**

```bash
git add crates/config/README.md
git commit --signoff -m "docs(config): 更新 identity mapping 说明中的 API 引用"
```

---

### Task 11: 更新审计进度

**Files:**
- Modify: `docs/audit/audit-progress.md`

- [ ] **Step 1: 更新审计进度文件**

更新 `docs/audit/audit-progress.md`：
- "当前 Phase" 改为 "R1 — 原语层（实施中）"
- "下一个目标" 改为 "R2 — 同步与 Per-CPU"
- 更新"上次对话摘要"记录 R1 的审查和修复内容
- 在"已完成的目标"表中追加 R1 记录

- [ ] **Step 2: Commit**

```bash
git add docs/audit/audit-progress.md
git commit --signoff -m "docs(audit): 更新 R1 审计进度"
```
