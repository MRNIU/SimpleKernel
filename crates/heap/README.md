# heap

内核堆分配器——`#[global_allocator]` 实现。

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
