//! 内存子系统初始化——主核 / 从核。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 初始化顺序：堆 → 帧分配器 → 页表 → identity mapping → 所有权声明。
///
/// SAS 架构下全部物理内存在此阶段完成 identity mapping（PTE 永不删除）。
/// 内核段帧通过 `MappedPages::from_claimed` 声明所有权，free memory 帧
/// 由 buddy allocator 管理，使用时通过 `MappedPages::claim` 声明。
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
    let kernel_end_aligned = kernel_end.align_up();

    // 计算各段页数——仅覆盖内核镜像，不包含 free memory
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (kernel_end_aligned - rodata_end) / config::PAGE_SIZE;

    // 空闲内存 = 内核镜像之后的部分，交由 buddy allocator 管理
    let free_start = kernel_end_aligned;
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

    // 创建页表并建立全部物理内存的 identity mapping。
    // 分段设置权限：.text(RWX)、.rodata(RO)、其余(RW)。
    // 内核段的 CLAIMED 位在此设置——表示这些帧由 MappedPages 持有。
    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    {
        let mut guard = paging::kernel_page_table().lock();
        // .text 段：RWX + CLAIMED
        guard.identity_map_range(
            mem_start,
            text_end,
            PteFlags::kernel_rwx().with_claimed(true),
        );
        // .rodata 段：RO + CLAIMED
        guard.identity_map_range(
            text_end,
            rodata_end,
            PteFlags::kernel_ro().with_claimed(true),
        );
        // .data+.bss 段：RW + CLAIMED
        guard.identity_map_range(
            rodata_end,
            kernel_end_aligned,
            PteFlags::kernel_rw().with_claimed(true),
        );
        // free memory：RW（无 CLAIMED——由 buddy 管理，使用时通过 claim 声明）
        guard.identity_map_range(free_start, mem_end, PteFlags::kernel_rw());
    }

    let mut kernel_as = AddressSpace::new();

    // 包装 reserved frames 为 MappedPages——仅做 typestate 转换，
    // PTE 和 CLAIMED 位已在上面的 identity_map_range 中设置。
    let segments: [PteFlags; 3] = [
        PteFlags::kernel_rwx(),
        PteFlags::kernel_ro(),
        PteFlags::kernel_rw(),
    ];
    let seg_starts = [mem_start, text_end, rodata_end];

    for (i, flags) in segments.into_iter().enumerate() {
        let frames = reserved.remove(0);
        let va = memory_types::VirtAddr::new(seg_starts[i].as_usize());
        // SAFETY: PTE 已由 identity_map_range 建立且 CLAIMED 位已设置
        let mapping = unsafe { paging::MappedPages::from_claimed(frames, flags) };
        kernel_as.register_kernel_mapping(va, mapping);
    }

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW), free {}-{} (RW)",
        mem_start,
        text_end,
        text_end,
        rodata_end,
        rodata_end,
        kernel_end_aligned,
        free_start,
        mem_end
    );

    kernel_as
}

/// 从核内存初始化——复用主核页表并激活分页。
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let guard = paging::kernel_page_table().lock();
    activate(&guard);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
