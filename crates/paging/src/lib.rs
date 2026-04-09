//! 分页子系统——页表 + 仿射类型映射所有权。
//!
//! 本 crate 合并了原 `page_table` 和 `mapped_pages` 两个 crate，
//! 实现了 **编译期强制的仿射类型安全**：
//!
//! - [`PageTable`] 的写操作（`map_page`、`unmap_page` 等）为 `pub`，
//!   但正常使用时应通过 `OwnedPages` / `MmioRegion` 等 RAII 类型调用，
//!   确保映射的创建与销毁通过仿射类型管理。
//!
//! PTE 编解码由 [`page_table_entry`] crate 提供。
//! 帧分配通过 [`NodeFrame`] 类型别名（[`KernelNodeFrame`]）使用物理帧分配器，
//! 页表和映射类型均为非泛型。

#![no_std]

extern crate alloc;

use memory_types::PhysAddr;

pub mod error;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

// 页表几何常量——定义在 arch crate，此处 re-export 供下游使用。
pub use arch::{ENTRIES_PER_TABLE, INDEX_BITS, INDEX_MASK, LEVEL_SHIFTS, page_size_at_level};

pub mod table;
pub use table::PageTable;

pub mod mapping;
pub use mapping::OwnedPages;

pub mod mmio;

/// 全局内核页表——SAS 架构下只有一张页表，所有映射共用。
///
/// `spin::Once` 在 `.bss` 中静态分配内存，运行时通过 [`init_kernel_page_table`]
/// 一次性写入 `SpinLock<PageTable>`。无需 `Box::leak`，无需 `unsafe`。
static KERNEL_PAGE_TABLE: spin::Once<sync_crate::SpinLock<PageTable>> = spin::Once::new();

/// 初始化全局内核页表——消费 `PageTable` 的所有权，写入静态存储。
///
/// 仅在启动时调用一次。重复调用时 `spin::Once` 忽略后续 `call_once`。
pub fn init_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| {
        sync_crate::SpinLock::new(pt, "kernel_pt", sync_crate::lock_level::KERNEL_PT)
    });
}

/// 获取全局内核页表引用。
///
/// 未初始化时 panic。
pub fn kernel_page_table() -> &'static sync_crate::SpinLock<PageTable> {
    KERNEL_PAGE_TABLE
        .get()
        .expect("kernel page table not initialized")
}

/// 页表节点帧的统一接口。
///
/// 通过 [`NodeFrame`] 类型别名（[`KernelNodeFrame`]）选择具体实现。
/// 页表、映射等类型直接使用 `NodeFrame`，不再泛型化。
pub trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, error::PagingError>;
    /// 获取帧的物理地址（identity mapping）。
    fn paddr(&self) -> PhysAddr;
}

/// 裸机页表节点帧——包装 `AllocatedFrames`。
///
/// Newtype 用于为外部类型 `AllocatedFrames` 实现本 crate 的 `NodeFrameOps`。
pub struct KernelNodeFrame(frame_allocator::AllocatedFrames);

impl NodeFrameOps for KernelNodeFrame {
    fn alloc() -> Result<Self, error::PagingError> {
        let frames = frame_allocator::AllocatedFrames::alloc_one().map_err(|e| {
            log::warn!("页表节点帧分配失败: {:?}", e);
            error::PagingError::AllocationFailed
        })?;
        // 页表节点帧必须清零——残留垃圾会被当作有效 PTE 解读。
        // 节点帧不经过 OwnedPages::map，
        // 需在此手动清零。
        // SAFETY: 帧刚分配，无其他引用；boot 阶段 paging 未激活，
        // PA 可直接访问；paging 激活后节点帧在已映射区域内分配。
        unsafe {
            core::ptr::write_bytes(
                frames.start_paddr().to_virt().as_mut_ptr::<u8>(),
                0,
                config::PAGE_SIZE,
            );
        }
        Ok(Self(frames))
    }
    fn paddr(&self) -> PhysAddr {
        self.0.start_paddr()
    }
}

/// 页表节点帧类型——[`KernelNodeFrame`]（物理帧分配器）。
pub type NodeFrame = KernelNodeFrame;

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
///
/// 此函数因依赖 [`memory_types::VirtAddr`] 而无法放入 `arch` crate
/// （`memory_types` 已依赖 `arch`，反向依赖会形成循环）。
#[inline]
pub fn vpn_index(va: memory_types::VirtAddr, level: usize) -> usize {
    (va.as_usize() >> LEVEL_SHIFTS[level]) & INDEX_MASK
}
