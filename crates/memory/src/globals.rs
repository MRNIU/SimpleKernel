//! 全局内存状态——`MemoryInfo`、内核地址空间。

use memory_types::PhysAddr;

use sync_crate::SpinLock;

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

/// 全局内核地址空间。
static KERNEL_ADDRESS_SPACE: spin::Once<SpinLock<AddressSpace>> = spin::Once::new();

/// 将构建完成的内核地址空间存入全局 `KERNEL_ADDRESS_SPACE`。
pub fn store_kernel_address_space(addr_space: AddressSpace) {
    KERNEL_ADDRESS_SPACE
        .call_once(|| SpinLock::new(addr_space, "kernel_as", sync_crate::lock_level::KERNEL_AS));
}

/// 获取全局内核地址空间的引用；初始化前返回 `None`。
pub fn kernel_address_space() -> Option<&'static SpinLock<AddressSpace>> {
    KERNEL_ADDRESS_SPACE.get()
}
