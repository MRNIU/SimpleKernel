//! 内存子系统初始化——主核 / 从核。
//!
//! 初始化完成后页表包含两层权限：
//! 1. **背景层**（identity_map_range）：全部物理内存 identity-map 为 kernel_rw
//! 2. **覆盖层**（OwnedPages）：内核段各自权限覆盖背景层
//!
//! 初始化顺序（ADR-006）：先背景映射全部物理内存，再 OwnedPages 覆盖内核段权限。

use memory_types::PhysAddr;
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段权限覆盖）。
///
/// 初始化顺序：堆 -> 帧分配器 -> 页表 -> 背景映射 -> 内核段权限覆盖。
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

    // 空闲内存 = 内核之后的部分
    let free_start = kernel_end.align_up();
    let free_size = mem_size - (free_start - mem_start);

    // 各段页数——data 段只覆盖到 free_start，不含空闲帧区域
    let text_pages = (text_end - mem_start) / config::PAGE_SIZE;
    let rodata_pages = (rodata_end - text_end) / config::PAGE_SIZE;
    let data_pages = (free_start - rodata_end) / config::PAGE_SIZE;

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

    // 背景映射：全部物理内存 identity-map 为 kernel_rw（背景层）。
    // 必须先于 OwnedPages 执行——OwnedPages::new 调用 set_page_flags，
    // 要求 PTE 已存在（幂等更新 flags）。
    {
        let mem_end = mem_start + mem_size;
        let mut guard = paging::kernel_page_table().lock();
        guard.identity_map_range(mem_start, mem_end, PteFlags::kernel_rw());
    }

    let mut kernel_as = AddressSpace::new();

    // 内核段权限覆盖：.text(RWX) / .rodata(RO) / .data(RW)
    // reserved 的元素顺序与传入 init 的 reserved 参数顺序一致。
    // identity mapping: VA == PA，OwnedPages::new 从 PA 推导 VA。
    let segments: [PteFlags; 3] = [
        PteFlags::kernel_rwx(),
        PteFlags::kernel_ro(),
        PteFlags::kernel_rw(),
    ];

    let seg_starts = [mem_start, text_end, rodata_end];

    for (i, flags) in segments.into_iter().enumerate() {
        let frames = reserved.remove(0);
        let va = memory_types::VirtAddr::new(seg_starts[i].as_usize());
        let mapping = paging::OwnedPages::new(frames, flags);
        kernel_as.register_kernel_mapping(va, mapping);
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
