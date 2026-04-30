# memory_types

内核内存基础类型——编译期区分物理/虚拟地址与帧/页号，防止混用。

## 概览

`memory_types` 提供类型安全的地址和页帧 newtype，在编译期区分物理地址与虚拟地址、
字节粒度与页粒度，杜绝跨类型的误用。

地址 newtype 均为 `#[repr(transparent)]`，零开销包装 `usize`。
不同类型之间不可隐式转换——`PhysAddr` 和 `VirtAddr` 是编译期不同的类型，
必须通过显式的 `PhysAddr::to_virt()` / `VirtAddr::to_phys()` 方法转换。
SimpleKernel 当前只支持 SAS identity mapping，因此这两个方法保持地址数值不变。

范围类型 `Span<A>` 定义在本 crate 内部。

## 核心类型

```rust
// 字节粒度地址
pub struct PhysAddr(usize);          // 物理地址
pub struct VirtAddr(usize);          // 虚拟地址

// 页粒度标识（固定 4KB）
pub struct Frame { number: usize }    // 物理帧
pub struct Page  { number: usize }    // 虚拟页

// 半开区间
pub struct Span<A> { start: A, end: A }    // [start, end)

// 便利别名（FrameSpan 定义在 frame_allocator 内部，不在本 crate 公共 API 中）
// pub(crate) type FrameSpan = Span<Frame>;  // frame_allocator 内部使用
```

## 类型关系

```
PhysAddr ←──→ Frame            (page_number / start_addr)
VirtAddr ←──→ Page             (page_number / start_addr)

PhysAddr ←──→ VirtAddr         (PhysAddr::to_virt / VirtAddr::to_phys)

Span<PhysAddr>                 字节粒度的物理地址范围
Span<VirtAddr>                 字节粒度的虚拟地址范围
Span<Frame>                    物理帧范围（frame_allocator 内部使用）
Span<Page>                     虚拟页范围（当前未使用）
```

## 模块结构

```
crates/memory_types/src/
├── lib.rs           crate 入口，impl_usize_newtype! 宏
├── addr.rs          PhysAddr / VirtAddr、对齐方法、PhysAddr::to_virt / VirtAddr::to_phys
├── page_frame.rs    Frame / Page、与地址类型的互转
└── span.rs          Span<A> 半开区间，重叠检测
```

三个宏（`impl_usize_newtype!`、`impl_addr!`、`impl_page_or_frame!`）消除
重复代码——构造/访问、checked 算术运算、对齐方法、`From` 互转、
`Display` 格式化等，均由宏统一生成。

## 使用示例

### 地址与帧/页

```rust
use memory_types::{PhysAddr, Frame};

// 构造与对齐
let pa = PhysAddr::new(0x8020_0001);
assert!(!pa.is_aligned());
assert_eq!(pa.align_down(), PhysAddr::new(0x8020_0000));
assert_eq!(pa.align_up(), PhysAddr::new(0x8020_1000));

// 物理 ↔ 虚拟
let va = pa.to_virt();
assert_eq!(va.to_phys(), pa);

// 地址 ↔ 帧号（向下取整）
let addr = PhysAddr::new(0x8020_3000);
let f = addr.page_number();
assert_eq!(f.as_usize(), 0x8020_3);
assert_eq!(f.start_addr(), addr);
```

### 帧范围

```rust
use memory_types::{Frame, Span};

let range = Span::new(Frame::new(0), Frame::new(8));
assert_eq!(range.size(), 8);
assert_eq!(range.start(), Frame::new(0));
assert_eq!(range.end(), Frame::new(8));

// 重叠检测
let other = Span::new(Frame::new(4), Frame::new(12));
assert!(range.overlaps(other));
```

## 注意事项

### 1. 未对齐地址转页号会截断

`page_number()` 执行右移（向下取整），不会报错。未页对齐的地址低位会被丢弃：

```rust
let addr = PhysAddr::new(0x8020_3FFF);
let f = addr.page_number();
assert_eq!(f.start_addr(), PhysAddr::new(0x8020_3000)); // 不是 0x8020_3FFF
```

### 2. `align_up` 在地址空间顶部会 panic

当地址接近 `usize::MAX` 时，`align_up()` 无法向上对齐而不溢出，
会 panic 而非静默回绕。这是有意为之，防止产生错误的地址值。

### 3. `PhysAddr::to_virt` / `VirtAddr::to_phys` 仅适用于 identity mapping

这两个方法假设物理地址和虚拟地址数值相同（VA == PA）。
对于非线性映射的地址（如用户空间地址），必须通过页表查询进行转换。
