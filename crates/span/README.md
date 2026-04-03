# `span` — 通用半开区间 `[start, end)` — 零依赖泛型数据结构

> **所属层级**：R1 原语层
>
> **依赖方向**：无依赖；被 `memory_types` 依赖（用于 `FrameSpan`/`PageSpan`）

## 架构设计

### 在系统中的位置

`span` 是最底层的数据结构 crate，提供与领域无关的半开区间抽象。上层 crate（如 `memory_types`）通过 newtype 包装将其特化为物理帧区间、虚拟页区间等内核概念。零依赖设计确保它可以被任意层级复用，不引入传递依赖。

### 设计决策

| 决策 | 选择 | 理由 | ADR |
|------|------|------|-----|
| 区间语义 | 半开区间 `[start, end)` | 与 Rust `Range` 一致，避免 off-by-one；拼接时无重叠 | — |
| `merge` 策略 | 仅支持相邻区间合并，重叠视为错误 | 符合内核分配器语义——已分配区间不应重叠 | — |
| 泛型约束 | `A: Copy + Ord` | 最小化约束：`Copy` 保证轻量值语义，`Ord` 支持比较和排序 | — |

### 参考

- Rust 标准库 `core::ops::Range<T>` — 半开区间语义参考
- Linux `struct resource` — 内核资源区间管理

## 模块细节

### 公共 API

| 类型/函数 | 用途 |
|-----------|------|
| `Span<A>` | 泛型半开区间 `[start, end)` |
| `Span::new(start, end)` | 构造区间，`start > end` 时 panic |
| `Span::start()` / `Span::end()` | 获取起止点 |
| `Span::size()` | 区间大小 |
| `Span::is_empty()` | 判断是否为空区间 |
| `Span::contains(point)` | 判断点是否在区间内 |
| `Span::overlaps(other)` | 判断两区间是否重叠 |
| `Span::contiguous_with(other)` | 判断两区间是否相邻 |
| `Span::split_at(point)` | 在指定点将区间一分为二，越界时 panic |
| `Span::merge(other)` | 合并相邻区间，非相邻时 panic |
| `Span::iter()` | 返回区间内元素的迭代器 |
| `SpanIter<A>` | 实现 `Iterator` + `ExactSizeIterator` |

### 内部设计

`Span<A>` 仅包含 `start: A` 和 `end: A` 两个字段，无堆分配。所有方法均为纯函数式操作（`split_at` 和 `merge` 返回新 `Span`，不修改原值）。`SpanIter` 通过 `Step` trait 约束实现逐元素迭代。

### Safety 说明

本 crate 无 unsafe 代码。

### 错误处理

- `Span::new(start, end)`：`start > end` 时 panic（构造不变量）
- `Span::split_at(point)`：`point` 不在 `[start, end]` 范围内时 panic
- `Span::merge(other)`：两区间不相邻时 panic

所有 panic 均属于"不应该发生"的不变量违反，符合项目内核错误处理策略。

## 测试

| 测试类型 | 文件 | 覆盖范围 |
|----------|------|----------|
| 单元测试 | `src/lib.rs` | 11 个测试：basic、empty、overlaps、invalid_panics、split、merge_ok、merge_fail、iter_elements、contiguous、iter_exact_size、iter_exact_size_empty |

```bash
cargo test -p span
```

## 依赖

无依赖（零依赖 crate）。
