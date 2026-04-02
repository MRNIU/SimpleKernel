//! 内存子系统初始化——主核 / 从核。

use memory_types::{PhysAddr, VirtAddr};
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 初始化顺序：堆 → 帧分配器（统一入口）→ 页分配器 → 页表 → 分段映射。
pub fn init() -> AddressSpace {
    // SAFETY: 在任何堆分配之前调用，且仅调用一次（由启动流程保证）
    unsafe { heap_crate::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();
    let mem_end = mem_start + mem_size;

    // 计算各段页数
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (mem_end - rodata_end) / config::PAGE_SIZE;

    // 空闲内存 = 内核之后的部分
    let free_start = kernel_end.align_up();
    let free_size = mem_size - (free_start - mem_start);

    // SAFETY: 范围有效、页对齐、互不重叠、仅调用一次
    let mut reserved = unsafe {
        frame_allocator::init(
            free_start,
            free_size,
            &[
                (mem_start, text_pages),
                (text_end, rodata_pages),
                (rodata_end, data_pages),
            ],
        )
    };

    // 初始化虚拟页分配器——SAS identity mapping 下 VA == PA。
    // 管理范围从地址 0 到物理内存末尾，覆盖低地址的 MMIO 设备区域
    // 和高地址的物理 RAM。后续 AllocatedPages::alloc_at 从中取页。
    // SAFETY: 虚拟地址范围有效，仅调用一次
    let va_end = mem_start + mem_size;
    unsafe {
        page_allocator::init(VirtAddr::new(0), va_end.as_usize());
    }

    // 创建页表——Box::leak 产出 'static 引用
    let pt = PageTable::create().expect("创建内核页表失败");
    let pt_lock = sync_crate::SpinLock::new(pt, "kernel_pt");
    let pt_static: &'static _ = alloc::boxed::Box::leak(alloc::boxed::Box::new(pt_lock));
    // SAFETY: pt_static 是 'static 引用
    unsafe { paging::set_kernel_page_table(pt_static) };

    let mut kernel_as = AddressSpace::new();

    // 分段映射：.text(RWX) · .rodata(RO) · .data+free(RW)
    // reserved 的元素顺序与传入 init 的 reserved 参数顺序一致。
    // 使用 swap_remove(0) 按顺序消费所有权。
    let segments: [(PhysAddr, PteFlags); 3] = [
        (mem_start, PteFlags::kernel_rwx()),
        (text_end, PteFlags::kernel_ro()),
        (rodata_end, PteFlags::kernel_rw()),
    ];

    for (seg_start, flags) in segments {
        let frames = reserved.swap_remove(0);
        let page_count = frames.count();
        let va = VirtAddr::new(seg_start.as_usize());
        let pages =
            page_allocator::AllocatedPages::alloc_at(va, page_count).expect("内核段页分配失败");
        let mapping = paging::MappedPages::map(pages, frames, flags);
        kernel_as.register_kernel_mapping(va, mapping, crate::vma::VmaKind::Identity);
    }

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
    let guard = paging::kernel_page_table().lock();
    activate(&*guard);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
