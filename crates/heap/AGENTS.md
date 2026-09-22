<!-- Copyright The SimpleKernel Contributors -->

# heap

内核堆分配器——`#[global_allocator]` 实现。

## 职责

本 crate 提供全局 allocator、BSS 引导堆和帧分配器就绪后的堆扩展入口。

## 边界

- 本 crate 不选择物理帧，不管理页表映射，也不定义 DMA-safe 分配语义。
- 本 crate 不允许中断上下文分配；中断 handler 应使用栈对象或 `heapless` 容器。
- 帧来源和堆扩展地址由 `memory` / `frame_allocator` 编排后传入。

## 两阶段初始化

```
memory::init()
   │
   ├── 1. heap::init()              ← BSS 引导堆（64KB，仅够 BTreeSet）
   ├── 2. frame_allocator::init()   （依赖引导堆）
   ├── 3. heap::extend(addr, size)  ← 帧分配器就绪后，用物理帧扩展堆
   └── 4. PageTable 初始化          （使用扩展后的完整堆）
```

引导堆存在的原因：`frame_allocator` 的 buddy 后端内部使用 `BTreeSet`
（第三方 crate `buddy_system_allocator`），需要堆分配。
引导堆是打破 "堆需要帧 ↔ 帧需要堆" 循环依赖的最小 BSS 垫片。

## 中断上下文约束

**禁止在中断上下文中进行堆分配**——alloc/dealloc 入口包含运行时断言。
中断处理器应使用栈分配或 `heapless` 容器。

## 验证入口

- 文档-only 变更：`git diff --check`。
- allocator 初始化或分配路径变更：`cargo clippy -p heap --target riscv64gc-unknown-none-elf -- -D warnings`。
- 堆扩展或中断上下文约束变更：`cargo xtask test --arch riscv64 --name heap-test --timeout 30`。

## 不要假设

- 不要在帧分配器就绪前移除 BSS 引导堆；它用于打破堆和帧分配器的启动循环。
- 不要在中断上下文中引入 `Box`、`Vec`、`String` 或 `format!`。
