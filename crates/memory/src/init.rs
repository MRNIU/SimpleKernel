//! 内存子系统初始化——主核 / 从核。

use address::{PhysAddr, VirtAddr};

use crate::page_table::{PageTable, PteFlags, PteFlagsOps};
use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 页表在内部创建并存入全局 `KERNEL_PAGE_TABLE`，
/// 返回的 `AddressSpace` 持有该页表的 `&'static` 引用。
/// 调用方通过 `AddressSpace::page_table()` 获取页表引用以激活分页。
pub fn init() -> AddressSpace {
    // SAFETY: 在任何堆分配之前调用，且仅调用一次（由启动流程保证）
    unsafe { crate::heap::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    // SAFETY: alloc_start 页对齐（由 align_up 保证），内存区域在内核镜像之后、
    // 物理内存范围之内，不与堆重叠，且仅调用一次
    unsafe { crate::frame::init(alloc_start, alloc_size) };

    // 创建页表并存入全局——获取 &'static 引用以构建 AddressSpace
    let pt = PageTable::create().expect("failed to create kernel page table");
    crate::globals::store_kernel_page_table(pt);
    let pt_ref = crate::globals::kernel_page_table().expect("just stored");

    let mut kernel_as = AddressSpace::new(pt_ref);

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();
    let mem_end = mem_start + mem_size;

    // 分段映射（通过 VMA，自动选择大页）：
    // [mem_start, text_end)    → RWX（.boot 段混合了 code+data，无法拆分为 RX/RW）
    // [text_end, rodata_end)   → RO（.rodata——只读数据，防止意外修改）
    // [rodata_end, mem_end)    → RW（.data + .bss + 空闲内存）
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(mem_start.as_usize()),
            VirtAddr::new(text_end.as_usize()),
            PteFlags::kernel_rwx(),
        )
        .expect("failed to map kernel code region");
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(text_end.as_usize()),
            VirtAddr::new(rodata_end.as_usize()),
            PteFlags::kernel_ro(),
        )
        .expect("failed to map kernel rodata region");
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(rodata_end.as_usize()),
            VirtAddr::new(mem_end.as_usize()),
            PteFlags::kernel_rw(),
        )
        .expect("failed to map kernel data + free memory");

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW)",
        mem_start,
        text_end,
        text_end,
        rodata_end,
        rodata_end,
        mem_end
    );

    kernel_as
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
