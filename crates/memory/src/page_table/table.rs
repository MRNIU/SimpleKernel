//! 多级页表——walk / map / unmap 逻辑。
//!
//! 裸机和宿主机测试共享同一份 walk 实现，
//! 通过 `#[cfg]` 选择页表节点的帧类型（裸机用 buddy allocator，测试用堆分配）。
//!
//! # TODO
//!
//! - **中间节点回收**：当前 `unmap_page` 不回收空的中间页表节点。
//!   如果大量映射后 unmap，中间节点帧会一直保留直到整个 `PageTable` drop。
//!   Linux 的 `free_pgtables()` 在 VMA 销毁时递归回收空中间页；
//!   Theseus 在 unmap 时检查中间节点是否全空并回收。
//!   后续可在 `unmap_page` 后检查同级所有 PTE 是否均为空，
//!   若是则清除上级 PTE 并归还该中间节点帧。

extern crate alloc;

use alloc::vec::Vec;

use super::{Level0, PageTableEntry, PteFlags, PteFlagsOps, PteOps, Table, vpn_index};
use crate::error::MemoryError;
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

#[cfg(target_os = "none")]
type NodeFrame = crate::frame::AllocatedFrames;

#[cfg(test)]
struct NodeFrame {
    ptr: *mut u8,
    layout: core::alloc::Layout,
}

#[cfg(test)]
impl Drop for NodeFrame {
    fn drop(&mut self) {
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}

/// 分配一个页表节点帧（零初始化）。
fn alloc_node() -> Result<NodeFrame, MemoryError> {
    #[cfg(target_os = "none")]
    {
        crate::frame::AllocatedFrames::alloc_one()
    }
    #[cfg(test)]
    {
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("NodeFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "NodeFrame: allocation failed");
        Ok(NodeFrame { ptr, layout })
    }
}

/// 获取节点帧的物理地址（裸机 identity mapping）或堆地址（测试）。
fn node_paddr(frame: &NodeFrame) -> PhysAddr {
    #[cfg(target_os = "none")]
    {
        frame.start_paddr()
    }
    #[cfg(test)]
    {
        PhysAddr::new(frame.ptr as usize)
    }
}

/// 从物理地址构造 `Table<Level0>` 用于 walker 内部。
///
/// walker 在运行时循环中无法使用编译期类型参数区分层级，
/// 统一使用 `Level0` 是因为当前所有层级的 entries 数量相同（均从 PAGE_SIZE 推导）。
/// 索引计算通过 [`vpn_index`] + [`LEVEL_INFO`] 查表实现，不依赖 `L::ENTRIES`。
///
/// # Safety
/// `paddr` 必须指向有效、页对齐的帧（裸机 identity mapping 或测试堆地址）。
#[inline]
unsafe fn table_at(paddr: PhysAddr) -> Table<Level0> {
    // SAFETY: 调用方保证 paddr 有效
    unsafe { Table::<Level0>::from_paddr(paddr) }
}

/// 多级页表。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧。
pub struct PageTable {
    root_paddr: PhysAddr,
    /// 持有根帧所有权，阻止帧被释放——字段本身不直接访问。
    #[expect(dead_code, reason = "仅用于持有所有权，通过 root_paddr 访问")]
    root: NodeFrame,
    frames: Vec<NodeFrame>,
}

/// Walker 行为——遇到无效中间节点时的策略。
enum WalkAction {
    /// 只读遍历，无效时返回错误
    ReadOnly,
    /// 自动分配中间节点
    CreateIntermediate,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, MemoryError> {
        let root = alloc_node()?;
        let root_paddr = node_paddr(&root);
        Ok(Self {
            root_paddr,
            root,
            frames: Vec::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    #[cfg(target_os = "none")]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    /// 统一 walker——遍历到 `target_level` 层级的 PTE 并返回裸指针。
    ///
    /// 从根（最高级）向下遍历至 `target_level`，中间层级根据 `action` 决定
    /// 遇到无效 PTE 时是分配新节点还是返回错误。
    ///
    /// 遍历过程中若遇到叶节点（大页），返回 `MapFailed`
    /// （表示该路径上已有大页映射，不可再创建子映射）。
    fn walk_to_level(
        &mut self,
        va: VirtAddr,
        target_level: usize,
        action: WalkAction,
    ) -> Result<*mut PageTableEntry, MemoryError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                match action {
                    WalkAction::ReadOnly => return Err(MemoryError::PageNotMapped),
                    WalkAction::CreateIntermediate => {
                        let frame = alloc_node()?;
                        let frame_paddr = node_paddr(&frame);
                        table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                        self.frames.push(frame);
                        paddr = frame_paddr;
                    }
                }
            } else if pte.is_leaf(level) {
                // 路径上已有大页映射，不可在其子级创建新映射
                return Err(MemoryError::MapFailed);
            } else {
                paddr = pte.paddr();
            }
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, target_level);
        Ok(table.entry_ptr(idx))
    }

    /// 映射单个虚拟页到物理帧（Level 0，4KB）。若该 VA 已被映射则返回错误。
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), MemoryError> {
        self.map_at_level(va, pa, flags, 0)
    }

    /// 在指定层级映射虚拟地址到物理地址。
    ///
    /// - `level = 0`：4KB 页（标准映射）
    /// - `level = 1`：2MB 大页（megapage / block）
    /// - `level = 2`：1GB 大页（gigapage / block）
    ///
    /// # Errors
    ///
    /// 该 VA 已被映射（同级或路径上有大页）时返回 `MapFailed`。
    pub fn map_at_level(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PteFlags,
        level: usize,
    ) -> Result<(), MemoryError> {
        let pte_ptr = self.walk_to_level(va, level, WalkAction::CreateIntermediate)?;
        // SAFETY: walk_to_level 返回的指针指向 self 持有的帧内存
        let current = unsafe { pte_ptr.read() };
        if current.is_valid() {
            return Err(MemoryError::MapFailed);
        }
        // 适配标志位格式：AArch64 block entry (level > 0) 需清除 TABLE 位
        let leaf_flags = flags.for_leaf_at_level(level);
        // SAFETY: pte_ptr 指向 self 持有的帧内存，上方已检查无冲突映射
        unsafe { pte_ptr.write(PageTableEntry::new(pa, leaf_flags)) };
        Ok(())
    }

    /// 取消映射单个虚拟页，返回其原始物理地址。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte_ptr = self
            .walk_to_level(va, 0, WalkAction::ReadOnly)
            .map_err(|_| MemoryError::PageNotMapped)?;
        // SAFETY: walk_to_level 返回的指针指向 self 持有的帧内存
        let pte = unsafe { pte_ptr.read() };
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        // SAFETY: pte_ptr 指向 self 持有的帧内存，上方已确认该 PTE 有效
        unsafe { pte_ptr.write(PageTableEntry::empty()) };
        Ok(old_pa)
    }

    /// 只读遍历——从根向下查找叶 PTE，返回 PTE 及其所在层级。
    ///
    /// 支持大页：遍历过程中若遇到叶节点即返回。
    /// 不分配中间节点，遇到无效中间 PTE 返回 `None`。
    fn walk_readonly(&self, va: VirtAddr) -> Option<(PageTableEntry, usize)> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            if pte.is_leaf(level) {
                return Some((pte, level));
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read(idx);
        if pte.is_valid() && pte.is_leaf(0) {
            Some((pte, 0))
        } else {
            None
        }
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    ///
    /// 支持大页：遍历过程中若遇到叶节点即返回。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, _level) = self.walk_readonly(va)?;
        Some((pte.paddr(), pte.flags()))
    }
}
