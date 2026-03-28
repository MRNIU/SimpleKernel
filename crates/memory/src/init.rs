//! 内存子系统初始化——主核 / 从核。

use address::PhysAddr;

use crate::page_table::{PageTable, PteFlags, PteFlagsOps};

/// 主核内存初始化——返回页表，不激活。
pub fn init() -> PageTable {
    unsafe { crate::heap::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    unsafe { crate::frame::init(alloc_start, alloc_size) };

    let mut pt = PageTable::create().expect("failed to create kernel page table");

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();

    // 分段映射：
    // [mem_start, text_end) → RWX（.boot 段混合了 code+data，无法拆分为 RX/RW）
    // [text_end, mem_end)   → RW（.rodata + .data + .bss + 空闲内存）
    crate::identity_map_range(&mut pt, mem_start, text_end, PteFlags::kernel_rwx())
        .expect("failed to map kernel code region");
    crate::identity_map_range(
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

/// 从核内存初始化——复用主核页表并激活分页。
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let kpt = crate::globals::kernel_page_table().expect("KERNEL_PAGE_TABLE not initialized");
    let guard = kpt.lock();
    activate(&*guard);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
