#![cfg_attr(not(test), no_std)]
#![feature(sync_unsafe_cell)]

//! 内核内存管理——帧分配器、页表、堆、MMIO 映射。

#[cfg(not(test))]
extern crate alloc;

pub mod address;
#[cfg(not(test))]
pub mod mapped_pages;
#[cfg(not(test))]
pub mod mmio;
pub mod page_table;

#[cfg(not(test))]
pub mod frame;
/// 堆分配器——使用 `target_os = "none"` 门控：
/// `#[global_allocator]` 在宿主机上会与系统分配器冲突。
#[cfg(target_os = "none")]
pub mod heap;

#[cfg(not(test))]
use address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use page_table::{PageFlags, PageTable};

#[cfg(not(test))]
use sync_crate::SpinLock;

/// 全局内核页表。
#[cfg(not(test))]
static KERNEL_PAGE_TABLE: spin::Once<SpinLock<PageTable>> = spin::Once::new();

#[cfg(not(test))]
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize())
}

#[cfg(not(test))]
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize())
}

#[cfg(not(test))]
pub fn identity_map_range(
    pt: &mut PageTable,
    start: PhysAddr,
    end: PhysAddr,
    flags: PageFlags,
) -> error::KResult<()> {
    let mut addr = start.align_down();
    let end_aligned = end.align_up();
    while addr.as_usize() < end_aligned.as_usize() {
        pt.map_page(VirtAddr::new(addr.as_usize()), addr, flags)?;
        addr = addr + config::PAGE_SIZE;
    }
    Ok(())
}

/// 主核内存初始化——返回页表，不激活。
///
/// 使用 `target_os = "none"` 门控（因为依赖 `heap` 模块）。
#[cfg(target_os = "none")]
pub fn init() -> PageTable {
    unsafe { heap::init() };

    let info = boot_info::BASIC_INFO
        .get()
        .expect("BASIC_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    unsafe { frame::init(alloc_start, alloc_size) };

    let mut pt = PageTable::new().expect("failed to create kernel page table");

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();

    // W^X 分段映射：
    // [mem_start, text_end) → RWX（包含 .boot 段的混合 code+data，无法拆分）
    // [text_end, mem_end)   → RW（.rodata + .data + .bss + 空闲内存）
    //
    // 注意：链接脚本的 .boot 段混合了 .text.boot / .data.boot / .bss.boot，
    // 位于 __etext 之前。如果映射为纯 RX，写 .data.boot 会触发 store page fault。
    // 因此 __etext 之前的区域保留 X 权限。
    identity_map_range(&mut pt, mem_start, text_end, PageFlags::kernel_rwx())
        .expect("failed to map kernel code region");
    identity_map_range(
        &mut pt,
        text_end,
        mem_start + mem_size,
        PageFlags::kernel_rw(),
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

#[cfg(not(test))]
pub fn store_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| SpinLock::new(pt, "kernel_pt"));
}

#[cfg(not(test))]
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

#[cfg(not(test))]
pub fn kernel_page_table() -> Option<&'static SpinLock<PageTable>> {
    KERNEL_PAGE_TABLE.get()
}

#[cfg(not(test))]
pub fn map_mmio(paddr: PhysAddr, size: usize) -> error::KResult<VirtAddr> {
    let kpt = KERNEL_PAGE_TABLE
        .get()
        .ok_or(error::ErrorCode::VmInvalidPageTable)?;
    let mut guard = kpt.lock();
    identity_map_range(&mut *guard, paddr, paddr + size, PageFlags::kernel_rw())?;
    arch_traits::flush_tlb();
    Ok(VirtAddr::new(paddr.as_usize()))
}
