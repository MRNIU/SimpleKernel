//! 多级页表——walk / map / unmap 逻辑。
//!
//! 裸机和宿主机测试共享同一份 walk 实现，
//! 通过 [`NodeFrameOps`] trait 统一帧类型（裸机用 buddy allocator，测试用堆分配）。
//!
//! `unmap_page` 在清除叶 PTE 后回溯检查中间节点是否全空，
//! 若是则清除上级 PTE 并回收该帧——参考 Linux `free_pgtables()`。

extern crate alloc;

use alloc::collections::BTreeMap;

use super::{Level0, PageLevel, PageTableEntry, PteFlags, PteFlagsOps, PteOps, Table, vpn_index};
use crate::error::MemoryError;
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

/// 页表节点帧的统一接口——裸机和测试各自实现。
trait NodeFrameOps: Send + Sized {
    /// 分配一个零初始化的页表节点帧。
    fn alloc() -> Result<Self, MemoryError>;
    /// 获取帧的物理地址（裸机 identity mapping）或堆地址（测试）。
    fn paddr(&self) -> PhysAddr;
}

#[cfg(target_os = "none")]
impl NodeFrameOps for crate::frame::AllocatedFrames {
    fn alloc() -> Result<Self, MemoryError> {
        Self::alloc_one()
    }
    fn paddr(&self) -> PhysAddr {
        self.start_paddr()
    }
}

#[cfg(target_os = "none")]
type NodeFrame = crate::frame::AllocatedFrames;

#[cfg(test)]
struct NodeFrame {
    ptr: *mut u8,
    layout: core::alloc::Layout,
}

// SAFETY: NodeFrame 独占其分配的内存，可安全跨线程传递。
#[cfg(test)]
unsafe impl Send for NodeFrame {}

#[cfg(test)]
impl Drop for NodeFrame {
    fn drop(&mut self) {
        // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}

#[cfg(test)]
impl NodeFrameOps for NodeFrame {
    fn alloc() -> Result<Self, MemoryError> {
        let layout = core::alloc::Layout::from_size_align(config::PAGE_SIZE, config::PAGE_SIZE)
            .expect("NodeFrame: invalid layout");
        // SAFETY: layout 非零大小
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "NodeFrame: allocation failed");
        Ok(Self { ptr, layout })
    }
    fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.ptr as usize)
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
    /// 中间页表节点——以物理地址为键，O(log n) 查找/删除。
    frames: BTreeMap<PhysAddr, NodeFrame>,
}

impl PageTable {
    /// 创建新页表，分配根帧。
    pub fn create() -> Result<Self, MemoryError> {
        let root = <NodeFrame as NodeFrameOps>::alloc()?;
        let root_paddr = NodeFrameOps::paddr(&root);
        Ok(Self {
            root_paddr,
            root,
            frames: BTreeMap::new(),
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    #[cfg(target_os = "none")]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    /// 映射用 walker——遍历到 `target_level` 并按需分配中间节点。
    ///
    /// 遍历过程中若遇到叶节点（大页），返回 `MapFailed`
    /// （表示该路径上已有大页映射，不可再创建子映射）。
    fn walk_create(
        &mut self,
        va: VirtAddr,
        target_level: usize,
    ) -> Result<*mut PageTableEntry, MemoryError> {
        let mut paddr = self.root_paddr;

        for level in (target_level + 1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = <NodeFrame as NodeFrameOps>::alloc()?;
                let frame_paddr = NodeFrameOps::paddr(&frame);
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.insert(frame_paddr, frame);
                paddr = frame_paddr;
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
        let pte_ptr = self.walk_create(va, level)?;
        // SAFETY: walk_create 返回的指针指向 self 持有的帧内存
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
    ///
    /// unmap 后自动检查中间页表节点是否全空——若是则清除上级 PTE
    /// 并回收该节点帧，参考 Linux `free_pgtables()` 的行为。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        // 手动 walk 并记录路径（parent_paddr, index, child_paddr），
        // 用于 unmap 后回溯检查空中间节点。
        let mut path: [(PhysAddr, usize, PhysAddr); 4] =
            [(PhysAddr::new(0), 0, PhysAddr::new(0)); 4];
        let mut path_len = 0;
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return Err(MemoryError::PageNotMapped);
            }
            if pte.is_leaf(level) {
                return Err(MemoryError::PageNotMapped);
            }
            let child_paddr = pte.paddr();
            path[path_len] = (paddr, idx, child_paddr);
            path_len += 1;
            paddr = child_paddr;
        }

        // Level 0：清除叶 PTE
        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        let pte = table.read(idx);
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        table.write(idx, PageTableEntry::empty());

        // 回溯：从叶向根检查空中间节点并回收
        let entries_per_table = Level0::ENTRIES;
        let mut child_paddr = paddr;
        for &(parent_paddr, parent_idx, _) in path[..path_len].iter().rev() {
            // SAFETY: child_paddr 指向由 self 持有的有效帧
            let child_table = unsafe { table_at(child_paddr) };
            let all_empty = (0..entries_per_table).all(|j| !child_table.read(j).is_valid());
            if !all_empty {
                break;
            }
            // 清除父级 PTE
            // SAFETY: parent_paddr 指向由 self 持有的有效帧
            let mut parent_table = unsafe { table_at(parent_paddr) };
            parent_table.write(parent_idx, PageTableEntry::empty());
            // 从 self.frames 中移除并回收 child 帧（O(log n) 查找，Drop 归还分配器）
            self.frames.remove(&child_paddr);
            child_paddr = parent_paddr;
        }

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
    /// 支持大页：遇到叶节点时自动加上页内偏移，返回精确物理地址。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PteFlags)> {
        let (pte, level) = self.walk_readonly(va)?;
        let page_size = super::page_size_at_level(level);
        let offset = va.as_usize() & (page_size - 1);
        Some((pte.paddr() + offset, pte.flags()))
    }

    /// 将 `[start, end)` 物理地址区间 identity-map（VA == PA）。
    ///
    /// 自动使用最大可用页大小（1GB / 2MB / 4KB），减少 TLB 压力和页表内存占用。
    /// 地址和剩余大小都对齐到大页边界时才使用大页映射。
    ///
    /// # Errors
    ///
    /// 映射冲突或 `start >= end` 时返回 `MapFailed`。
    pub fn identity_map_range(
        &mut self,
        start: PhysAddr,
        end: PhysAddr,
        flags: PteFlags,
    ) -> Result<(), MemoryError> {
        let mut addr = start.align_down();
        let end_aligned = end.align_up();

        if addr.as_usize() >= end_aligned.as_usize() {
            return Err(MemoryError::MapFailed);
        }

        while addr.as_usize() < end_aligned.as_usize() {
            let remaining = end_aligned.as_usize() - addr.as_usize();
            let va = VirtAddr::new(addr.as_usize());

            // 从最大页尝试到最小页
            let mut mapped = false;
            for level in (1..PT_LEVELS).rev() {
                let page_size = super::page_size_at_level(level);
                if addr.as_usize().is_multiple_of(page_size) && remaining >= page_size {
                    self.map_at_level(va, addr, flags, level)?;
                    addr += page_size;
                    mapped = true;
                    break;
                }
            }
            if !mapped {
                self.map_page(va, addr, flags)?;
                addr += config::PAGE_SIZE;
            }
        }
        Ok(())
    }
}
