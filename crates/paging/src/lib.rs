//! 分页子系统——页表 + 仿射类型映射所有权。
//!
//! 本 crate 合并了原 `page_table` 和 `mapped_pages` 两个 crate，
//! 实现了 **编译期强制的仿射类型安全**：
//!
//! - [`PageTable`] 的写操作（`map_page`、`unmap_page` 等）为 `pub(crate)`，
//!   外部只能通过 `MappedPages` / `MmioRegion` 等 RAII 类型调用，
//!   确保映射的创建与销毁通过仿射类型管理。
//!
//! PTE 编解码由 [`page_table_entry`] crate 提供。
//! 页表节点帧直接使用 [`frame_allocator::AllocatedFrames`]。

#![no_std]

extern crate alloc;

pub mod error;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

// 页表几何常量——定义在 arch crate，此处 re-export 供下游使用。
pub use arch::{ENTRIES_PER_TABLE, INDEX_BITS, INDEX_MASK, LEVEL_SHIFTS, page_size_at_level};

pub mod table;
pub use table::PageTable;

pub mod mapping;
pub use mapping::MappedPages;

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

/// 分配一个页表节点帧，返回 `AllocatedFrames`。
fn alloc_node_frame() -> Result<frame_allocator::AllocatedFrames, error::PagingError> {
    frame_allocator::AllocatedFrames::alloc_one().map_err(|e| {
        log::warn!("页表节点帧分配失败: {:?}", e);
        error::PagingError::AllocationFailed
    })
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
///
/// 此函数因依赖 [`memory_types::VirtAddr`] 而无法放入 `arch` crate
/// （`memory_types` 已依赖 `arch`，反向依赖会形成循环）。
#[inline]
pub fn vpn_index(va: memory_types::VirtAddr, level: usize) -> usize {
    (va.as_usize() >> LEVEL_SHIFTS[level]) & INDEX_MASK
}
