//! 内存子系统初始化——主核 / 从核。
//!
//! 初始化完成后页表包含两层权限：
//! 1. **背景层**（identity_map_range）：全部物理内存 identity-map 为 kernel_rw
//! 2. **覆盖层**（OwnedPages）：内核段各自权限覆盖背景层
//!
//! 初始化顺序（ADR-006）：先背景映射全部物理内存，再 OwnedPages 覆盖内核段权限。
//!
//! 内核段的 OwnedPages 通过 `mem::forget` 永久持有——
//! 这些帧与内核同生命周期，永远不归还分配器，权限永远不恢复。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

/// 主核内存初始化——堆、帧分配器、页表、权限覆盖。
///
/// 内核段的 OwnedPages 通过 `mem::forget` 永久持有：
/// - 权限已设置到页表中，无需再访问 OwnedPages
/// - AllocatedFrames 不被 drop → 帧不会归还 buddy 分配器
/// - OwnedPages 不被 drop → 权限不会恢复为 kernel_rw
pub fn init() {
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

    let free_start = kernel_end.align_up();
    let free_size = mem_size - (free_start - mem_start);

    // data 段只覆盖到 free_start，不含空闲帧区域
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (free_start - rodata_end) / config::PAGE_SIZE;

    // SAFETY: 范围有效、页对齐、互不重叠、仅调用一次
    let reserved = unsafe {
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

    let pt = PageTable::create().expect("创建内核页表失败");
    paging::init_kernel_page_table(pt);

    // 背景层：必须先于 OwnedPages——OwnedPages::new 通过 update_flags 更新权限，
    // 要求 PTE 已存在。
    {
        let mem_end = mem_start + mem_size;
        let mut guard = paging::kernel_page_table().lock();
        guard.identity_map_range(mem_start, mem_end, PteFlags::kernel_rw());
    }

    // 覆盖层：内核段各自权限覆盖背景层
    let segments: [(PhysAddr, PteFlags); 3] = [
        (mem_start, PteFlags::kernel_rwx()),
        (text_end, PteFlags::kernel_ro()),
        (rodata_end, PteFlags::kernel_rw()),
    ];

    for ((start, flags), frames) in segments.into_iter().zip(reserved) {
        let va = memory_types::VirtAddr::new(start.as_usize());
        let mapping = paging::OwnedPages::new(frames, flags);
        log::debug!(
            "MemoryInit: segment {}: {} pages, {:?}",
            va,
            mapping.page_count(),
            flags
        );
        // 内核段与内核同生命周期——权限已写入页表，
        // forget 阻止 Drop（不恢复权限、不归还帧）
        core::mem::forget(mapping);
    }

    log::info!(
        "MemoryInit: code {}-{} (RWX), rodata {}-{} (RO), data {}-{} (RW), free {}-{} (RW bg)",
        mem_start,
        text_end,
        text_end,
        rodata_end,
        rodata_end,
        free_start,
        free_start,
        mem_start + mem_size
    );
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
