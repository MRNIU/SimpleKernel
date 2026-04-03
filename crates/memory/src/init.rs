//! 内存子系统初始化——主核 / 从核。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 初始化顺序：堆 -> 帧分配器 -> 页表 -> 分段映射。
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

    // 创建页表——写入 paging 模块的静态存储，无需堆分配
    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    let mut kernel_as = AddressSpace::new();

    // 分段映射：.text(RWX) / .rodata(RO) / .data+free(RW)
    // reserved 的元素顺序与传入 init 的 reserved 参数顺序一致。
    // identity mapping: VA == PA，MappedPages::map 从 PA 推导 VA。
    let segments: [PteFlags; 3] = [
        PteFlags::kernel_rwx(),
        PteFlags::kernel_ro(),
        PteFlags::kernel_rw(),
    ];

    let seg_starts = [mem_start, text_end, rodata_end];

    for (i, flags) in segments.into_iter().enumerate() {
        let frames = reserved.remove(0);
        let va = memory_types::VirtAddr::new(seg_starts[i].as_usize());
        let mapping = paging::MappedPages::map(frames, flags);
        kernel_as.register_kernel_mapping(va, mapping);
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
