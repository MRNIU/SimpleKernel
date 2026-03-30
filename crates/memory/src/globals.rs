//! 全局内存状态——`MemoryInfo`、内核页表、内核地址空间。

use address::PhysAddr;

#[cfg(any(test, target_os = "none"))]
use alloc::sync::Arc;
#[cfg(any(test, target_os = "none"))]
use sync_crate::SpinLock;

#[cfg(any(test, target_os = "none"))]
use crate::page_table::PageTable;
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

/// 全局内核页表（`Arc` 共享引用——内核与所有内核线程共享同一页表）。
#[cfg(any(test, target_os = "none"))]
static KERNEL_PAGE_TABLE: spin::Once<Arc<SpinLock<PageTable>>> = spin::Once::new();

/// 全局内核地址空间。
#[cfg(any(test, target_os = "none"))]
static KERNEL_ADDRESS_SPACE: spin::Once<SpinLock<AddressSpace>> = spin::Once::new();

/// 将构建完成的内核页表存入全局 `KERNEL_PAGE_TABLE`。
#[cfg(any(test, target_os = "none"))]
pub fn store_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| Arc::new(SpinLock::new(pt, "kernel_pt")));
}

/// 获取全局内核页表的 `Arc` 引用；初始化前返回 `None`。
///
/// 内核页表通过 `Arc` 共享——内核地址空间、`MappedPages`、
/// 以及未来的用户进程页表都持有各自的 `Arc<SpinLock<PageTable>>`。
/// 用户进程退出时其 `Arc` 引用计数归零，页表自动释放。
#[cfg(any(test, target_os = "none"))]
pub fn kernel_page_table() -> Option<Arc<SpinLock<PageTable>>> {
    KERNEL_PAGE_TABLE.get().cloned()
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
