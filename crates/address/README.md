# address

内核地址类型库——编译期区分物理/虚拟地址，防止混用。

## 概览

`address` 提供四种 newtype 包装和一个泛型范围类型，在编译期区分物理地址与虚拟地址、
字节粒度与页粒度，杜绝跨类型的误用。

四种 newtype 均为 `#[repr(transparent)]`，零开销包装 `usize`。
不同类型之间不可隐式转换——`PhysAddr` 和 `VirtAddr` 是编译期不同的类型，
必须通过显式的 `phys_to_virt()` / `virt_to_phys()` 函数转换。
转换使用 `wrapping_add` / `wrapping_sub` 以支持 higher-half kernel 布局
（虚拟地址在高地址空间，直接加法可能溢出 `usize`），
偏移量由 `config::PHYS_OFFSET` 控制。

从 `memory` crate 独立出来的原因：地址类型是内存子系统中最底层的抽象，
被帧分配器（`frame_allocator`）、页表（`page_table`）、映射管理等上层模块共同依赖。
独立 crate 使依赖方向单向化，也避免了上层模块因引用地址类型而被迫依赖整个 `memory`。

## 核心类型

```rust
// 字节粒度地址
pub struct PhysAddr(usize);   // 物理地址——标识物理内存或 MMIO 的字节位置
pub struct VirtAddr(usize);   // 虚拟地址——CPU 可直接访问的指针级地址

// 页粒度索引
pub struct PhysPageNum(usize); // 物理页号——页表操作中的帧索引
pub struct VirtPageNum(usize); // 虚拟页号——页表操作中的虚拟页索引

// 泛型范围——半开区间 [start, end)
pub struct AddrRange<A> { start: A, end: A }

// 便利别名
pub type FrameRange = AddrRange<PhysPageNum>;
pub type PageRange  = AddrRange<VirtPageNum>;
```

## 类型关系

```
PhysAddr ←──→ PhysPageNum       (页号 = 地址 >> PAGE_SIZE_BITS)
VirtAddr ←──→ VirtPageNum       (页号 = 地址 >> PAGE_SIZE_BITS)

PhysAddr ←──→ VirtAddr          (phys_to_virt / virt_to_phys，偏移量 = PHYS_OFFSET)

AddrRange<PhysAddr>             字节粒度的物理地址范围
AddrRange<VirtAddr>             字节粒度的虚拟地址范围
AddrRange<PhysPageNum>          = FrameRange（物理帧范围）
AddrRange<VirtPageNum>          = PageRange（虚拟页范围）
```

## 模块结构

```
src/
├── lib.rs           crate 入口，pub use 汇总 + impl_usize_newtype! 宏
├── addr.rs          PhysAddr / VirtAddr 定义、对齐方法、phys_to_virt / virt_to_phys
├── page_num.rs      PhysPageNum / VirtPageNum 定义、与地址类型的互转
└── range.rs         AddrRange<A> 泛型范围、分割/合并/迭代
```

三个宏（`impl_usize_newtype!`、`impl_addr!`、`define_page_num!`）消除了
四种 newtype 之间的重复代码——构造/访问、算术运算、对齐方法、`From` 互转、
`Display` 格式化等，均由宏统一生成。详见各宏的文档注释。

## 使用示例

### 地址与页号

```rust
use address::{PhysAddr, PhysPageNum, phys_to_virt, virt_to_phys};

// 构造与对齐
let pa = PhysAddr::new(0x8020_0001);
assert!(!pa.is_aligned());
assert_eq!(pa.align_down(), PhysAddr::new(0x8020_0000));
assert_eq!(pa.align_up(), PhysAddr::new(0x8020_1000));

// 物理 ↔ 虚拟
let va = phys_to_virt(pa);
assert_eq!(virt_to_phys(va), pa);

// 地址 ↔ 页号（向下取整）
let addr = PhysAddr::new(0x8020_3000);
let pn = addr.page_number();
assert_eq!(pn.as_usize(), 0x8020_3);
assert_eq!(pn.start_addr(), addr);

// VirtAddr ↔ 裸指针
let ptr: *const u8 = va.as_ptr();
let back = address::VirtAddr::from(ptr);
```

### 地址范围

```rust
use address::{PhysPageNum, FrameRange};

let range = FrameRange::new(PhysPageNum::new(0), PhysPageNum::new(8));
assert_eq!(range.size(), 8);
assert!(range.contains(PhysPageNum::new(3)));
assert!(!range.contains(PhysPageNum::new(8))); // 半开区间，end 不含

// 分割与合并
let (left, right) = range.split_at(PhysPageNum::new(4));
let merged = left.merge(right).expect("相邻范围可合并");
assert_eq!(merged, range);

// 迭代
for page in range.iter() {
    // page: PhysPageNum(0), PhysPageNum(1), ..., PhysPageNum(7)
}
```

## 注意事项

### 1. 未对齐地址转页号会截断

`page_number()` 执行右移（向下取整），不会报错。未页对齐的地址低位会被丢弃：

```rust
let addr = PhysAddr::new(0x8020_3FFF);
let pn = addr.page_number();
assert_eq!(pn.start_addr(), PhysAddr::new(0x8020_3000)); // 不是 0x8020_3FFF
```

### 2. `align_up` 在地址空间顶部会 panic

当地址接近 `usize::MAX` 时，`align_up()` 无法向上对齐而不溢出，
会 panic 而非静默回绕。这是有意为之，防止产生错误的地址值。

### 3. `phys_to_virt` / `virt_to_phys` 仅适用于线性映射

这两个函数假设物理地址和虚拟地址之间存在固定偏移关系（`PHYS_OFFSET`）。
对于非线性映射的地址（如用户空间地址），必须通过页表查询进行转换。
