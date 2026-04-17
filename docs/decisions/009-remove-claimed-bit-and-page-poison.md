# ADR-009: 删除 CLAIMED 软件位与 page poison 纵深防御

> **状态**: 提议
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

| 项 | 代码量/运行时成本 |
|---|------------------|
| `PteFlagsOps::is_claimed` + `with_claimed` 两个 trait 方法 | 4 行 trait 声明 |
| RISC-V `PteFlags::CLAIMED` 定义 + 两个方法实现 | 17 行 |
| AArch64 `PteFlags::CLAIMED` 定义 + 两个方法实现 | 17 行 |
| `claim_pages` 函数（`paging/mapping.rs`） | 18 行 |
| `OwnedPages::set_flags` 保留 CLAIMED 位的额外逻辑 | 3 行 |
| `OwnedPages::Drop` 的 poison `write_bytes` 调用 | 8 行（含 SAFETY 注释） |
| `FREED_PAGE_POISON` 配置常量 | 2 行 |
| `tests/paging-test/src/double_claim_panic.rs` | 34 行 |
| `tests/paging-test/Cargo.toml` 中 `[[bin]]` 条目 | 5 行 |
| **合计代码** | **~108 行 + 两架构各一个 PTE 软件位** |

**Drop 路径运行时成本**：每次 `OwnedPages::drop` 执行 `write_bytes(0xFE, PAGE_SIZE × n)`：
- 4 页 RAII buffer：16 KB memory write
- 64 页 VM stack：256 KB memory write
- 对于频繁分配/释放场景（如后续引入的任务栈、DMA buffer），这是 hot path 的显著 bandwidth 消耗

**PTE 软件位的机会成本**：
- RISC-V RSW[0] / AArch64 bit 55 被占用后，未来无法用于：
  - ASID 元数据（per-task 地址空间标识）
  - COW（copy-on-write）标记
  - swap-entry 编码（换出到磁盘的页）
  - 引用计数（如 shared pages）

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

### 方案 A: 删除 CLAIMED 位 + page poison

删除清单：

```
crates/page_table_entry/src/lib.rs       PteFlagsOps::is_claimed / with_claimed
crates/page_table_entry/src/riscv64.rs   PteFlags::CLAIMED + impl 方法
crates/page_table_entry/src/aarch64.rs   PteFlags::CLAIMED + impl 方法
crates/paging/src/mapping.rs             claim_pages 函数；OwnedPages::new 改为直接调
                                         batch_update_flags；OwnedPages::Drop 删 poison
                                         写入；OwnedPages::set_flags 删 with_claimed(true)
crates/config/src/lib.rs                 FREED_PAGE_POISON 常量
tests/paging-test/src/double_claim_panic.rs                  整个文件
tests/paging-test/Cargo.toml             对应 [[bin]] 条目
```

**优点**:
- 净删 ~108 行 + Drop 路径省 `write_bytes(PAGE_SIZE × n)` 内存写
- 释放两架构各一个 PTE 软件位
- `OwnedPages::Drop` 简化为 `batch_update_flags(kernel_rw) + AllocatedFrames 自然 drop`——路径更短、更易推理
- `claim_pages` 本身存在 "write-before-check" 的代码与 commit 描述不一致问题（审查中已标记），删除后此问题消失
- `OwnedPages::{new, set_flags}` 的实现统一走 `batch_update_flags`，不再需要 "claimed_flags = flags.with_claimed(true)" 的辅助逻辑

**缺点**:
- 失去对第三方 crate（buddy allocator）bug 的一层运行时捕获——应由 buddy 自己测试覆盖
- 失去 debug 场景下"访问到 0xFEFEFEFE 模式"的调试辅助——可由 `debug_assertions` 下的单独机制替代（见方案 B）

### 方案 B: 保留但改为 `#[cfg(debug_assertions)]` 限定

`poison` 和 CLAIMED 检查只在 debug build 编译，release build 零成本。

**优点**:
- 保留调试场景的辅助
- release 性能不受影响

**缺点**:
- 两套代码路径（debug/release 行为不同），增加测试矩阵
- CLAIMED 位依然占用 PTE 空间（PTE 编码在两种 build 下必须一致，否则内存布局错位）——此优点无法兑现
- 实质是 "debug 提示，release 无用"，不如改用专门的 kasan-like 工具（Linux KMSAN 模式）

### 方案 C: 保持现状

**优点**:
- 零改动

**缺点**:
- 上述所有成本持续存在
- 下次出现"write-before-check"类问题时需要解释/维护

## 决策

选择 **方案 A**。

## 理由

- **成本-收益失衡**：108 行代码 + 每次 Drop 的 PAGE_SIZE × n bytes 写入 + 两个 PTE 软件位，换来检测"第三方 crate bug 或 unsafe 代码违规"——前者应由第三方自己负责，后者 CLAIMED 检测对已有 unsafe 能力的代码无效
- **与真正的纵深防御不在同一维度**：层次 1（语言安全）和层次 2（PTE 权限）已经建立，CLAIMED 不构成独立层次
- **方案 B 因 PTE 编码一致性要求失效**：PTE 位布局不能随 build 模式变化，`cfg(debug_assertions)` 只能砍掉 poison 写入，CLAIMED 位本身无法条件编译
- **方案 C 保留无收益的成本**

**Rust 范式层面**：这是对 "纵深防御 != 越多越好" 的承认。RAII + 所有权已经在编译期保证了 use-after-free 不可能；在此之上加运行时检测机制，需要 **(a) 防住额外的、类型系统覆盖不到的威胁，(b) 防御边界与类型系统独立**。CLAIMED + poison 两项皆不满足，属于 security theater。

## 影响

### 代码变更

| 文件 | 变更 |
|------|------|
| `crates/page_table_entry/src/lib.rs` | `PteFlagsOps` 删除 `is_claimed` / `with_claimed` 方法 |
| `crates/page_table_entry/src/riscv64.rs` | 删除 `CLAIMED` 常量 + `is_claimed` / `with_claimed` impl |
| `crates/page_table_entry/src/aarch64.rs` | 同上 |
| `crates/paging/src/mapping.rs` | 删除 `claim_pages` 函数；`OwnedPages::new` 改为直接 `batch_update_flags(va_start, page_count, flags)`；`OwnedPages::set_flags` 删除 `.with_claimed(true)`；`OwnedPages::Drop` 删除 poison `write_bytes` |
| `crates/paging/src/mapping.rs` | 模块头注释删除 "PTE 中的 CLAIMED 软件位…" 段落 |
| `crates/config/src/lib.rs` | 删除 `FREED_PAGE_POISON` |
| `tests/paging-test/src/double_claim_panic.rs` | 整个文件删除 |
| `tests/paging-test/Cargo.toml` | 删除 `[[bin]] name = "double_claim_panic"` 条目 |

### API 变更

| 项 | 变更 |
|----|------|
| `PteFlagsOps::is_claimed` / `with_claimed` | 删除（未在 crates 外使用） |
| `PteFlags::CLAIMED`（两架构） | 删除（未在 crates 外使用） |
| `config::FREED_PAGE_POISON` | 删除（未在 crates 外使用） |
| `OwnedPages` 公开 API | 不变（`new`, `set_flags`, `vaddr`, `size`, `page_count`, `flags`） |

### 测试

- 删除 `tests/paging-test/src/double_claim_panic.rs` 及其 `Cargo.toml` bin 条目
- 其他 paging-test 测试不受影响
- 无需新增测试——被删除的机制在新设计下等价于 "Rust 类型系统已覆盖"

### 文档

- `crates/paging/src/mapping.rs` 模块 doc comment：删除 CLAIMED + poison 段落
- `docs/design/memory-subsystem-v2.md`：§1.3 "编译期保证优于运行时保证" 章节原本就是这个立场，不需改；§6 "权限覆盖——OwnedPages" 的"编译期保证"表格已经与此 ADR 一致——确认无需修改
- ADR 索引（`docs/decisions/README.md`）新增本条目

## 参考

- commit 7d788053e `feat(paging): PTE CLAIMED 软件位 + page poison 纵深防御` — 引入本 ADR 要删除的机制
- [ADR-006](006-memory-subsystem-simplification.md) — 确立"编译期保证优于运行时保证"的整体方向（本 ADR 与之一致）
- [Theseus OSDI'20 §4.2](https://www.usenix.org/system/files/osdi20-boos.pdf) — "the Rust compiler is the protection ring"；CLAIMED 机制在此原则下是冗余
- [RustBelt POPL'18](https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf) — Rust 所有权与借用形式化证明；本 ADR 依赖其结论
- [Linux KASAN](https://docs.kernel.org/dev-tools/kasan.html) / [Linux KMSAN](https://docs.kernel.org/dev-tools/kmsan.html) — 专用 use-after-free / uninit 检测工具；若未来需要此类能力，应引入专门工具而非在 PTE 位上做通用标记
