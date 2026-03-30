//! 虚拟内存区域（VMA）与地址空间管理。
//!
//! - [`Vma`]：描述一段连续虚拟地址空间的属性（权限、backing 类型）
//! - [`AddressSpace`]：管理一个页表及其所有 VMA

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use crate::MappedPages;
use crate::error::MemoryError;
use crate::node_frame::PageTable;
use address::{AddrRange, VirtAddr};
use config::PAGE_SIZE;
use page_table::{PteFlags, PteFlagsOps};
use sync_crate::SpinLock;

/// VMA backing 类型——描述物理内存的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmaKind {
    /// 匿名映射——物理帧由帧分配器按需分配。
    ///
    /// 用于堆、栈、`mmap(MAP_ANONYMOUS)` 等场景。
    Anonymous,
    /// Identity mapping（VA == PA）——不分配新帧。
    ///
    /// 用于内核 RAM 映射、MMIO 等。
    Identity,
}

/// 虚拟内存区域——描述地址空间中一段连续区域的属性。
///
/// VMA 不直接持有物理帧或页表映射——这些由内部的
/// [`MappedPages`] 管理。VMA 的职责是：
/// 1. 记录区域的虚拟地址范围
/// 2. 记录访问权限（通过 `PteFlags`）
/// 3. 记录 backing 类型（匿名 / identity）
/// 4. 在映射建立后持有 `MappedPages` 的所有权
///
/// Drop 时内部 `MappedPages` 自动 unmap 并回收帧。
pub struct Vma {
    /// 虚拟地址范围 `[start, end)`，页对齐
    range: AddrRange<VirtAddr>,
    /// 访问权限
    flags: PteFlags,
    /// backing 类型
    kind: VmaKind,
    /// 已建立的映射——`None` 表示尚未物化（lazy）
    mapping: Option<MappedPages>,
}

impl Vma {
    /// 返回虚拟地址范围。
    #[must_use]
    pub fn range(&self) -> AddrRange<VirtAddr> {
        self.range
    }

    /// 返回起始虚拟地址。
    #[must_use]
    pub fn start(&self) -> VirtAddr {
        self.range.start()
    }

    /// 返回区域大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.range.size()
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.size() / PAGE_SIZE
    }

    /// 返回访问权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 返回 backing 类型。
    #[must_use]
    pub fn kind(&self) -> VmaKind {
        self.kind
    }

    /// 是否已建立物理映射。
    #[must_use]
    pub fn is_mapped(&self) -> bool {
        self.mapping.is_some()
    }
}

impl core::fmt::Debug for Vma {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Vma({}-{}, {:?}, {:?}, {})",
            self.range.start(),
            self.range.end(),
            self.flags,
            self.kind,
            if self.is_mapped() { "mapped" } else { "lazy" },
        )
    }
}

/// 地址空间——管理一个页表及其所有 VMA。
///
/// 每个进程拥有一个 `AddressSpace`；内核自身也有一个。
/// VMA 以起始地址为键存储在 `BTreeMap` 中，保证有序、O(log n) 查找。
///
/// # 所有权模型
///
/// `AddressSpace` 通过 `Arc<SpinLock<PageTable>>` 共享页表引用。
/// 内核地址空间共享全局内核页表；用户进程各自持有独立页表的 `Arc`。
/// 进程退出后 `Arc` 引用计数归零，页表自动释放。
pub struct AddressSpace {
    /// 所属页表的 `Arc` 引用
    page_table: Arc<SpinLock<PageTable>>,
    /// VMA 集合——以起始虚拟地址为键，保证有序且不重叠
    areas: BTreeMap<VirtAddr, Vma>,
}

impl AddressSpace {
    /// 创建空地址空间。
    pub fn new(page_table: Arc<SpinLock<PageTable>>) -> Self {
        Self {
            page_table,
            areas: BTreeMap::new(),
        }
    }

    /// 返回页表的 `Arc` 引用。
    #[must_use]
    pub fn page_table(&self) -> &Arc<SpinLock<PageTable>> {
        &self.page_table
    }

    /// 返回所有 VMA 的迭代器。
    pub fn iter(&self) -> impl Iterator<Item = &Vma> {
        self.areas.values()
    }

    /// VMA 数量。
    #[must_use]
    pub fn area_count(&self) -> usize {
        self.areas.len()
    }

    /// 查找包含 `addr` 的 VMA。
    ///
    /// 用于 page fault handler：检查访问地址是否属于合法区域。
    #[must_use]
    pub fn find_vma(&self, addr: VirtAddr) -> Option<&Vma> {
        // BTreeMap 的 range 查询：找到 start <= addr 的最后一个 VMA，
        // 然后检查 addr 是否在其范围内。
        self.areas
            .range(..=addr)
            .next_back()
            .map(|(_, vma)| vma)
            .filter(|vma| vma.range.contains(addr))
    }

    /// 创建匿名映射——分配帧并建立 VA→PA 映射。
    ///
    /// 等价于 `mmap(addr, size, prot, MAP_ANONYMOUS | MAP_FIXED, -1, 0)`。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    /// - `AllocationFailed` / `OutOfMemory`：帧分配失败
    /// - `MapFailed`：页表映射冲突
    pub fn mmap_anonymous(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, page_count) = Self::validate_range(start, size)?;
        let range = AddrRange::new(start, end);
        self.check_overlap(range)?;

        let mapping = {
            let mut pt = self.page_table.lock();
            MappedPages::map_alloc(&mut pt, self.page_table.clone(), start, page_count, flags)?
        };

        let vma = Vma {
            range,
            flags: mapping.flags(),
            kind: VmaKind::Anonymous,
            mapping: Some(mapping),
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 创建 identity mapping——VA == PA，不分配新帧。
    ///
    /// 用于内核 RAM 映射和 MMIO。映射标记为永久（drop 时不 unmap）。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    /// - `MapFailed`：页表映射冲突
    pub fn mmap_identity(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, page_count) = Self::validate_range(start, size)?;
        let range = AddrRange::new(start, end);
        self.check_overlap(range)?;

        let mapping = {
            let mut pt = self.page_table.lock();
            let pa = address::PhysAddr::new(start.as_usize());
            let mp =
                MappedPages::map_identity(&mut pt, self.page_table.clone(), pa, page_count, flags)?;
            mp.into_permanent()
        };

        let vma = Vma {
            range,
            flags,
            kind: VmaKind::Identity,
            mapping: Some(mapping),
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 创建 lazy VMA——仅记录区域属性，不立即分配帧或建立映射。
    ///
    /// 后续通过 [`handle_page_fault`](AddressSpace::handle_page_fault)
    /// 按需分配。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    pub fn mmap_lazy(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
        kind: VmaKind,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = AddrRange::new(start, end);
        self.check_overlap(range)?;

        let vma = Vma {
            range,
            flags,
            kind,
            mapping: None,
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 取消映射并移除 VMA。
    ///
    /// 查找包含 `addr` 的 VMA，移除它。内部 `MappedPages` drop 时
    /// 自动 unmap 并回收 EXCLUSIVE 帧。
    ///
    /// # Errors
    ///
    /// - `RegionNotFound`：没有包含 `addr` 的 VMA
    pub fn munmap(&mut self, addr: VirtAddr) -> Result<(), MemoryError> {
        let start = self
            .areas
            .range(..=addr)
            .next_back()
            .filter(|(_, vma)| vma.range.contains(addr))
            .map(|(&k, _)| k)
            .ok_or(MemoryError::RegionNotFound)?;
        // 移除 VMA——MappedPages 的 Drop 自动 unmap + 帧回收
        self.areas.remove(&start);
        Ok(())
    }

    /// 处理 page fault——检查地址是否在 lazy VMA 中，若是则按需映射。
    ///
    /// # 返回值
    ///
    /// - `Ok(true)`：成功处理（已建立映射）
    /// - `Ok(false)`：地址不在任何 VMA 中（非法访问，应发送 SIGSEGV）
    /// - `Err(_)`：映射过程中发生错误
    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> Result<bool, MemoryError> {
        // 找到包含 addr 的 VMA
        let start = match self
            .areas
            .range(..=addr)
            .next_back()
            .filter(|(_, vma)| vma.range.contains(addr))
            .map(|(&k, _)| k)
        {
            Some(k) => k,
            None => return Ok(false),
        };

        let vma = self.areas.get_mut(&start).expect("刚查到的 VMA");

        // 已映射的 VMA 发生 fault——可能是权限错误，当前不处理
        if vma.is_mapped() {
            return Ok(false);
        }

        // Lazy VMA：物化映射
        let mapping = match vma.kind {
            VmaKind::Anonymous => {
                let mut pt = self.page_table.lock();
                MappedPages::map_alloc(
                    &mut pt,
                    self.page_table.clone(),
                    vma.range.start(),
                    vma.page_count(),
                    vma.flags,
                )?
            }
            VmaKind::Identity => {
                let mut pt = self.page_table.lock();
                let pa = address::PhysAddr::new(vma.range.start().as_usize());
                MappedPages::map_identity(
                    &mut pt,
                    self.page_table.clone(),
                    pa,
                    vma.page_count(),
                    vma.flags,
                )?
                .into_permanent()
            }
        };

        vma.flags = mapping.flags();
        vma.mapping = Some(mapping);
        Ok(true)
    }

    /// Identity-map 一段物理地址区间（VA == PA），自动选择大页。
    ///
    /// 内部委托给 [`PageTable::identity_map_range`]，自动使用最大可用页大小
    /// （1GB / 2MB / 4KB），减少 TLB 压力。映射标记为永久（drop 时不 unmap）。
    ///
    /// 与 [`mmap_identity`] 的区别：`mmap_identity` 逐页映射（通过 `MappedPages`），
    /// 此方法使用大页映射，适合内核启动时的大段 RAM 映射。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    /// - `MapFailed`：页表映射冲突或无效范围
    pub fn mmap_identity_range(
        &mut self,
        start: VirtAddr,
        end: VirtAddr,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let start_aligned = start.align_down();
        let end_aligned = end.align_up();
        if start_aligned.as_usize() >= end_aligned.as_usize() {
            return Err(MemoryError::MapFailed);
        }
        let range = AddrRange::new(start_aligned, end_aligned);
        self.check_overlap(range)?;

        {
            let mut pt = self.page_table.lock();
            let pa_start = address::PhysAddr::new(start_aligned.as_usize());
            let pa_end = address::PhysAddr::new(end_aligned.as_usize());
            pt.identity_map_range(pa_start, pa_end, flags)?;
        }

        let vma = Vma {
            range,
            flags,
            kind: VmaKind::Identity,
            mapping: None, // 大页映射由页表直接管理，不通过 MappedPages
        };
        self.areas.insert(start_aligned, vma);
        Ok(self.areas.get(&start_aligned).expect("刚插入的 VMA"))
    }

    /// 注册已由外部建立的映射——仅做簿记，不操作页表。
    ///
    /// 用于内核启动时：`PageTable::identity_map_range()` 已直接建立映射，
    /// 此方法将这些区域记录到 `AddressSpace` 中以供 `find_vma` 查询。
    ///
    /// 与 `mmap_identity` 的区别：`mmap_identity` 会调用页表映射操作，
    /// 而 `register_existing` 假设映射已存在，仅创建 VMA 记录。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    pub fn register_existing(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
        kind: VmaKind,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = AddrRange::new(start, end);
        self.check_overlap(range)?;

        let vma = Vma {
            range,
            flags,
            kind,
            mapping: None, // 映射由外部管理，VMA 仅做记录
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 修改 VMA 权限。
    ///
    /// 当前实现仅更新 VMA 的 flags 记录，**不修改已建立的 PTE**。
    ///
    // TODO: 实现页表级权限更新——遍历 VMA 范围内的 PTE 修改标志位，
    // 并执行 TLB flush。
    ///
    /// # Errors
    ///
    /// - `RegionNotFound`：没有包含 `addr` 的 VMA
    pub fn mprotect(&mut self, addr: VirtAddr, flags: PteFlags) -> Result<(), MemoryError> {
        let start = self
            .areas
            .range(..=addr)
            .next_back()
            .filter(|(_, vma)| vma.range.contains(addr))
            .map(|(&k, _)| k)
            .ok_or(MemoryError::RegionNotFound)?;
        let vma = self.areas.get_mut(&start).expect("刚查到的 VMA");
        vma.flags = flags;
        Ok(())
    }

    /// 验证并对齐地址范围，返回 `(start_aligned, end_aligned, page_count)`。
    fn validate_range(
        start: VirtAddr,
        size: usize,
    ) -> Result<(VirtAddr, VirtAddr, usize), MemoryError> {
        if size == 0 {
            return Err(MemoryError::MapFailed);
        }
        let start = start.align_down();
        let end = VirtAddr::new(start.as_usize() + size).align_up();
        let page_count = (end - start) / PAGE_SIZE;
        Ok((start, end, page_count))
    }

    /// 检查新区域是否与已有 VMA 重叠。
    fn check_overlap(&self, range: AddrRange<VirtAddr>) -> Result<(), MemoryError> {
        // 检查前一个和后一个 VMA 是否与新区域重叠
        if let Some((_, prev)) = self.areas.range(..range.start()).next_back() {
            if prev.range.overlaps(range) {
                return Err(MemoryError::RegionOverlap);
            }
        }
        if let Some((_, next)) = self.areas.range(range.start()..).next() {
            if next.range.overlaps(range) {
                return Err(MemoryError::RegionOverlap);
            }
        }
        Ok(())
    }
}

impl core::fmt::Debug for AddressSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AddressSpace({} areas)", self.areas.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapped_pages_crate::test_pt;

    /// 空地址空间应无 VMA。
    #[test]
    fn empty_address_space() {
        let pt_ref = test_pt();
        let aspace = AddressSpace::new(pt_ref);
        assert_eq!(aspace.area_count(), 0);
        assert!(aspace.find_vma(VirtAddr::new(0x1000)).is_none());
    }

    /// mmap_identity 应创建 VMA 并建立映射。
    #[test]
    fn mmap_identity_creates_vma() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x10_0000);
        let vma = aspace
            .mmap_identity(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap_identity 应成功");
        assert_eq!(vma.start(), start);
        assert_eq!(vma.size(), PAGE_SIZE);
        assert_eq!(vma.kind(), VmaKind::Identity);
        assert!(vma.is_mapped());
        assert_eq!(aspace.area_count(), 1);
    }

    /// mmap_anonymous 应分配帧并创建 EXCLUSIVE 映射。
    #[test]
    fn mmap_anonymous_creates_exclusive() {
        crate::frame::ensure_test_init();
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x20_0000);
        let vma = aspace
            .mmap_anonymous(start, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap_anonymous 应成功");
        assert_eq!(vma.page_count(), 2);
        assert_eq!(vma.kind(), VmaKind::Anonymous);
        assert!(vma.is_mapped());
        assert!(vma.flags().is_exclusive());
    }

    /// find_vma 应找到包含地址的 VMA。
    #[test]
    fn find_vma_lookup() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x30_0000);
        aspace
            .mmap_identity(start, 3 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");

        // 区域内地址应能找到
        assert!(aspace.find_vma(start).is_some());
        assert!(aspace.find_vma(start + PAGE_SIZE).is_some());
        assert!(aspace.find_vma(start + 2 * PAGE_SIZE).is_some());
        // 区域外地址应找不到
        assert!(aspace.find_vma(start + 3 * PAGE_SIZE).is_none());
        assert!(aspace.find_vma(VirtAddr::new(0)).is_none());
    }

    /// 重叠区域应被拒绝。
    #[test]
    fn overlap_rejected() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x40_0000);
        aspace
            .mmap_identity(start, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("第一次 mmap");

        // 完全重叠
        let err = aspace
            .mmap_identity(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect_err("完全重叠应失败");
        assert_eq!(err, MemoryError::RegionOverlap);

        // 部分重叠
        let err = aspace
            .mmap_identity(start + PAGE_SIZE, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect_err("部分重叠应失败");
        assert_eq!(err, MemoryError::RegionOverlap);
    }

    /// 相邻但不重叠的区域应允许。
    #[test]
    fn adjacent_regions_allowed() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start1 = VirtAddr::new(0x50_0000);
        let start2 = start1 + 2 * PAGE_SIZE;
        aspace
            .mmap_identity(start1, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("第一段");
        aspace
            .mmap_identity(start2, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("相邻段应成功");
        assert_eq!(aspace.area_count(), 2);
    }

    /// munmap 应移除 VMA。
    #[test]
    fn munmap_removes_vma() {
        crate::frame::ensure_test_init();
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x60_0000);
        aspace
            .mmap_anonymous(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");
        assert_eq!(aspace.area_count(), 1);

        aspace.munmap(start).expect("munmap 应成功");
        assert_eq!(aspace.area_count(), 0);
        assert!(aspace.find_vma(start).is_none());
    }

    /// munmap 不存在的地址应失败。
    #[test]
    fn munmap_nonexistent_fails() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let err = aspace
            .munmap(VirtAddr::new(0x99_0000))
            .expect_err("munmap 不存在的地址应失败");
        assert_eq!(err, MemoryError::RegionNotFound);
    }

    /// mmap_lazy 应创建未映射的 VMA。
    #[test]
    fn mmap_lazy_creates_unmapped() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x70_0000);
        let vma = aspace
            .mmap_lazy(start, PAGE_SIZE, PteFlags::kernel_rw(), VmaKind::Anonymous)
            .expect("mmap_lazy 应成功");
        assert!(!vma.is_mapped());
        assert_eq!(vma.kind(), VmaKind::Anonymous);
    }

    /// handle_page_fault 应物化 lazy VMA。
    #[test]
    fn page_fault_materializes_lazy_vma() {
        crate::frame::ensure_test_init();
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x80_0000);
        aspace
            .mmap_lazy(start, PAGE_SIZE, PteFlags::kernel_rw(), VmaKind::Anonymous)
            .expect("mmap_lazy");
        assert!(!aspace.find_vma(start).expect("应存在").is_mapped());

        let handled = aspace
            .handle_page_fault(start)
            .expect("page fault 处理应成功");
        assert!(handled);
        assert!(aspace.find_vma(start).expect("应存在").is_mapped());
    }

    /// 不在任何 VMA 中的 page fault 应返回 false。
    #[test]
    fn page_fault_outside_vma_returns_false() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let handled = aspace
            .handle_page_fault(VirtAddr::new(0xDEAD_0000))
            .expect("不应出错");
        assert!(!handled);
    }

    /// mprotect 应更新 VMA 权限。
    #[test]
    fn mprotect_updates_flags() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x90_0000);
        aspace
            .mmap_identity(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");

        aspace
            .mprotect(start, PteFlags::kernel_ro())
            .expect("mprotect 应成功");
        let vma = aspace.find_vma(start).expect("应存在");
        assert!(!vma.flags().is_writable());
    }

    /// 多个 VMA 的 find_vma 应正确区分。
    #[test]
    fn multiple_vmas_find_correct() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);

        let a_start = VirtAddr::new(0xA0_0000);
        let b_start = VirtAddr::new(0xB0_0000);
        aspace
            .mmap_identity(a_start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("VMA A");
        aspace
            .mmap_identity(b_start, 2 * PAGE_SIZE, PteFlags::kernel_ro())
            .expect("VMA B");

        let a = aspace.find_vma(a_start).expect("A 应存在");
        assert_eq!(a.size(), PAGE_SIZE);

        let b = aspace.find_vma(b_start + PAGE_SIZE).expect("B 应存在");
        assert_eq!(b.size(), 2 * PAGE_SIZE);

        // A 和 B 之间的空隙应找不到 VMA
        assert!(aspace.find_vma(a_start + PAGE_SIZE).is_none());
    }

    /// size 为 0 的 mmap 应失败。
    #[test]
    fn mmap_zero_size_fails() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let err = aspace
            .mmap_identity(VirtAddr::new(0xC0_0000), 0, PteFlags::kernel_rw())
            .expect_err("size=0 应失败");
        assert_eq!(err, MemoryError::MapFailed);
    }

    /// handle_page_fault 对已映射的 VMA 应返回 false（权限错误，非 lazy 缺页）。
    #[test]
    fn page_fault_on_mapped_vma_returns_false() {
        crate::frame::ensure_test_init();
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0xA0_0000);
        aspace
            .mmap_anonymous(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");
        // VMA 已映射，page fault 应返回 false（可能是权限错误）
        let handled = aspace.handle_page_fault(start).expect("不应出错");
        assert!(!handled);
    }

    /// mmap_identity_range 应使用大页映射并创建 VMA 记录。
    #[test]
    fn mmap_identity_range_creates_vma() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0xD0_0000);
        let end = VirtAddr::new(0xD0_0000 + 3 * PAGE_SIZE);
        let vma = aspace
            .mmap_identity_range(start, end, PteFlags::kernel_rw())
            .expect("mmap_identity_range 应成功");
        assert_eq!(vma.size(), 3 * PAGE_SIZE);
        assert_eq!(vma.kind(), VmaKind::Identity);
        // 页表中应能查到映射
        let pt = aspace.page_table().lock();
        assert!(pt.get_mapping(start).is_some());
        assert!(pt.get_mapping(start + PAGE_SIZE).is_some());
        assert!(pt.get_mapping(start + 2 * PAGE_SIZE).is_some());
    }

    /// mmap_identity_range start >= end 应失败。
    #[test]
    fn mmap_identity_range_empty_fails() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let addr = VirtAddr::new(0xE0_0000);
        let err = aspace
            .mmap_identity_range(addr, addr, PteFlags::kernel_rw())
            .expect_err("start == end 应失败");
        assert_eq!(err, MemoryError::MapFailed);
    }

    /// register_existing 应创建 VMA 记录且可通过 find_vma 查询。
    #[test]
    fn register_existing_creates_record() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0xF0_0000);
        let vma = aspace
            .register_existing(
                start,
                2 * PAGE_SIZE,
                PteFlags::kernel_rw(),
                VmaKind::Identity,
            )
            .expect("register_existing 应成功");
        assert_eq!(vma.size(), 2 * PAGE_SIZE);
        assert!(!vma.is_mapped()); // 仅簿记，mapping 为 None
        assert!(aspace.find_vma(start).is_some());
        assert!(aspace.find_vma(start + PAGE_SIZE).is_some());
    }

    /// register_existing 与已有区域重叠应失败。
    #[test]
    fn register_existing_overlap_fails() {
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0x100_0000);
        aspace
            .register_existing(start, PAGE_SIZE, PteFlags::kernel_rw(), VmaKind::Identity)
            .expect("首次注册");
        let err = aspace
            .register_existing(start, PAGE_SIZE, PteFlags::kernel_rw(), VmaKind::Identity)
            .expect_err("重复注册应失败");
        assert_eq!(err, MemoryError::RegionOverlap);
    }

    /// munmap 后重新 mmap 同一地址应成功。
    #[test]
    fn munmap_then_remap() {
        crate::frame::ensure_test_init();
        let pt_ref = test_pt();
        let mut aspace = AddressSpace::new(pt_ref);
        let start = VirtAddr::new(0xB0_0000);
        aspace
            .mmap_anonymous(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("首次 mmap");
        aspace.munmap(start).expect("munmap");
        aspace
            .mmap_anonymous(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("重新 mmap 应成功");
        assert_eq!(aspace.area_count(), 1);
    }
}
