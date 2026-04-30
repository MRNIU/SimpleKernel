//! 内存子系统初始化——主核 / 从核。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

/// 主核内存初始化——引导堆 → 帧分配器 → 堆扩展 → 页表 → 权限覆盖。
pub fn init() {
    // SAFETY: 在任何堆分配之前调用，且仅调用一次（由启动流程保证）
    unsafe { heap_crate::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_start = info.kernel_addr;
    let kernel_end = PhysAddr::new(
        kernel_start
            .as_usize()
            .checked_add(info.kernel_size)
            .expect("MemoryInit: kernel 结束地址溢出"),
    );
    let mem_end = PhysAddr::new(
        mem_start
            .as_usize()
            .checked_add(mem_size)
            .expect("MemoryInit: RAM 结束地址溢出"),
    );

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();

    let free_start = kernel_end.align_up();
    assert!(mem_size > 0, "MemoryInit: RAM 大小为 0 (start={mem_start})");
    assert!(
        kernel_start >= mem_start && kernel_end <= mem_end,
        "MemoryInit: kernel [{kernel_start}, {kernel_end}) 不在 RAM [{mem_start}, {mem_end}) 内"
    );
    assert!(
        free_start <= mem_end,
        "MemoryInit: free_start {free_start} 超出 RAM 结束地址 {mem_end}"
    );
    let free_size = mem_end - free_start;

    // 段计算——注意 text 段从 kernel_start 开始，不是 mem_start。
    // [mem_start, kernel_start) 是固件区域（OpenSBI 等），保留背景层 kernel_rw。
    let text_pages = (text_end - kernel_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (free_start - rodata_end) / config::PAGE_SIZE;

    // SAFETY: 范围有效、页对齐、互不重叠、仅调用一次
    unsafe {
        frame_allocator::init(
            free_start,
            free_size,
            &[
                (kernel_start, text_pages),
                (text_end, rodata_pages),
                (rodata_end, data_pages),
            ],
        );
    }

    {
        let extend_size = config::KERNEL_HEAP_SIZE - config::BOOTSTRAP_HEAP_SIZE;
        let extend_pages = extend_size / config::PAGE_SIZE;
        let heap_frames =
            frame_allocator::AllocatedFrames::alloc(extend_pages).expect("heap extend: 帧分配失败");
        let heap_start = heap_frames.start_paddr().to_virt().as_usize();
        // SAFETY: 帧刚分配，identity-mapped，无其他引用
        unsafe { heap_crate::extend(heap_start, extend_size) };
        // 堆帧永久持有——与内核同生命周期
        core::mem::forget(heap_frames);
    }

    let pt = PageTable::create();
    paging::init_kernel_page_table(pt);

    // 背景层必须先于权限覆盖——覆盖操作要求 PTE 已存在
    {
        paging::kernel_page_table().identity_map_range(mem_start, mem_end, PteFlags::kernel_rw());
    }

    // 覆盖层——[mem_start, kernel_start) 保持 kernel_rw（固件区域）
    let segments: [(PhysAddr, usize, PteFlags); 3] = [
        (kernel_start, text_pages, PteFlags::kernel_rx()),
        (text_end, rodata_pages, PteFlags::kernel_ro()),
        (rodata_end, data_pages, PteFlags::kernel_rw()),
    ];

    for (start, page_count, flags) in segments {
        let va = start.to_virt();
        paging::kernel_page_table().update_range_flags(va, page_count, flags);
        log::debug!(
            "MemoryInit: segment {}: {} pages, {:?}",
            va,
            page_count,
            flags
        );
    }

    log::info!(
        "MemoryInit: fw {}-{} (RW bg), code {}-{} (RX), rodata {}-{} (RO), data {}-{} (RW), free {}-{} (RW bg)",
        mem_start,
        kernel_start,
        kernel_start,
        text_end,
        text_end,
        rodata_end,
        rodata_end,
        free_start,
        free_start,
        mem_end
    );
}

/// 从核内存初始化——复用主核页表并激活分页。
pub fn init_smp(activate: impl FnOnce(&PageTable)) {
    let pt = paging::kernel_page_table();
    activate(pt);
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        per_cpu::current_core_id()
    );
}
