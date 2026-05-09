// Copyright The SimpleKernel Contributors

//! 架构抽象层——所有因处理器架构而异的底层操作的统一接口。
//!
//! 本 crate 封装 CPU 寄存器访问、中断控制、TLB 维护等硬件原语，
//! 对外暴露架构无关的模块级函数，上层 crate 无需关心具体架构差异。
//!
//! ## 支持的架构
//!
//! | 架构 | 条件编译 | 说明 |
//! |------|----------|------|
//! | RISC-V 64 | `bare_riscv64` | S 模式，Sv39 |
//! | AArch64 | `bare_aarch64` | EL1，4KB granule |
//!
//! ## 使用示例
//!
//! ```ignore
//! arch::disable_irq();
//! let base = arch::percpu_base();
//! arch::flush_tlb_page(vaddr);
//! ```

#![no_std]

#[cfg(bare_aarch64)]
mod aarch64;
#[cfg(bare_riscv64)]
mod riscv64;

/// 架构实现契约——各架构必须完整实现。
///
/// 此 trait 仅在 crate 内部用于强制各架构提供完整的方法集，
/// 对外通过模块级函数暴露，调用方无需感知 trait 的存在。
pub(crate) trait ArchImpl {
    /// 物理地址有效位宽。
    ///
    /// - RISC-V Sv39/Sv48/Sv57: 56 位
    /// - AArch64: 44 位（匹配当前 QEMU `cortex-a72` 平台）
    const PA_BITS: usize;

    /// 页表层级数。
    ///
    /// - RISC-V Sv39: 3 级
    /// - AArch64 4KB granule: 4 级
    const PT_LEVELS: usize;

    /// 读取 per-CPU 基地址寄存器（RISC-V: TP, AArch64: TPIDR_EL1）。
    fn percpu_base() -> usize;

    /// 写入 per-CPU 基地址寄存器。
    ///
    /// # Safety
    /// 调用方必须保证 `val` 指向有效的 per-CPU 区域基地址。
    unsafe fn set_percpu_base(val: usize);

    /// 从硬件寄存器读取当前核心 ID。
    ///
    /// 在 per-CPU 子系统初始化之前可用。
    /// - RISC-V: 从 TP 读取 hart_id（boot.S 写入）
    /// - AArch64: 从 MPIDR_EL1.Aff0 读取
    fn core_id() -> usize;

    /// 查询当前中断是否启用。
    fn is_irq_enabled() -> bool;

    /// 禁用中断。
    fn disable_irq();

    /// 启用中断。
    ///
    /// # Safety
    /// 调用方必须确保：
    /// 1. 当前不在持有禁止中断的锁的临界区内
    /// 2. 中断向量表已正确初始化
    /// 3. 栈和上下文状态允许安全地处理中断
    unsafe fn enable_irq();

    /// 刷新整个 TLB。
    fn flush_tlb_all();

    /// 刷新指定虚拟地址的单条 TLB 表项。
    fn flush_tlb_page(vaddr: usize);
}

#[cfg(bare_riscv64)]
type Impl = riscv64::Riscv64;
#[cfg(bare_aarch64)]
type Impl = aarch64::Aarch64;

/// 物理地址有效位宽（RISC-V: 56, AArch64: 44）。
pub const PA_BITS: usize = Impl::PA_BITS;

/// 页表层级数（RISC-V Sv39: 3, AArch64 4KB: 4）。
pub const PT_LEVELS: usize = Impl::PT_LEVELS;

/// PTE 大小的位移量——`log2(sizeof(u64))` = 3。
///
/// 两种架构的 PTE 均为 64 位，此常量在所有架构下一致。
pub const PTE_SIZE_SHIFT: usize = core::mem::size_of::<u64>().trailing_zeros() as usize;

/// 每张页表中的条目数（PAGE_SIZE / sizeof(PTE)）。
///
/// 64 位架构中 PTE 均为 8 字节，4KB 页对应 512 条目。
pub const ENTRIES_PER_TABLE: usize = config::PAGE_SIZE / core::mem::size_of::<u64>();

/// 单级索引位宽（log2(ENTRIES_PER_TABLE)）。
pub const INDEX_BITS: usize = config::PAGE_SIZE_BITS - PTE_SIZE_SHIFT;

/// 每级 VPN 索引掩码——所有层级相同（`ENTRIES_PER_TABLE - 1`）。
///
/// RISC-V Sv39/48/57 和 AArch64 的页表每级索引位宽相同（9 bits），
/// 因此 mask 无需 per-level 存储。
pub const INDEX_MASK: usize = ENTRIES_PER_TABLE - 1;

/// 各级 VPN 在虚拟地址中的起始位位置。
///
/// `LEVEL_SHIFTS[0]` = `PAGE_SIZE_BITS`（12），
/// 后续每级递增 `INDEX_BITS`（9）。
const fn compute_level_shifts() -> [usize; PT_LEVELS] {
    let mut shifts = [0usize; PT_LEVELS];
    shifts[0] = config::PAGE_SIZE_BITS;
    let mut i = 1;
    while i < PT_LEVELS {
        shifts[i] = shifts[i - 1] + INDEX_BITS;
        i += 1;
    }
    shifts
}

pub const LEVEL_SHIFTS: [usize; PT_LEVELS] = compute_level_shifts();

/// 返回第 `level` 级映射的页大小（字节）。
#[inline]
pub const fn page_size_at_level(level: usize) -> usize {
    1usize << LEVEL_SHIFTS[level]
}

/// 虚拟地址有效位宽——由 [`PT_LEVELS`] 自动推导。
///
/// 计算公式：`PAGE_SIZE_BITS + PT_LEVELS × INDEX_BITS`
/// - Sv39 (PT_LEVELS=3): 12 + 3×9 = 39
/// - Sv48 / AArch64 4KB (PT_LEVELS=4): 12 + 4×9 = 48
/// - Sv57 (PT_LEVELS=5): 12 + 5×9 = 57
pub const VA_BITS: usize = config::PAGE_SIZE_BITS + Impl::PT_LEVELS * INDEX_BITS;

/// 读取 per-CPU 基地址寄存器（RISC-V: TP, AArch64: TPIDR_EL1）。
#[inline(always)]
pub fn percpu_base() -> usize {
    Impl::percpu_base()
}

/// 写入 per-CPU 基地址寄存器。
///
/// # Safety
/// 调用方必须保证 `val` 指向有效的 per-CPU 区域基地址。
#[inline(always)]
pub unsafe fn set_percpu_base(val: usize) {
    // SAFETY: 由调用方保证
    unsafe { Impl::set_percpu_base(val) };
}

/// 从硬件寄存器读取当前核心 ID（per-CPU 初始化前可用）。
#[inline(always)]
pub fn core_id() -> usize {
    Impl::core_id()
}

/// 查询当前中断是否启用。
#[inline(always)]
pub fn is_irq_enabled() -> bool {
    Impl::is_irq_enabled()
}

/// 禁用中断。
#[inline(always)]
pub fn disable_irq() {
    Impl::disable_irq();
}

/// 启用中断。
///
/// # Safety
/// 调用方必须确保：
/// 1. 当前不在持有禁止中断的锁的临界区内
/// 2. 中断向量表已正确初始化
/// 3. 栈和上下文状态允许安全地处理中断
#[inline(always)]
pub unsafe fn enable_irq() {
    // SAFETY: 由调用方保证
    unsafe { Impl::enable_irq() };
}

/// 刷新整个 TLB。
#[inline(always)]
pub fn flush_tlb_all() {
    Impl::flush_tlb_all();
}

/// 刷新指定虚拟地址的单条 TLB 表项。
#[inline(always)]
pub fn flush_tlb_page(vaddr: usize) {
    Impl::flush_tlb_page(vaddr);
}
