# memory_types

内核内存基础类型——编译期区分物理/虚拟地址与帧/页号，防止混用。

## 概览

`memory_types` 提供类型安全的地址和页帧 newtype，在编译期区分物理地址与虚拟地址、
字节粒度与页粒度，杜绝跨类型的误用。

地址 newtype 均为 `#[repr(transparent)]`，零开销包装 `usize`。
不同类型之间不可隐式转换——`PhysAddr` 和 `VirtAddr` 是编译期不同的类型，
必须通过显式的 `PhysAddr::to_virt()` / `VirtAddr::to_phys()` 方法转换。
转换使用 `wrapping_add` / `wrapping_sub` 以支持 higher-half kernel 布局，
偏移量由 `config::PHYS_OFFSET` 控制。

范围类型 `Span<A>` 由独立的 `span` crate 提供，本 crate re-export。

## 核心类型

```rust
// 字节粒度地址
pub struct PhysAddr(usize);          // 物理地址
pub struct VirtAddr(usize);          // 虚拟地址

// 页粒度标识（泛型页大小，默认 4K）
pub struct Frame<P: PageSize = Page4K> { .. } // 物理帧
pub struct Page<P: PageSize = Page4K>  { .. } // 虚拟页

// 页大小标记（sealed trait）
pub struct Page4K;
pub struct Page2M;
pub struct Page1G;

// 范围（re-export from span crate）
pub struct Span<A> { start: A, end: A }    // 半开区间 [start, end)

// 便利别名
pub type FrameSpan = Span<Frame>;
pub type PageSpan  = Span<Page>;
```

## 类型关系

```
PhysAddr ←──→ Frame            (page_number / start_addr)
VirtAddr ←──→ Page             (page_number / start_addr)

PhysAddr ←──→ VirtAddr         (PhysAddr::to_virt / VirtAddr::to_phys)

Span<PhysAddr>                 字节粒度的物理地址范围
Span<VirtAddr>                 字节粒度的虚拟地址范围
Span<Frame>    = FrameSpan     物理帧范围
Span<Page>     = PageSpan      虚拟页范围
```

## 模块结构

```
crates/span/src/
└── lib.rs           Span<A> 泛型范围、分割/合并/迭代（零依赖）

crates/memory_types/src/
├── lib.rs           crate 入口，re-export + impl_usize_newtype! 宏
├── addr.rs          PhysAddr / VirtAddr、对齐方法、PhysAddr::to_virt / VirtAddr::to_phys
├── page_frame.rs    Frame<P> / Page<P>、与地址类型的互转
└── page_size.rs     PageSize trait + Page4K / Page2M / Page1G
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
use memory_types::{Frame, Page4K, FrameSpan};

type F = Frame<Page4K>;
let range = FrameSpan::new(F::new(0), F::new(8));
assert_eq!(range.size(), 8);
assert!(range.contains(F::new(3)));
assert!(!range.contains(F::new(8))); // 半开区间，end 不含

// 分割与合并
let (left, right) = range.split_at(F::new(4));
let merged = left.merge(right).expect("相邻范围可合并");
assert_eq!(merged, range);

// 迭代
for frame in range.iter() {
    // frame: Frame<4K>(0x0), Frame<4K>(0x1), ..., Frame<4K>(0x7)
}
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

### 3. `PhysAddr::to_virt` / `VirtAddr::to_phys` 仅适用于线性映射

这两个方法假设物理地址和虚拟地址之间存在固定偏移关系（`PHYS_OFFSET`）。
对于非线性映射的地址（如用户空间地址），必须通过页表查询进行转换。
