# page_table_entry

硬件页表项编解码——PTE trait 定义与各架构实现。

## 概览

`page_table_entry` 为不同 CPU 架构的页表项（PTE）提供统一的 trait 接口，
上层页表管理器通过 trait 操作 PTE，无需关心底层位域布局的差异。

两个核心 trait：

- **`PteFlagsOps`** — 标志位操作：preset 映射（`kernel_rw` / `user_rw` 等）、
  权限查询（`is_writable` / `is_user`）、Builder（`with_writable` / `with_executable`）、层级适配
- **`PteOps`** — 完整 PTE 操作：构造（叶 / 中间节点）、地址与标志提取、
  有效性判断、原始值访问

两套架构实现：

| | RISC-V (Sv39/48/57) | AArch64 (Stage 1, 4KB) |
|-|----------------------|------------------------|
| **格式** | PPN [53:10] \| flags [9:0] | addr [47:12] \| 分散标志位 |
| **叶节点判定** | V=1 且 R\|W\|X 至少一位为 1 | 层级相关：L0 所有有效项均为叶；L1+ 需 TABLE=0 |
| **中间节点** | V=1, R/W/X 全零 | VALID=1, TABLE=1 |
| **设备 MMIO** | 等同 kernel_rw（TODO: Svpbmt） | MAIR_IDX1 (Device-nGnRnE) |
| **层级适配** | 无需（格式统一） | 清除 TABLE 位（L1+ 的块描述符） |
| **用户态标记** | USER 位（bit 4） | AP_UNPRIV (bit 6) + NG (bit 11) |

本 crate 无 `alloc` / `config` 依赖，可在 heap 未初始化的早期启动阶段使用。

## PTE 位域布局

### RISC-V

```
63    54 53                       10 9  8  7  6  5  4  3  2  1  0
┌───────┬───────────────────────────┬────┬──┬──┬──┬──┬──┬──┬──┬──┐
│ rsv'd │          PPN              │RSW │ D│ A│ G│ U│ X│ W│ R│ V│
└───────┴───────────────────────────┴────┴──┴──┴──┴──┴──┴──┴──┴──┘
         44-bit PPN → PA[55:12]      ↑ 软件保留位（当前未使用）
```

- Sv39/48/57 使用相同格式，模式无关
- W=1 时必须 R=1（RISC-V 特权规范要求，`new()` 中 debug_assert 检查）

### AArch64

```
63  58 55 54 53  12 11 10 9  8  7  6  4  2  1  0
┌───┬──┬──┬──┬───┬──┬──┬────┬──┬────┬────┬──┬──┐
│rsv│SW│UX│PX│OA │nG│AF│ SH │  │ AP │MAIR│TBL│ V│
└───┴──┴──┴──┴───┴──┴──┴────┴──┴────┴────┴──┴──┘
  ↑                                         ↑
  bit 55: 软件可用（当前未使用）         bit 1: TABLE（叶/中间节点区分）
```

- 叶节点（页/块描述符）与中间节点（表描述符）通过 TABLE 位区分
- L0（最低级）：TABLE=1 表示 4KB 页描述符（叶节点）
- L1+：TABLE=0 表示块描述符（叶节点），TABLE=1 表示表描述符（中间节点）

## W^X 与 preset 映射

所有 preset 映射均遵循 W^X（Write XOR Execute）原则：

| Preset | 权限 | RISC-V 标志 | AArch64 标志 |
|--------|------|-------------|-------------|
| `kernel_rw` | 读写，不可执行 | V+R+W+A+D | VALID+AF+SH_INNER+PXN+UXN |
| `kernel_rx` | 读+执行，不可写 | V+R+X+A+D | VALID+AF+SH_INNER+UXN |
| `kernel_ro` | 只读 | V+R+A+D | VALID+AF+SH_INNER+AP_RO+PXN+UXN |
| `kernel_rwx` | 读写执行 | V+R+W+X+A+D | VALID+AF+SH_INNER+UXN |
| `kernel_device` | 设备 MMIO | V+R+W+A+D | VALID+AF+MAIR_IDX1+PXN+UXN |

### 用户态 preset（当前未使用）

> **注意**：SimpleKernel 采用单地址空间（SAS）架构，不存在用户态/内核态分离。
> 以下 preset 保留用于未来可能的 MPU 辅助隔离或兼容性需求，当前不会被调用。

| Preset | 权限 | RISC-V 标志 | AArch64 标志 |
|--------|------|-------------|-------------|
| `user_rw` | 读写，不可执行 | V+R+W+U+A+D | VALID+AF+SH_INNER+AP_UNPRIV+NG+PXN+UXN |
| `user_rx` | 读+执行，不可写 | V+R+X+U+A | VALID+AF+SH_INNER+AP_UNPRIV+AP_RO+NG+PXN |
| `user_ro` | 只读 | V+R+U+A | VALID+AF+SH_INNER+AP_UNPRIV+AP_RO+NG+PXN+UXN |
| `user_rwx` | 读写执行 | V+R+W+X+U+A+D | VALID+AF+SH_INNER+AP_UNPRIV+NG+PXN |

与内核 preset 的关键差异：

| | RISC-V | AArch64 |
|-|--------|---------|
| **特权区分** | 设 USER 位（仅 U-mode 可访问） | 设 AP_UNPRIV（EL0 可访问） |
| **全局/私有** | 不设 GLOBAL（per-process） | 设 NG（TLB 条目绑定 ASID） |
| **内核执行保护** | N/A（由 USER 位隔离） | 所有用户 preset 均设 PXN（禁止内核执行用户代码页） |

使用 preset 而非手动组合标志位，可以防止出现不合法的权限组合。

## 模块结构

```
src/
├── lib.rs           PteFlagsOps / PteOps trait 定义 + 架构类型别名导出
├── riscv64.rs       RISC-V PteFlags (bitflags) + PageTableEntry 实现
├── aarch64.rs       AArch64 PteFlags (bitflags) + PageTableEntry 实现
└── tests.rs         跨架构泛型测试（地址边界往返）
```

架构选择通过 `#[cfg(target_arch)]` 在编译期完成：

```rust
#[cfg(target_arch = "aarch64")]
pub use aarch64::{PageTableEntry, PteFlags};
#[cfg(target_arch = "riscv64")]
pub use riscv64::{PageTableEntry, PteFlags};
// 非目标架构（宿主机 cargo test）回退到 RISC-V
#[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64")))]
pub use riscv64::{PageTableEntry, PteFlags};
```

## 使用示例

### 构造叶节点

```rust
use address::PhysAddr;
use page_table_entry::{PageTableEntry, PteFlags, PteOps, PteFlagsOps};

// 使用 preset 映射
let pte = PageTableEntry::new(
    PhysAddr::new(0x8020_0000),
    PteFlags::kernel_rw(),
);
assert!(pte.is_valid());
assert!(pte.flags().is_writable());
assert_eq!(pte.paddr(), PhysAddr::new(0x8020_0000));

// 层级适配（AArch64 需要为 L1+ 清除 TABLE 位）
let flags = PteFlags::kernel_rx().for_leaf_at_level(2);
let pte = PageTableEntry::new(PhysAddr::new(0x4000_0000), flags);
```

### 构造中间节点

```rust
// 指向下一级页表的中间节点
let pte = PageTableEntry::new_intermediate(PhysAddr::new(0x8030_0000));
assert!(pte.is_valid());
assert!(!pte.is_leaf(1)); // 中间节点不是叶
```

## 注意事项

### 1. TLB 刷新不在本 crate 职责内

修改页表项后，调用者**必须**执行对应架构的 TLB 维护操作
（RISC-V `sfence.vma` / AArch64 `TLBI` + `DSB` + `ISB`），
否则 CPU 可能继续使用过期的 TLB 缓存。

### 2. `is_leaf` 在 AArch64 上依赖层级参数

RISC-V 的叶节点判定与层级无关（看 R/W/X 位），但 AArch64 必须传入正确的
`level` 参数——L0 的所有有效项均为叶（4KB 页），L1+ 需要 TABLE=0 才是叶（块描述符）。
传错层级会导致叶/中间节点误判。

### 3. RISC-V 的 W+R 约束

RISC-V 特权规范要求 W=1 时 R 必须为 1。`new()` 中有 `debug_assert` 检查，
但 release 模式下不会触发。使用 preset 映射可以完全避免此问题。

## TODO

### Svpbmt 支持

RISC-V 的 `kernel_device()` 当前等同于 `kernel_rw()`，
缺少非缓存属性。待平台支持 Svpbmt 扩展时，需使用 PBMT 位设置
NC（Non-Cacheable）或 IO 属性，避免 MMIO 区域被 CPU 缓存。
