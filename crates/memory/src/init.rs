//! 内存子系统初始化——主核 / 从核。

use address::{PhysAddr, VirtAddr};
use paging::{PageTable, PteFlags, PteFlagsOps};

use crate::vma::AddressSpace;

/// 主核内存初始化——返回内核地址空间（包含所有内核段映射）。
///
/// 初始化顺序：堆 → 帧分配器 → 页分配器 → 页表 → 分段映射。
pub fn init() -> AddressSpace {
    // SAFETY: 在任何堆分配之前调用，且仅调用一次（由启动流程保证）
    unsafe { heap_crate::init() };

    let info = crate::globals::MEMORY_INFO
        .get()
        .expect("MEMORY_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    // SAFETY: alloc_start 页对齐，内存区域不与堆重叠，仅调用一次
    unsafe { frame_allocator::init(alloc_start, alloc_size) };

    // 初始化虚拟页分配器——SAS 下 VA == PA，用整段物理内存范围
    // SAFETY: 虚拟地址范围有效，仅调用一次
    unsafe {
        page_allocator::init(VirtAddr::new(mem_start.as_usize()), mem_size);
    }
    // 扣除内核镜像占用的虚拟地址区域（由分段映射管理，不可被自动分配）
    page_allocator::reserve(VirtAddr::new(mem_start.as_usize()), mem_size);

    // 创建页表——Box::leak 产出 'static 引用
    let pt = PageTable::create().expect("创建内核页表失败");
    let pt_lock = sync_crate::SpinLock::new(pt, "kernel_pt");
    let pt_static: &'static _ = alloc::boxed::Box::leak(alloc::boxed::Box::new(pt_lock));
    // SAFETY: pt_static 是 'static 引用
    unsafe { paging::set_kernel_page_table(pt_static) };

    let mut kernel_as = AddressSpace::new();

    // SAFETY: 链接器定义的符号
    unsafe extern "C" {
        static __etext: u8;
        static __erodata: u8;
    }
    let text_end = PhysAddr::new(unsafe { &__etext as *const u8 as usize }).align_up();
    let rodata_end = PhysAddr::new(unsafe { &__erodata as *const u8 as usize }).align_up();
    let mem_end = mem_start + mem_size;

    // 分段映射（通过 VMA，自动选择大页）：
    // [mem_start, text_end)    → RWX（.boot 段混合了 code+data）
    // [text_end, rodata_end)   → RO（.rodata）
    // [rodata_end, mem_end)    → RW（.data + .bss + 空闲内存）
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(mem_start.as_usize()),
            VirtAddr::new(text_end.as_usize()),
            PteFlags::kernel_rwx(),
        )
        .expect("映射内核代码区域失败");
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(text_end.as_usize()),
            VirtAddr::new(rodata_end.as_usize()),
            PteFlags::kernel_ro(),
        )
        .expect("映射内核只读数据区域失败");
    kernel_as
        .mmap_identity_range(
            VirtAddr::new(rodata_end.as_usize()),
            VirtAddr::new(mem_end.as_usize()),
            PteFlags::kernel_rw(),
        )
        .expect("映射内核数据+空闲区域失败");

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
