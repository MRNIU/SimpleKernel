//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#![cfg_attr(not(test), no_std)]
#![feature(sync_unsafe_cell)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

#[cfg(target_os = "none")]
extern crate alloc;

/// 错误类型。
pub mod error;
/// 仿射类型映射（Theseus 风格 `MappedPages`）。
#[cfg(target_os = "none")]
pub mod mapped_pages;
/// 类型化 MMIO 区域。
#[cfg(target_os = "none")]
pub mod mmio;
/// 多级页表与架构原生 PTE 标志位。
pub mod page_table;
/// TLB 刷新。
pub mod tlb;

/// 物理帧分配器与帧生命周期状态机。
#[cfg(target_os = "none")]
pub mod frame;
/// 堆分配器。
///
/// `#[global_allocator]` 在宿主机上会与系统分配器冲突，因此门控为裸机专用。
#[cfg(target_os = "none")]
pub mod heap;

use address::PhysAddr;
#[cfg(target_os = "none")]
use address::VirtAddr;
#[cfg(target_os = "none")]
use page_table::{PageTable, PteFlags, PteFlagsOps};

#[cfg(target_os = "none")]
use sync_crate::SpinLock;
/// 内核启动时从 FDT 解析出的内存布局信息。
///
/// 在 `early_init()` 中通过 `MEMORY_INFO.call_once()` 填充，
/// 之后由内存子系统只读访问。
pub struct MemoryInfo {
    /// 物理内存起始地址
    pub physical_memory_addr: PhysAddr,
    /// 物理内存大小（字节）
    pub physical_memory_size: usize,
    /// 内核镜像起始物理地址
    pub kernel_addr: PhysAddr,
    /// 内核镜像大小（字节）
    pub kernel_size: usize,
}

/// 全局内存布局信息（一次性初始化，之后只读）
pub static MEMORY_INFO: spin::Once<MemoryInfo> = spin::Once::new();

/// 全局内核页表。
#[cfg(target_os = "none")]
static KERNEL_PAGE_TABLE: spin::Once<SpinLock<PageTable>> = spin::Once::new();

/// 物理地址转虚拟地址（当前为 identity mapping，直接透传）。
#[cfg(target_os = "none")]
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize())
}

/// 虚拟地址转物理地址（当前为 identity mapping，直接透传）。
#[cfg(target_os = "none")]
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize())
}

/// 将 `[start, end)` 物理地址区间 identity-map 到页表中。
///
/// 自动使用最大可用页大小（1GB / 2MB / 4KB），减少 TLB 压力和页表内存占用。
/// 地址和剩余大小都对齐到大页边界时才使用大页映射。
///
/// # Errors
///
/// 映射冲突时返回错误。
#[cfg(target_os = "none")]
pub fn identity_map_range(
    pt: &mut PageTable,
    start: PhysAddr,
    end: PhysAddr,
    flags: PteFlags,
) -> Result<(), crate::error::MemoryError> {
    let mut addr = start.align_down();
    let end_aligned = end.align_up();

    while addr.as_usize() < end_aligned.as_usize() {
        let remaining = end_aligned.as_usize() - addr.as_usize();
        let va = VirtAddr::new(addr.as_usize());

        // 从最大页尝试到最小页
        let mut mapped = false;
        for level in (1..config::PT_LEVELS).rev() {
            let page_size = page_table::page_size_at_level(level);
            if addr.as_usize() % page_size == 0 && remaining >= page_size {
                pt.map_at_level(va, addr, flags, level)?;
                addr += page_size;
                mapped = true;
                break;
            }
        }
        if !mapped {
            pt.map_page(va, addr, flags)?;
            addr += config::PAGE_SIZE;
        }
    }
    Ok(())
}

/// 主核内存初始化——返回页表，不激活。
///
/// 使用 `target_os = "none"` 门控（因为依赖 `heap` 模块）。
#[cfg(target_os = "none")]
pub fn init() -> PageTable {
    unsafe { heap::init() };

    let info = MEMORY_INFO.get().expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    unsafe { frame::init(alloc_start, alloc_size) };

    let mut pt = PageTable::create().expect("failed to create kernel page table");

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();

    // 分段映射：
    // [mem_start, text_end) → RWX（.boot 段混合了 code+data，无法拆分为 RX/RW）
    // [text_end, mem_end)   → RW（.rodata + .data + .bss + 空闲内存）
    identity_map_range(&mut pt, mem_start, text_end, PteFlags::kernel_rwx())
        .expect("failed to map kernel code region");
    identity_map_range(
        &mut pt,
        text_end,
        mem_start + mem_size,
        PteFlags::kernel_rw(),
    )
    .expect("failed to map kernel data + free memory");

    log::info!(
        "MemoryInit: code {}-{} (RWX), data {}-{} (RW)",
        mem_start,
        text_end,
        text_end,
        mem_start + mem_size
    );

    pt
}

/// 将构建完成的内核页表存入全局 `KERNEL_PAGE_TABLE`。
#[cfg(target_os = "none")]
pub fn store_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| SpinLock::new(pt, "kernel_pt"));
}

/// 从核内存初始化——复用主核页表并激活分页。
#[cfg(target_os = "none")]
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let kpt = KERNEL_PAGE_TABLE
        .get()
        .expect("KERNEL_PAGE_TABLE not initialized");
    let guard = kpt.lock();
    activate(&*guard);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}

/// 获取全局内核页表的引用；初始化前返回 `None`。
#[cfg(target_os = "none")]
pub fn kernel_page_table() -> Option<&'static SpinLock<PageTable>> {
    KERNEL_PAGE_TABLE.get()
}

/// 将 MMIO 物理地址区间 identity-map 到内核页表，返回对应虚拟地址。
///
/// # Errors
///
/// 内核页表未初始化或映射冲突时返回错误。
#[cfg(target_os = "none")]
pub fn map_mmio(paddr: PhysAddr, size: usize) -> Result<VirtAddr, crate::error::MemoryError> {
    let kpt = KERNEL_PAGE_TABLE
        .get()
        .ok_or(crate::error::MemoryError::InvalidPageTable)?;
    let mut guard = kpt.lock();
    identity_map_range(&mut *guard, paddr, paddr + size, PteFlags::kernel_device())?;
    crate::tlb::flush_tlb();
    Ok(VirtAddr::new(paddr.as_usize()))
}
