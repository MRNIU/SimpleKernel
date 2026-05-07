//! 分页子系统——多级页表管理。
//!
//! MMIO 类型化包装位于 `memory::mmio`（业务层），此 crate 只提供页表原语。

#![no_std]

extern crate alloc;

pub use page_table_entry::{PageTableEntry, PteFlags, PteFlagsOps, PteOps};

pub use arch::{ENTRIES_PER_TABLE, INDEX_BITS, INDEX_MASK, LEVEL_SHIFTS, page_size_at_level};

pub mod table;
pub use table::PageTable;

/// 全局内核页表——SAS 架构下只有一张页表，所有权限覆盖共用。
///
/// 锁已移入 `PageTable` 内部（`nodes` 字段），外层无需再加锁。
/// hot path（`get_mapping`、`update_range_flags`、`walk_to_leaf`）完全无锁，
/// 仅 `identity_map_range` 的慢路径（分配中间节点）需要内部锁。
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
///
/// # Panics
///
/// 帧分配失败时 panic——页表节点分配发生在 boot 时，OOM 是内核 bug。
fn alloc_node_frame() -> frame_allocator::AllocatedFrames {
    let frame = frame_allocator::AllocatedFrames::alloc_one()
        .expect("页表节点帧分配失败（boot-time OOM 是内核 bug）");
    // 页表节点需要全零初态（无效 PTE = 0），由此处负责清零
    // SAFETY: identity mapping 下 PA.to_virt() 有效；帧刚分配，无其他引用
    unsafe {
        core::ptr::write_bytes(
            frame.start_paddr().to_virt().as_mut_ptr::<u8>(),
            0,
            config::PAGE_SIZE,
        );
    }
    frame
}

/// 从虚拟地址中提取第 `level` 级的 VPN 索引。
///
/// 此函数因依赖 [`memory_types::VirtAddr`] 而无法放入 `arch` crate
/// （`memory_types` 已依赖 `arch`，反向依赖会形成循环）。
#[inline]
pub fn vpn_index(va: memory_types::VirtAddr, level: usize) -> usize {
    (va.as_usize() >> LEVEL_SHIFTS[level]) & INDEX_MASK
}
