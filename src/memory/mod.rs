pub mod address;
pub mod page_table;

#[cfg(not(test))]
pub mod frame;
#[cfg(not(test))]
pub mod heap;

#[cfg(not(test))]
use address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use page_table::{PageFlags, PageTable};

#[cfg(not(test))]
use crate::sync::SpinLock;

/// 全局内核页表 —— `map_mmio()` 和 `memory_init_smp()` 共享。
#[cfg(not(test))]
static KERNEL_PAGE_TABLE: spin::Once<SpinLock<PageTable>> = spin::Once::new();

/// Identity mapping: phys_to_virt is identity for now.
#[cfg(not(test))]
#[allow(dead_code)]
pub fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    VirtAddr::new(pa.as_usize())
}

/// Identity mapping: virt_to_phys is identity for now.
#[cfg(not(test))]
#[allow(dead_code)]
pub fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    PhysAddr::new(va.as_usize())
}

/// Map a range of pages with identity mapping (VA == PA).
#[cfg(not(test))]
fn identity_map_range(
    pt: &mut PageTable,
    start: PhysAddr,
    end: PhysAddr,
    flags: PageFlags,
) -> crate::error::KResult<()> {
    let mut addr = start.align_down();
    let end_aligned = end.align_up();
    while addr.as_usize() < end_aligned.as_usize() {
        pt.map_page(VirtAddr::new(addr.as_usize()), addr, flags)?;
        addr = addr + crate::config::PAGE_SIZE;
    }
    Ok(())
}

/// 主核内存初始化 — BSP 调用。
///
/// 1. 初始化堆分配器（静态 BSS 区域）
/// 2. 初始化帧分配器（从 FDT 获取物理内存范围）
/// 3. 创建内核页表，identity map 整个 RAM
/// 4. 映射分页激活前必须就绪的架构特定 MMIO
/// 5. 激活分页，将页表存入全局
#[cfg(not(test))]
pub fn memory_init() {
    // Step 1: Heap — must come first so we can use Vec/Box
    unsafe { heap::heap_init() };

    // Step 2: Frame allocator
    let info = crate::per_cpu::BASIC_INFO
        .get()
        .expect("BASIC_INFO not initialized");
    let mem_start = info.physical_memory_addr;
    let mem_size = info.physical_memory_size;
    let kernel_end = info.kernel_addr + info.kernel_size;

    // 可分配区域从内核镜像末尾（页对齐）开始
    let alloc_start = kernel_end.align_up();
    let alloc_size = mem_size - (alloc_start - mem_start);

    unsafe { frame::frame_init(alloc_start, alloc_size) };

    // Step 3: 创建内核页表，identity map 整个物理内存（RWX，初期不区分代码/数据段）
    let mut pt = PageTable::new().expect("failed to create kernel page table");

    identity_map_range(
        &mut pt,
        mem_start,
        mem_start + mem_size,
        PageFlags::kernel_rwx(),
    )
    .expect("failed to identity-map memory");

    log::info!(
        "MemoryInit: kernel mapped {}-{}",
        mem_start,
        mem_start + mem_size
    );

    // Step 4: 映射分页激活前必须就绪的架构特定 MMIO
    map_early_mmio(&mut pt);

    // Step 5: 激活分页
    // SAFETY: 页表已覆盖所有内核代码/数据（RAM identity map）及早期 MMIO
    unsafe { page_table::activate_page_table(&pt) };
    log::info!("MemoryInit: paging enabled");

    // 将页表存入全局，供 map_mmio() / memory_init_smp() 使用
    // pt 所有权转移至 KERNEL_PAGE_TABLE，页表帧永不释放（内核页表生命周期无限）
    KERNEL_PAGE_TABLE.call_once(|| SpinLock::new(pt, "kernel_pt"));
}

/// 映射分页激活前必须就绪的控制台 MMIO。
///
/// - RISC-V：console 通过 SBI ecall（M-mode，绕过 MMU），无需 MMIO 映射。
/// - AArch64：console 直接读写 PL011 MMIO，必须在激活分页前完成映射。
#[cfg(not(test))]
fn map_early_mmio(pt: &mut PageTable) {
    #[cfg(target_arch = "aarch64")]
    {
        // PL011 UART @ 0x0900_0000, 1 页 —— console 直接 MMIO 访问
        identity_map_range(
            pt,
            PhysAddr::new(0x0900_0000),
            PhysAddr::new(0x0900_1000),
            PageFlags::kernel_rw(),
        )
        .expect("failed to map PL011 UART MMIO");
        log::info!("MemoryInit: mapped PL011 UART @ 0x09000000");
    }
}

/// 从核内存初始化 — 加载主核创建的内核页表到本核 MMU。
#[cfg(not(test))]
pub fn memory_init_smp() {
    let kpt = KERNEL_PAGE_TABLE
        .get()
        .expect("KERNEL_PAGE_TABLE not initialized");
    let guard = kpt.lock();
    // SAFETY: 主核已验证页表正确性；从核仅将同一根地址写入本核 MMU 寄存器
    unsafe { page_table::activate_page_table(&*guard) };
    log::info!(
        "MemoryInitSMP: paging enabled on core {}",
        crate::per_cpu::current_core_id()
    );
}

/// 映射 MMIO 区域，返回虚拟地址（当前为 identity mapping：VA == PA）。
///
/// 将 `[paddr, paddr+size)` identity-map 进内核页表，刷新 TLB 后返回对应 VA。
/// P4 的 PLIC/GIC 初始化通过此函数映射中断控制器寄存器。
#[cfg(not(test))]
pub fn map_mmio(paddr: PhysAddr, size: usize) -> crate::error::KResult<VirtAddr> {
    let kpt = KERNEL_PAGE_TABLE
        .get()
        .ok_or(crate::error::ErrorCode::VmInvalidPageTable)?;
    let mut guard = kpt.lock();
    identity_map_range(&mut *guard, paddr, paddr + size, PageFlags::kernel_rw())?;
    // 添加新映射后刷新 TLB，确保后续访问命中新条目
    // SAFETY: sfence.vma / tlbi 是特权指令，在对应特权级下有效
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!("sfence.vma")
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("tlbi vmalle1", "dsb sy", "isb")
    };
    Ok(VirtAddr::new(paddr.as_usize()))
}
