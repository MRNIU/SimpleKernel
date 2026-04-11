//! 虚拟内存区域（VMA）与地址空间管理。
//!
//! SAS 架构下所有映射均为 identity mapping（VA == PA），
//! VMA 仅做簿记——追踪哪些地址区间已被占用。
//!
//! - [`Vma`]：描述一段连续虚拟地址空间的属性（权限、backing 类型）
//! - [`AddressSpace`]：管理所有 VMA

use alloc::collections::BTreeMap;

use crate::OwnedPages;
use crate::error::MemoryError;
use config::PAGE_SIZE;
use memory_types::{Span, VirtAddr};
use paging::PteFlags;

/// 虚拟内存区域——描述地址空间中一段连续区域的属性。
pub struct Vma {
    /// 虚拟地址范围 `[start, end)`，页对齐
    range: Span<VirtAddr>,
    /// 访问权限
    flags: PteFlags,
    /// 已建立的映射——`None` 表示尚未物化（lazy）或外部建立的映射
    mapping: Option<OwnedPages>,
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
            "Vma({}-{}, {:?}, {})",
            self.range.start(),
            self.range.end(),
            self.flags,
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

    /// 分配帧并建立 identity mapping（VA == PA）。
    ///
    /// # Errors
    ///
    /// - `RegionOverlap`：与已有 VMA 重叠
    /// - `AllocationFailed` / `OutOfMemory`：分配失败
    pub fn mmap(&mut self, size: usize, flags: PteFlags) -> Result<&Vma, MemoryError> {
        if size == 0 {
            return Err(MemoryError::MapFailed);
        }
        let page_count = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        let frames = frame_allocator::AllocatedFrames::alloc(page_count).map_err(|e| {
            log::warn!("mmap 帧分配失败 (page_count={}): {:?}", page_count, e);
            MemoryError::OutOfMemory
        })?;

        // identity mapping: VA = PA
        let start = frames.start_paddr().to_virt();
        let end = start + page_count * PAGE_SIZE;
        let range = Span::new(start, end);
        self.check_overlap(range)?;

        let mapping = OwnedPages::new(frames, flags);

        let vma = Vma {
            range,
            flags: mapping.flags(),
            mapping: Some(mapping),
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
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
        self.check_overlap(range)?;

        let vma = Vma {
            range,
            flags,
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

        let page_count = vma.page_count();
        let frames = frame_allocator::AllocatedFrames::alloc(page_count).map_err(|e| {
            log::warn!(
                "page fault 帧分配失败 (addr={}, page_count={}): {:?}",
                addr,
                page_count,
                e
            );
            MemoryError::OutOfMemory
        })?;
        let mapping = OwnedPages::new(frames, vma.flags);

        vma.flags = mapping.flags();
        vma.mapping = Some(mapping);
        Ok(true)
    }

    /// 注册已由外部建立的映射——仅做簿记，不操作页表。
    pub fn register_existing(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: PteFlags,
    ) -> Result<&Vma, MemoryError> {
        let (start, end, _) = Self::validate_range(start, size)?;
        let range = Span::new(start, end);
        self.check_overlap(range)?;

        let vma = Vma {
            range,
            flags,
            mapping: None,
        };
        self.areas.insert(start, vma);
        Ok(self.areas.get(&start).expect("刚插入的 VMA"))
    }

    /// 注册已由外部建立的映射——直接接管 OwnedPages 所有权。
    ///
    /// 用于 init 阶段注册内核段映射（帧由 frame_allocator::init 预留）。
    pub fn register_kernel_mapping(&mut self, start: VirtAddr, mapping: paging::OwnedPages) {
        let size = mapping.size();
        let flags = mapping.flags();
        let end = start + size;
        let range = Span::new(start, end);
        let vma = Vma {
            range,
            flags,
            mapping: Some(mapping),
        };
        self.areas.insert(start, vma);
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
    ///
    /// - 完全重合（start + size 均相同）→ `RegionIdentical`
    /// - 部分重叠 → `RegionOverlap`
    fn check_overlap(&self, range: Span<VirtAddr>) -> Result<(), MemoryError> {
        // 检查前一个 VMA（起始地址 < range.start 的最近邻）
        if let Some((_, prev)) = self.areas.range(..range.start()).next_back() {
            if prev.range.overlaps(range) {
                return Err(MemoryError::RegionOverlap);
            }
        }
        // 检查起始地址 >= range.start 的 VMA
        if let Some((_, existing)) = self.areas.range(range.start()..).next() {
            if existing.range == range {
                return Err(MemoryError::RegionIdentical);
            }
            if existing.range.overlaps(range) {
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
