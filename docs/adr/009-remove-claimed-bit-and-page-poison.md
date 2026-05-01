# ADR-009: 删除 CLAIMED 软件位，保留 page poison

> **状态**: 已接受（部分由 [ADR-013](013-ownedpages-necessity.md) 后续取代）
>
> **日期**: 2026-04-17
>
> **审计阶段**: R3 — 内存子系统（回看）
>
> **涉及模块**: `crates/page_table_entry`, `crates/paging`, `crates/config`

## 背景

commit 7d788053e 引入了 PTE CLAIMED 软件保留位 + `FREED_PAGE_POISON` 双重机制，声称与编译期 typestate 互补提供"纵深防御"（defense in depth），检测 `unsafe` 代码或分配器 bug 导致的双重分配和 use-after-free。

当前实现：

```rust
// crates/paging/src/mapping.rs
impl OwnedPages {
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        let va_start = frames.start_paddr().to_virt();
        claim_pages(va_start, page_count, flags);  // 设置 CLAIMED 位 + 检查双重分配
        Self { frames, flags }
    }
}

impl Drop for OwnedPages {
    fn drop(&mut self) {
        batch_update_flags(va_start, page_count, PteFlags::kernel_rw());  // 清 CLAIMED
        unsafe {
            core::ptr::write_bytes(                                         // poison 填充
                va_start.as_mut_ptr::<u8>(),
                config::FREED_PAGE_POISON,  // = 0xFE
                page_count * PAGE_SIZE,
            );
        }
    }
}
```

CLAIMED 位分别占用 RISC-V RSW[0]（bit 8）和 AArch64 bit 55。

## 威胁模型重审

重新审视这套机制**实际能防住什么**：

### 场景 1：`AllocatedFrames` 被双重持有

- **Rust 类型系统已在编译期阻止**——`AllocatedFrames` 不实现 `Clone`/`Copy`，move 语义保证唯一所有权
- CLAIMED 检测在此场景**没有独立价值**：要让 CLAIMED 触发必须先绕过类型系统

### 场景 2：`unsafe` 代码绕过类型系统

假设某处 `unsafe` 代码通过 `AllocatedFrames { range }` 手动构造（绕过 `alloc_from_backend`），然后调用 `OwnedPages::new`。这段 `unsafe` 代码同样可以：

- 直接构造裸指针 `paddr.as_usize() as *mut u8`，写入数据，绕过 CLAIMED + OwnedPages
- 直接操作 `kernel_page_table().lock()`，修改 PTE，绕过 CLAIMED 检测

**CLAIMED 防御依赖"违规者诚实走 OwnedPages::new"**——这对已经拥有 unsafe 能力的代码不成立。真正的防御在于 **限制谁能写 unsafe**（APP `#![forbid(unsafe_code)]` + crate 可见性），而非运行时 bit 检查。

### 场景 3：分配器 bug 导致同一帧被双重分配

- `buddy_system_allocator` 是有成熟测试的外部 crate，内部一致性由其维护
- 如果 buddy bug 真的发生，CLAIMED 检测确实能捕获——但这是检测**第三方 crate 的 bug**，成本/收益不成立：
  - 成本：两架构 PTE 位 + Drop 路径 PAGE_SIZE × n 字节 poison 写入（每次 OwnedPages drop）
  - 收益：捕获第三方 crate 的 bug（应当由其自身单测/模糊测试覆盖）

### 场景 4：use-after-free 检测（poison 写入）

poison 的逻辑是：Drop 后读到 0xFE 填充模式 → 知道访问了已释放页面。但：

- **Rust ownership 已阻止合法 use-after-free**——访问已 drop 的 `OwnedPages` 是编译错误
- 依赖 poison 检测的必定是 `unsafe` 代码通过 identity mapping 裸指针访问——这类代码**读到 0xFE 后依然会继续访问**（因为它本来就不经过 OwnedPages 界面）
- 真正能通过 poison 发现的是 "使用 raw pointer 访问后结果异常" 的调试场景——此时 debug logger 会看到 0xFEFEFEFE 模式而惊觉，但这是**调试辅助**而非**安全防御**

## 量化成本

### CLAIMED 位成本（本 ADR 删除）

| 项 | 代码量/运行时成本 |
|---|------------------|
| `PteFlagsOps::is_claimed` + `with_claimed` 两个 trait 方法 | 4 行 trait 声明 |
| RISC-V `PteFlags::CLAIMED` 定义 + 两个方法实现 | 17 行 |
| AArch64 `PteFlags::CLAIMED` 定义 + 两个方法实现 | 17 行 |
| `claim_pages` 函数（`paging/mapping.rs`） | 18 行 |
| `OwnedPages::set_flags` 保留 CLAIMED 位的额外逻辑 | 3 行 |
| `tests/paging-test/src/double_claim_panic.rs` | 34 行 |
| `tests/paging-test/Cargo.toml` 中 `[[bin]]` 条目 | 5 行 |
| **合计删除** | **~98 行 + 两架构各一个 PTE 软件位** |

**PTE 软件位的机会成本**（删除后释放）：
- RISC-V RSW[0] / AArch64 bit 55 释放后，未来可用于：
  - COW（copy-on-write）标记
  - swap-entry 编码（换出到磁盘的页）
  - ASID 元数据等

### Poison 成本（本 ADR 保留）

| 项 | 代码量/运行时成本 |
|---|------------------|
| `OwnedPages::Drop` 的 poison `write_bytes` 调用 | 8 行（含 SAFETY 注释） |
| `FREED_PAGE_POISON` 配置常量 | 2 行 |
| **合计保留** | **~10 行** |

**Drop 路径运行时成本**（保留，可接受）：每次 `OwnedPages::drop` 执行 `write_bytes(0xFE, PAGE_SIZE × n)`：
- 4 页 RAII buffer：16 KB memory write
- 64 页 VM stack：256 KB memory write
- 当前阶段（无频繁帧回收）可忽略；如果未来 benchmark 证明是瓶颈，可改为 `#[cfg(debug_assertions)]` 条件编译

## 关于"纵深防御"的判断

纵深防御的原则是 **"多层独立防御，任一层失效时其他层仍生效"**。CLAIMED + poison 相对编译期类型系统不是独立层次：

- 触发 CLAIMED 检测的路径必定经过已被类型系统覆盖的入口（`OwnedPages::new`）
- 绕过类型系统（写 unsafe）的代码同样可绕过 CLAIMED 检测
- 两者是**同一防御边界的两面**，不是纵深

真正的纵深（语言安全 + 硬件权限）在 SimpleKernel 已经有：
- 层 1：APP crate `#![forbid(unsafe_code)]` + crate 可见性
- 层 2：NX / RO / guard-page 等硬件权限位（由 OwnedPages 管理的 `PteFlags`）
- 层 3：panic observer（内核 bug 时 fail-fast）

CLAIMED + poison 不在这三层之内，属于"在已防线之内又加一道用处不大的墙"。

## 备选方案

### 方案 A: 删除 CLAIMED 位，保留 page poison

删除清单（CLAIMED 相关）：

```
crates/page_table_entry/src/lib.rs       PteFlagsOps::is_claimed / with_claimed
crates/page_table_entry/src/riscv64.rs   PteFlags::CLAIMED + impl 方法
crates/page_table_entry/src/aarch64.rs   PteFlags::CLAIMED + impl 方法
crates/paging/src/mapping.rs             claim_pages 函数；OwnedPages::new 改为直接调
                                         batch_update_pte；OwnedPages::set_flags 删
                                         with_claimed(true)
tests/paging-test/src/double_claim_panic.rs                  整个文件
tests/paging-test/Cargo.toml             对应 [[bin]] 条目
```

保留清单（poison 相关）：

```
crates/config/src/lib.rs                 FREED_PAGE_POISON 常量——保留
crates/paging/src/mapping.rs             OwnedPages::Drop 的 poison write_bytes——保留
```

**优点**:
- 净删 ~75 行（CLAIMED 相关）+ 释放两架构各一个 PTE 软件位
- `claim_pages` 本身存在 "write-before-check" 的代码与 commit 描述不一致问题（审查中已标记），删除后此问题消失
- `OwnedPages::{new, set_flags}` 的实现统一走 `batch_update_pte`，不再需要 "claimed_flags = flags.with_claimed(true)" 的辅助逻辑
- 保留 poison 的调试价值——任何人通过任何路径读到 0xFEFEFEFE 模式都能意识到 use-after-free，这是**被动的**调试信号，不依赖代码配合检查
- poison 不需要 PTE 软件位，不侵入 trait 接口，成本仅为 Drop 路径的 `write_bytes`

**缺点**:
- Drop 路径仍有 `write_bytes(PAGE_SIZE × n)` 开销——对频繁分配/释放场景有 bandwidth 影响
- 失去对第三方 crate（buddy allocator）bug 导致双重分配的一层运行时捕获——应由 buddy 自己测试覆盖

### 方案 B: 全部删除（CLAIMED + poison）

在方案 A 基础上再删除 poison。

**优点**:
- 净删 ~108 行
- `OwnedPages::Drop` 简化为 `batch_update_pte(kernel_rw) + AllocatedFrames 自然 drop`

**缺点**:
- 完全失去 use-after-free 调试辅助——裸指针访问已释放页面时看到的是之前的合法数据，无任何异常信号
- poison 的成本（per-drop write_bytes）在当前阶段（无频繁帧回收）可忽略

### 方案 C: 保持现状

**优点**:
- 零改动

**缺点**:
- 上述 CLAIMED 相关的所有成本持续存在
- 下次出现"write-before-check"类问题时需要解释/维护

## 决策

选择 **方案 A**——删除 CLAIMED，保留 poison。

## 理由

- **CLAIMED 位的成本-收益失衡**：CLAIMED 的防御依赖"违规者诚实走 OwnedPages::new"——对已有 unsafe 能力的代码不成立。编译期保证（AllocatedFrames move 语义）已覆盖 CLAIMED 所声称的防御场景
- **poison 有独立的调试价值**：poison 不依赖任何 PTE 位，不需要代码"配合检查"——它是被动的异常信号，与 Linux KASAN slab-poison (0x6B) / freed-poison (0x6C) 同一思路。读到 0xFEFEFEFE 模式即知访问了已释放页面，不论访问路径如何
- **poison 始终启用**：不做 `#[cfg(debug_assertions)]` 条件编译。成本（per-drop write_bytes）在当前阶段可接受；release 与 debug 行为一致避免了两套代码路径。如果未来 benchmark 证明是瓶颈，再考虑条件编译
- **方案 C 保留无收益的 CLAIMED 成本**

**CLAIMED vs poison 的本质区别**：
- CLAIMED 是在防线内部加防线——需要 PTE 位、侵入 trait 接口、只对"走 OwnedPages::new 入口"的代码有效
- poison 是独立的调试信号——不侵入任何接口，任何路径的异常访问都能通过数据模式识别

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/page_table_entry/src/lib.rs` | `PteFlagsOps` 删除 `is_claimed` / `with_claimed` 方法 |
| `crates/page_table_entry/src/riscv64.rs` | 删除 `CLAIMED` 常量 + `is_claimed` / `with_claimed` impl |
| `crates/page_table_entry/src/aarch64.rs` | 同上 |
| `crates/paging/src/mapping.rs` | 删除 `claim_pages` 函数；`OwnedPages::new` 改为直接 `batch_update_pte(va_start, page_count, flags)`；`OwnedPages::set_flags` 删除 `.with_claimed(true)` |
| `crates/paging/src/mapping.rs` | 模块头注释删除 "PTE 中的 CLAIMED 软件位…" 段落 |
| `crates/paging/src/mapping.rs` | `OwnedPages::Drop` 的 poison `write_bytes` **保留** |
| `crates/config/src/lib.rs` | `FREED_PAGE_POISON` 常量 **保留** |
| `tests/paging-test/src/double_claim_panic.rs` | 整个文件删除 |
| `tests/paging-test/Cargo.toml` | 删除 `[[bin]] name = "double_claim_panic"` 条目 |

### API 变更

| 项 | 变更 |
|----|------|
| `PteFlagsOps::is_claimed` / `with_claimed` | 删除（未在 crates 外使用） |
| `PteFlags::CLAIMED`（两架构） | 删除（未在 crates 外使用） |
| `config::FREED_PAGE_POISON` | **保留** |
| `OwnedPages` 公开 API | 不变（`new`, `set_flags`, `vaddr`, `size`, `page_count`, `flags`） |
| `OwnedPages::Drop` 行为 | poison 写入保留；仅删除 CLAIMED 清除逻辑 |

### 测试

- 删除 `tests/paging-test/src/double_claim_panic.rs` 及其 `Cargo.toml` bin 条目
- 其他 paging-test 测试不受影响
- 无需新增测试——被删除的 CLAIMED 机制在新设计下等价于 "Rust 类型系统已覆盖"

### 文档

- `crates/paging/src/mapping.rs` 模块 doc comment：删除 CLAIMED 相关段落；保留 poison 描述（"Drop 时写入 poison 填充用于 use-after-free 调试"）
- `docs/design/memory-subsystem-v2.md`：§1.3 "编译期保证优于运行时保证" 章节原本就是这个立场，不需改；§6 "权限覆盖——OwnedPages" 的"编译期保证"表格已经与此 ADR 一致——确认无需修改
- ADR 索引（`docs/adr/README.md`）新增本条目

## 参考

- commit 7d788053e `feat(paging): PTE CLAIMED 软件位 + page poison 纵深防御` — 引入 CLAIMED + poison 机制；本 ADR 删除 CLAIMED，保留 poison
- [ADR-006](006-memory-subsystem-simplification.md) — 确立"编译期保证优于运行时保证"的整体方向（本 ADR 与之一致）
- [Theseus OSDI'20 §4.2](https://www.usenix.org/system/files/osdi20-boos.pdf) — "the Rust compiler is the protection ring"；CLAIMED 机制在此原则下是冗余
- [RustBelt POPL'18](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf) — Rust 所有权与借用形式化证明；本 ADR 依赖其结论
- [Linux KASAN](https://docs.kernel.org/dev-tools/kasan.html) / [Linux KMSAN](https://docs.kernel.org/dev-tools/kmsan.html) — 专用 use-after-free / uninit 检测工具；若未来需要此类能力，应引入专门工具而非在 PTE 位上做通用标记
