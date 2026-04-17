//! 分页子系统——页表管理 + 仿射类型帧所有权 + MMIO 映射。
//!
//! # 在内存子系统中的定位
//!
//! 本 crate 是内存子系统的**机制层**——提供页表操作和权限管理原语，
//! 但不决定"何时"或"为什么"映射。策略层（`memory` crate）编排初始化和 MMIO，
//! 应用层通过 [`OwnedPages`] / [`MmioRegion`] RAII 类型安全使用。
//!
//! ```text
//! memory (策略: 何时映射)
//!    │
//!    ▼
//! paging (机制: 如何映射)  ← 本 crate
//!    │
//!    ├── frame_allocator (帧分配)
//!    ├── page_table_entry (PTE 编解码)
//!    └── tlb (TLB 刷新)
//! ```
//!
//! # 核心类型
//!
//! - [`PageTable`]：多级基数树页表，管理 PTE 的创建和更新
//! - [`OwnedPages`]：仿射类型权限守卫——持有帧所有权 + 管理 PTE 权限覆盖
//! - [`mmio::MmioRegion`]：永久 MMIO 映射 + volatile 寄存器访问
//!
//! # 使用方式
//!
//! [`PageTable`] 的写操作（`create_pte`、`update_pte` 等）为 `pub`，
//! 但正常使用时应通过 `OwnedPages` / `MmioRegion` 等 RAII 类型调用，
//! 确保帧所有权和权限通过仿射类型管理。
//!
//! PTE 编解码由 [`page_table_entry`] crate 提供（本 crate re-export）。

#![no_std]

extern crate alloc;

pub mod error;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

pub use arch::{ENTRIES_PER_TABLE, INDEX_BITS, INDEX_MASK, LEVEL_SHIFTS, page_size_at_level};

pub mod table;
pub use table::PageTable;

pub mod mapping;
pub use mapping::OwnedPages;

pub mod mmio;

/// 全局内核页表——SAS 架构下只有一张页表，所有权限覆盖共用。
///
/// 锁已移入 `PageTable` 内部（`nodes` 字段），外层无需再加锁。
/// hot path（`get_mapping`、`update_pte`、`walk_to_leaf`）完全无锁，
/// 仅 `create_pte` 的慢路径（分配中间节点）需要内部锁。
static KERNEL_PAGE_TABLE: spin::Once<PageTable> = spin::Once::new();

/// 初始化全局内核页表——消费 `PageTable` 的所有权，写入静态存储。
///
/// 仅在启动时调用一次。
pub fn init_kernel_page_table(pt: PageTable) {
    KERNEL_PAGE_TABLE.call_once(|| pt);
}

/// 获取全局内核页表引用。
///
/// 未初始化时 panic。
pub fn kernel_page_table() -> &'static PageTable {
    KERNEL_PAGE_TABLE
        .get()
        .expect("kernel page table not initialized")
}

/// 分配一个零初始化的页表节点帧。
fn alloc_node_frame() -> Result<frame_allocator::AllocatedFrames, error::PagingError> {
    let frame = frame_allocator::AllocatedFrames::alloc_one().map_err(|e| {
        log::warn!("页表节点帧分配失败: {:?}", e);
        error::PagingError::AllocationFailed
    })?;
    // 页表节点需要全零初态（无效 PTE = 0），由此处负责清零
    // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚分配，无其他引用
    unsafe {
        core::ptr::write_bytes(
            frame.start_paddr().to_virt().as_mut_ptr::<u8>(),
            0,
            config::PAGE_SIZE,
        );
    }
    Ok(frame)
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
///
/// 此函数因依赖 [`memory_types::VirtAddr`] 而无法放入 `arch` crate
/// （`memory_types` 已依赖 `arch`，反向依赖会形成循环）。
#[inline]
pub fn vpn_index(va: memory_types::VirtAddr, level: usize) -> usize {
    (va.as_usize() >> LEVEL_SHIFTS[level]) & INDEX_MASK
}
