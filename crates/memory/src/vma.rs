//! 虚拟内存区域（VMA）与地址空间管理。
//!
//! - [`Vma`]：描述一段连续虚拟地址空间的属性（权限、backing 类型）
//! - [`AddressSpace`]：管理所有 VMA

use core::mem::ManuallyDrop;

use alloc::collections::BTreeMap;

use crate::MappedPages;
use crate::error::MemoryError;
use config::PAGE_SIZE;
use memory_types::{Span, VirtAddr};
use page_allocator::AllocatedPages;
use paging::{PteFlags, PteFlagsOps};

/// VMA 内部映射存储——区分可回收和永久映射。
enum Mapping {
    /// 匿名映射——Drop 时 unmap 并回收帧
    Reclaimable(MappedPages),
    /// 永久映射——Drop 时不操作（ManuallyDrop 阻止 unmap）
    Permanent(ManuallyDrop<MappedPages>),
}

impl Mapping {
    /// 返回用户请求的原始权限。
    fn flags(&self) -> PteFlags {
        match self {
            Self::Reclaimable(mp) => mp.flags(),
            Self::Permanent(mp) => mp.flags(),
        }
    }
}

/// VMA backing 类型——描述物理内存的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmaKind {
    /// 匿名映射——物理帧由帧分配器按需分配。
    Anonymous,
    /// Identity mapping（VA == PA）——不分配新帧。
    Identity,
}

/// 虚拟内存区域——描述地址空间中一段连续区域的属性。
pub struct Vma {
    /// 虚拟地址范围 `[start, end)`，页对齐
    range: Span<VirtAddr>,
    /// 访问权限
    flags: PteFlags,
    /// backing 类型
    kind: VmaKind,
    /// 已建立的映射——`None` 表示尚未物化（lazy）
    mapping: Option<Mapping>,
}

impl Vma {
    /// 返回虚拟地址范围。
    #[must_use]
    pub fn range(&self) -> Span<VirtAddr> {
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

/// 地址空间——管理所有 VMA。
///
/// SAS 架构下全局唯一。VMA 以起始地址为键存储在 `BTreeMap` 中。
pub struct AddressSpace {
    /// VMA 集合——以起始虚拟地址为键，保证有序且不重叠
    areas: BTreeMap<VirtAddr, Vma>,
}

impl AddressSpace {
    /// 创建空地址空间。
    pub fn new() -> Self {
        Self {
            areas: BTreeMap::new(),
        }
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
    #[must_use]
    pub fn find_vma(&self, addr: VirtAddr) -> Option<&Vma> {
        self.areas
            .range(..=addr)
            .next_back()
            .map(|(_, vma)| vma)
            .filter(|vma| vma.range.contains(addr))
    }

    /// 创建匿名映射——分配帧并建立 VA→PA 映射。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    /// - `AllocationFailed` / `OutOfMemory`：分配失败
    pub fn mmap_anonymous(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, page_count) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
        self.check_overlap(range)?;

        let pages = AllocatedPages::alloc_at(start, page_count)
            .map_err(|_| MemoryError::AllocationFailed)?;
        let mapping = MappedPages::map_alloc(pages, flags);

        let vma = Vma {
            range,
            flags: mapping.flags(),
            kind: VmaKind::Anonymous,
            mapping: Some(Mapping::Reclaimable(mapping)),
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 创建 identity mapping——VA == PA，不分配新帧。
    ///
    /// 映射标记为永久（drop 时不 unmap）。
    pub fn mmap_identity(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, page_count) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
        self.check_overlap(range)?;

        let pages = AllocatedPages::alloc_at(start, page_count)
            .map_err(|_| MemoryError::AllocationFailed)?;
        let mapping = ManuallyDrop::new(MappedPages::map_identity(pages, flags));

        let vma = Vma {
            range,
            flags,
            kind: VmaKind::Identity,
            mapping: Some(Mapping::Permanent(mapping)),
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 创建 lazy VMA——仅记录区域属性，不立即分配或映射。
    pub fn mmap_lazy(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
        kind: VmaKind,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
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
    pub fn munmap(&mut self, addr: VirtAddr) -> Result<(), MemoryError> {
        let start = self
            .areas
            .range(..=addr)
            .next_back()
            .filter(|(_, vma)| vma.range.contains(addr))
            .map(|(&k, _)| k)
            .ok_or(MemoryError::RegionNotFound)?;
        self.areas.remove(&start);
        Ok(())
    }

    /// 处理 page fault——检查地址是否在 lazy VMA 中，若是则按需映射。
    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> Result<bool, MemoryError> {
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

        if vma.is_mapped() {
            return Ok(false);
        }

        let mapping = match vma.kind {
            VmaKind::Anonymous => {
                let pages = AllocatedPages::alloc_at(vma.range.start(), vma.page_count())
                    .map_err(|_| MemoryError::AllocationFailed)?;
                let mp = MappedPages::map_alloc(pages, vma.flags);
                Mapping::Reclaimable(mp)
            }
            VmaKind::Identity => {
                let pages = AllocatedPages::alloc_at(vma.range.start(), vma.page_count())
                    .map_err(|_| MemoryError::AllocationFailed)?;
                let mp = MappedPages::map_identity(pages, vma.flags);
                Mapping::Permanent(ManuallyDrop::new(mp))
            }
        };

        vma.flags = mapping.flags();
        vma.mapping = Some(mapping);
        Ok(true)
    }

    /// Identity-map 一段物理地址区间，自动选择大页。
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
        let range = Span::new(start_aligned, end_aligned);
        self.check_overlap(range)?;

        let page_count = (end_aligned - start_aligned) / PAGE_SIZE;
        let pages = AllocatedPages::alloc_at(start_aligned, page_count)
            .map_err(|_| MemoryError::AllocationFailed)?;
        let mapping = ManuallyDrop::new(MappedPages::map_identity(pages, flags));

        let vma = Vma {
            range,
            flags,
            kind: VmaKind::Identity,
            mapping: Some(Mapping::Permanent(mapping)),
        };
        self.areas.insert(start_aligned, vma);
        Ok(self.areas.get(&start_aligned).expect("刚插入的 VMA"))
    }

    /// 注册已由外部建立的映射——仅做簿记，不操作页表。
    pub fn register_existing(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
        kind: VmaKind,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
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

    /// 修改 VMA 权限。
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

    /// 验证并对齐地址范围。
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
    fn check_overlap(&self, range: Span<VirtAddr>) -> Result<(), MemoryError> {
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

    fn init() {
        paging::ensure_test_init();
    }

    /// 空地址空间应无 VMA。
    #[test]
    fn empty_address_space() {
        init();
        let aspace = AddressSpace::new();
        assert_eq!(aspace.area_count(), 0);
        assert!(aspace.find_vma(VirtAddr::new(0x1000)).is_none());
    }

    /// mmap_identity 应创建 VMA 并建立映射。
    #[test]
    fn mmap_identity_creates_vma() {
        init();
        let mut aspace = AddressSpace::new();
        let start = VirtAddr::new(0x1010_0000);
        let vma = aspace
            .mmap_identity(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap_identity 应成功");
        assert_eq!(vma.start(), start);
        assert_eq!(vma.size(), PAGE_SIZE);
        assert_eq!(vma.kind(), VmaKind::Identity);
        assert!(vma.is_mapped());
        assert_eq!(aspace.area_count(), 1);
    }

    /// mmap_anonymous 应分配帧并创建映射。
    #[test]
    fn mmap_anonymous_creates_mapping() {
        init();
        let mut aspace = AddressSpace::new();
        let start = VirtAddr::new(0x1020_0000);
        let vma = aspace
            .mmap_anonymous(start, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap_anonymous 应成功");
        assert_eq!(vma.page_count(), 2);
        assert_eq!(vma.kind(), VmaKind::Anonymous);
        assert!(vma.is_mapped());
    }

    /// find_vma 应找到包含地址的 VMA。
    #[test]
    fn find_vma_lookup() {
        init();
        let mut aspace = AddressSpace::new();
        let start = VirtAddr::new(0x1030_0000);
        aspace
            .mmap_identity(start, 3 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");
        assert!(aspace.find_vma(start).is_some());
        assert!(aspace.find_vma(start + PAGE_SIZE).is_some());
        assert!(aspace.find_vma(start + 3 * PAGE_SIZE).is_none());
    }

    /// 重叠区域应被拒绝。
    #[test]
    fn overlap_rejected() {
        init();
        let mut aspace = AddressSpace::new();
        let start = VirtAddr::new(0x1040_0000);
        aspace
            .mmap_identity(start, 2 * PAGE_SIZE, PteFlags::kernel_rw())
            .expect("首次 mmap");
        let err = aspace
            .mmap_identity(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect_err("重叠应失败");
        assert_eq!(err, MemoryError::RegionOverlap);
    }

    /// munmap 应移除 VMA。
    #[test]
    fn munmap_removes_vma() {
        init();
        let mut aspace = AddressSpace::new();
        let start = VirtAddr::new(0x1060_0000);
        aspace
            .mmap_anonymous(start, PAGE_SIZE, PteFlags::kernel_rw())
            .expect("mmap");
        assert_eq!(aspace.area_count(), 1);
        aspace.munmap(start).expect("munmap 应成功");
        assert_eq!(aspace.area_count(), 0);
    }

    /// size 为 0 的 mmap 应失败。
    #[test]
    fn mmap_zero_size_fails() {
        init();
        let mut aspace = AddressSpace::new();
        let err = aspace
            .mmap_identity(VirtAddr::new(0x10C0_0000), 0, PteFlags::kernel_rw())
            .expect_err("size=0 应失败");
        assert_eq!(err, MemoryError::MapFailed);
    }
}
