//! 全局内存状态——`MemoryInfo`、内核页表、内核地址空间、MMIO 便利映射。

use address::PhysAddr;
#[cfg(any(test, target_os = "none"))]
use address::VirtAddr;

#[cfg(any(test, target_os = "none"))]
use sync_crate::SpinLock;

#[cfg(any(test, target_os = "none"))]
use crate::page_table::{PageTable, PteFlagsOps};
#[cfg(any(test, target_os = "none"))]
use crate::vma::AddressSpace;

/// 内核启动时从 FDT 解析出的内存布局信息。
///
/// 在 `early_init()` 中通过 `MEMORY_INFO.call_once()` 填充，
/// 之后由内存子系统只读访问。
pub struct MemoryInfo {
    /// 物理内存起始地址
    pub physical_memory_addr: PhysAddr,
    /// 物理内存大小（字节）
    pub physical_memory_size: usize,
    /// 内核镜像起始物理地址
    pub kernel_addr: PhysAddr,
    /// 内核镜像大小（字节）
    pub kernel_size: usize,
}

/// 全局内存布局信息（一次性初始化，之后只读）。
pub static MEMORY_INFO: spin::Once<MemoryInfo> = spin::Once::new();

/// 全局内核页表。
#[cfg(any(test, target_os = "none"))]
static KERNEL_PAGE_TABLE: spin::Once<SpinLock<PageTable>> = spin::Once::new();

/// 全局内核地址空间。
#[cfg(any(test, target_os = "none"))]
static KERNEL_ADDRESS_SPACE: spin::Once<SpinLock<AddressSpace>> = spin::Once::new();

/// 将构建完成的内核页表存入全局 `KERNEL_PAGE_TABLE`。
#[cfg(any(test, target_os = "none"))]
pub fn store_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| SpinLock::new(pt, "kernel_pt"));
}

/// 获取全局内核页表的引用；初始化前返回 `None`。
#[cfg(any(test, target_os = "none"))]
pub fn kernel_page_table() -> Option<&'static SpinLock<PageTable>> {
    KERNEL_PAGE_TABLE.get()
}

/// 将构建完成的内核地址空间存入全局 `KERNEL_ADDRESS_SPACE`。
#[cfg(any(test, target_os = "none"))]
pub fn store_kernel_address_space(addr_space: AddressSpace) {
    KERNEL_ADDRESS_SPACE.call_once(|| SpinLock::new(addr_space, "kernel_as"));
}

/// 获取全局内核地址空间的引用；初始化前返回 `None`。
#[cfg(any(test, target_os = "none"))]
pub fn kernel_address_space() -> Option<&'static SpinLock<AddressSpace>> {
    KERNEL_ADDRESS_SPACE.get()
}

/// 将 MMIO 物理地址区间 identity-map 到内核页表，返回对应虚拟地址。
///
/// 映射标记为永久（drop 时不 unmap），并在内核地址空间中注册 VMA 记录。
/// 如需 RAII 管理的 MMIO 映射，请使用 [`crate::mmio::MmioRegion::map`]。
///
/// # Errors
///
/// 内核页表未初始化或映射冲突时返回错误。
#[cfg(any(test, target_os = "none"))]
pub fn map_mmio(paddr: PhysAddr, size: usize) -> Result<VirtAddr, crate::error::MemoryError> {
    let region = crate::mmio::MmioRegion::map(paddr, size)?;
    let vaddr = region.base();
    let region_size = region.size();
    let _permanent = region.into_permanent();

    // 在内核地址空间中注册 MMIO 区域
    if let Some(kas) = KERNEL_ADDRESS_SPACE.get() {
        let _ = kas.lock().register_existing(
            vaddr,
            region_size,
            crate::page_table::PteFlags::kernel_device(),
            crate::vma::VmaKind::Identity,
        );
    }

    Ok(vaddr)
}
