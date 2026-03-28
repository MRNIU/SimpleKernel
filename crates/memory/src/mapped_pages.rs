//! 仿射类型映射——借鉴 Theseus OS 的 `MappedPages` 设计。
//!
//! `MappedPages` 是一个 move-only（非 Clone、非 Copy）类型，
//! 持有一段已建立的 VA→PA 映射的所有权。
//!
//! # 核心安全保证
//!
//! 1. **编译期 use-after-unmap 防护**：`as_type::<T>()` 返回的引用
//!    生命周期绑定到 `&self`，编译器阻止在 `MappedPages` drop 后继续使用。
//! 2. **RAII 自动清理**：drop 时自动 unmap 并释放物理帧。
//! 3. **所有权三态**：`Owned`（持有帧）、`Borrowed`（不持有帧）、
//!    `Permanent`（永不释放），防止 identity-map drop 灾难。

#[cfg(not(test))]
use alloc::vec::Vec;

#[cfg(not(test))]
use crate::error::MemoryError;
#[cfg(not(test))]
use crate::frame::{AllocatedFrames, MappedFrames};
#[cfg(not(test))]
use crate::page_table::{PageTable, PteFlags, PteFlagsOps};
#[cfg(not(test))]
use address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use config::PAGE_SIZE;

/// 帧所有权模型——解决 identity-map drop 灾难和 Theseus 的 Owned/Borrowed 区分。
///
/// 参考 Theseus PTE `EXCLUSIVE` 位的设计理念，但在软件层面实现：
/// - `Owned`：帧由 `MappedPages` 持有，drop 时 unmap PTE + 释放帧
/// - `Borrowed`：帧不由 `MappedPages` 持有（如 identity mapping），drop 时仅 unmap PTE
/// - `Permanent`：永久映射，drop 时不做任何事
#[cfg(not(test))]
enum FrameOwnership {
    Owned(Vec<MappedFrames>),
    Borrowed,
    Permanent,
}

/// 永久帧注册表——持有永久映射的物理帧所有权，防止泄漏且保留追踪能力。
#[cfg(not(test))]
static PERMANENT_FRAMES: sync_crate::SpinLock<Vec<MappedFrames>> =
    sync_crate::SpinLock::new(Vec::new(), "perm_frames");

/// 仿射类型映射——持有此值即证明 VA→PA 映射有效。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
/// Drop 时根据 [`FrameOwnership`] 决定清理策略。
#[cfg(not(test))]
pub struct MappedPages {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 映射权限
    flags: PteFlags,
    /// 帧所有权模型
    ownership: FrameOwnership,
}

#[cfg(not(test))]
impl MappedPages {
    /// Identity-map 一段物理地址区间（VA == PA）。
    ///
    /// 不持有帧所有权（`Borrowed`）——drop 时仅 unmap PTE，不释放帧。
    /// 使用场景：内核启动时的 RAM identity mapping、MMIO 映射。
    ///
    /// # Errors
    ///
    /// 映射冲突时返回错误。
    pub fn map_identity(
        pt: &mut PageTable,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, MemoryError> {
        let va_start = VirtAddr::new(pa_start.as_usize());
        for i in 0..page_count {
            let pa = pa_start + i * PAGE_SIZE;
            let va = va_start + i * PAGE_SIZE;
            if let Err(e) = pt.map_page(va, pa, flags) {
                // 回滚：逆序 unmap 已映射的页
                for j in (0..i).rev() {
                    let _ = pt.unmap_page(va_start + j * PAGE_SIZE);
                }
                return Err(e);
            }
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            ownership: FrameOwnership::Borrowed,
        })
    }

    /// 分配新帧并建立映射。
    ///
    /// 持有帧所有权（`Owned`）——drop 时 unmap PTE + 释放帧。
    /// 部分失败时自动回滚已映射的页。
    ///
    /// # Errors
    ///
    /// 帧分配失败或映射冲突时返回错误。
    pub fn map_alloc(
        pt: &mut PageTable,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, MemoryError> {
        let mut frames: Vec<MappedFrames> = Vec::with_capacity(page_count);
        for i in 0..page_count {
            let frame = AllocatedFrames::alloc_one()?;
            let pa = frame.start_paddr();
            let va = va_start + i * PAGE_SIZE;
            match pt.map_page(va, pa, flags) {
                Ok(()) => {
                    frames.push(frame.into_mapped());
                }
                Err(e) => {
                    // 回滚：逆序 unmap 已映射的页
                    for j in (0..i).rev() {
                        let _ = pt.unmap_page(va_start + j * PAGE_SIZE);
                    }
                    // Mapped → Unmapped（drop 自动归还分配器）
                    for mapped_frame in frames.drain(..) {
                        let _unmapped = mapped_frame.into_unmapped();
                    }
                    return Err(e);
                }
            }
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            ownership: FrameOwnership::Owned(frames),
        })
    }

    /// 消耗 self，标记为永久映射（drop 时不 unmap）。
    ///
    /// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        match core::mem::replace(&mut self.ownership, FrameOwnership::Permanent) {
            FrameOwnership::Owned(frames) => {
                if !frames.is_empty() {
                    PERMANENT_FRAMES.lock().extend(frames);
                }
            }
            FrameOwnership::Borrowed | FrameOwnership::Permanent => {}
        }
        self
    }

    /// 返回映射起始虚拟地址。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.page_count * PAGE_SIZE
    }

    /// 返回映射权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用**生命周期绑定到 `&self`**——
    /// 编译器保证 `MappedPages` drop 后无法使用该引用（use-after-unmap 防护）。
    ///
    /// # Safety
    ///
    /// 调用方必须确保：
    /// 1. `offset + size_of::<T>()` 不超过映射大小
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    /// 3. 该地址处的内容可以安全地解释为 `T`
    #[inline]
    pub unsafe fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        let addr = self.vaddr.as_usize() + offset;
        assert!(
            addr % core::mem::align_of::<T>() == 0,
            "MappedPages::as_type: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *const T = addr as *const T;
        // SAFETY: 调用方保证偏移有效，self 的存在保证映射有效
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    ///
    /// # Safety
    ///
    /// 同 `as_type`，另外调用方必须确保映射具有 WRITE 权限。
    #[inline]
    pub unsafe fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type_mut: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        assert!(
            self.flags.is_writable(),
            "MappedPages::as_type_mut: 映射无 WRITE 权限"
        );
        let addr = self.vaddr.as_usize() + offset;
        assert!(
            addr % core::mem::align_of::<T>() == 0,
            "MappedPages::as_type_mut: 地址 {:#x} 未对齐到 {} 字节",
            addr,
            core::mem::align_of::<T>(),
        );
        let ptr: *mut T = addr as *mut T;
        // SAFETY: 调用方保证偏移有效、映射可写，&mut self 保证独占访问
        unsafe { &mut *ptr }
    }

    /// 从内核页表中 unmap 所有页并刷新 TLB。
    fn unmap_ptes(&self) {
        if let Some(kpt) = crate::kernel_page_table() {
            let mut guard = kpt.lock();
            for i in 0..self.page_count {
                let va = self.vaddr + i * PAGE_SIZE;
                let _ = guard.unmap_page(va);
            }
            // 单页用精确刷新，多页用全局刷新（避免逐页 barrier 的累积开销）
            if self.page_count == 1 {
                crate::tlb::flush_tlb_page(self.vaddr.as_usize());
            } else {
                crate::tlb::flush_tlb();
            }
        }
    }
}

#[cfg(not(test))]
impl Drop for MappedPages {
    fn drop(&mut self) {
        if matches!(self.ownership, FrameOwnership::Permanent) {
            return;
        }

        // 先 unmap PTE（顺序关键：帧释放后地址可能被复用）
        self.unmap_ptes();

        // Owned 帧走 Mapped → Unmapped（drop 自动归还分配器）
        if let FrameOwnership::Owned(frames) = &mut self.ownership {
            for frame in frames.drain(..) {
                let _unmapped = frame.into_unmapped();
            }
        }
    }
}

#[cfg(not(test))]
impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = match &self.ownership {
            FrameOwnership::Owned(_) => "owned",
            FrameOwnership::Borrowed => "borrowed",
            FrameOwnership::Permanent => "permanent",
        };
        write!(
            f,
            "MappedPages({}, {} pages, {:?}, {})",
            self.vaddr, self.page_count, self.flags, kind
        )
    }
}
