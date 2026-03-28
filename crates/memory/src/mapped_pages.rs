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
//! 3. **永久映射支持**：`into_permanent()` 消耗 self 而不 unmap，
//!    用于内核 identity mapping 等永远不会释放的映射。
//!
//! # 与传统方式的对比
//!
//! ```text
//! // 传统：map_page 返回 KResult<()>，VA 可在 unmap 后继续使用
//! pt.map_page(va, pa, flags)?;
//! let val: &u32 = unsafe { &*(va.as_usize() as *const u32) };
//! pt.unmap_page(va)?;
//! // val 仍可使用！use-after-unmap 无法被编译器检测 ❌
//!
//! // MappedPages：编译器强制生命周期
//! let pages = MappedPages::map_identity(&mut pt, pa, 1, flags)?;
//! let val: &u32 = pages.as_type(0);  // 生命周期绑定到 &pages
//! drop(pages); // 自动 unmap
//! // val 不可再使用——编译错误 ✅
//! ```

#[cfg(not(test))]
use alloc::vec::Vec;

#[cfg(not(test))]
use crate::address::{PhysAddr, VirtAddr};
#[cfg(not(test))]
use crate::error::MemoryError;
#[cfg(not(test))]
use crate::frame::FrameTracker;
#[cfg(not(test))]
use crate::page_table::{PageFlags, PageTable};
#[cfg(not(test))]
use config::PAGE_SIZE;

/// 永久帧注册表——持有永久映射的物理帧所有权，防止泄漏且保留追踪能力。
///
/// 参考 Theseus OS：永久映射仍由全局 registry 持有，
/// 只是生命周期与内核等长，而非通过 `mem::forget` 丢弃。
#[cfg(not(test))]
static PERMANENT_FRAMES: spin::Mutex<Vec<FrameTracker>> = spin::Mutex::new(Vec::new());

/// 仿射类型映射——持有此值即证明 VA→PA 映射有效。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
/// Drop 时自动从页表中移除映射并释放物理帧。
#[cfg(not(test))]
pub struct MappedPages {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 映射权限
    flags: PageFlags,
    /// 持有物理帧所有权——drop 时帧归还分配器
    frames: Vec<FrameTracker>,
    /// 是否为永久映射（drop 时不 unmap）
    permanent: bool,
}

#[cfg(not(test))]
impl MappedPages {
    /// Identity-map 一段物理地址区间（VA == PA）。
    ///
    /// 分配 `page_count` 个帧，将 `[pa, pa + page_count * PAGE_SIZE)` 映射。
    /// 使用场景：内核启动时的 RAM identity mapping、MMIO 映射。
    ///
    /// # Errors
    ///
    /// 帧分配失败或映射冲突时返回错误。
    pub fn map_identity(
        pt: &mut PageTable,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PageFlags,
    ) -> Result<Self, MemoryError> {
        let va_start = VirtAddr::new(pa_start.as_usize());
        for i in 0..page_count {
            let pa = pa_start + i * PAGE_SIZE;
            let va = va_start + i * PAGE_SIZE;
            pt.map_page(va, pa, flags)?;
        }
        // identity mapping 不持有 FrameTracker——帧由帧分配器管理，
        // 不是"映射拥有帧"而是"映射引用已有的物理内存"。
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            frames: Vec::new(),
            permanent: false,
        })
    }

    /// 分配新帧并建立映射。
    ///
    /// 与 `map_identity` 不同：此方法分配物理帧，帧所有权由 `MappedPages` 持有。
    /// Drop 时帧归还分配器。
    ///
    /// # Errors
    ///
    /// 帧分配失败或映射冲突时返回错误。
    pub fn map_alloc(
        pt: &mut PageTable,
        va_start: VirtAddr,
        page_count: usize,
        flags: PageFlags,
    ) -> Result<Self, MemoryError> {
        let mut frames = Vec::with_capacity(page_count);
        for i in 0..page_count {
            let frame = FrameTracker::alloc()?;
            let pa = frame.paddr();
            let va = va_start + i * PAGE_SIZE;
            pt.map_page(va, pa, flags)?;
            frames.push(frame);
        }
        Ok(Self {
            vaddr: va_start,
            page_count,
            flags,
            frames,
            permanent: false,
        })
    }

    /// 消耗 self，标记为永久映射（drop 时不 unmap）。
    ///
    /// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
    /// 调用后 `MappedPages` 仍可正常使用，直到被 drop（drop 时不做任何事）。
    #[must_use]
    pub fn into_permanent(mut self) -> Self {
        self.permanent = true;
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
    pub fn flags(&self) -> PageFlags {
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
    pub unsafe fn as_type<T: Copy>(&self, offset: usize) -> &T {
        debug_assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        let addr: *const T = (self.vaddr + offset).as_ptr();
        // SAFETY: 调用方保证偏移有效，self 的存在保证映射有效
        unsafe { &*addr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    ///
    /// # Safety
    ///
    /// 同 `as_type`，另外调用方必须确保映射具有 WRITE 权限。
    #[inline]
    pub unsafe fn as_type_mut<T: Copy>(&mut self, offset: usize) -> &mut T {
        debug_assert!(
            offset + core::mem::size_of::<T>() <= self.size(),
            "MappedPages::as_type_mut: offset {:#x} + {} 超出映射大小 {:#x}",
            offset,
            core::mem::size_of::<T>(),
            self.size(),
        );
        debug_assert!(
            self.flags.contains(PageFlags::WRITE),
            "MappedPages::as_type_mut: 映射无 WRITE 权限"
        );
        let addr: *mut T = (self.vaddr + offset).as_mut_ptr();
        // SAFETY: 调用方保证偏移有效、映射可写，&mut self 保证独占访问
        unsafe { &mut *addr }
    }
}

#[cfg(not(test))]
impl Drop for MappedPages {
    fn drop(&mut self) {
        if self.permanent {
            // 永久映射——不 unmap，帧转移到全局注册表（不释放但保留追踪）
            let frames = core::mem::take(&mut self.frames);
            if !frames.is_empty() {
                PERMANENT_FRAMES.lock().extend(frames);
            }
            return;
        }

        // 非永久映射——从内核页表中移除映射
        if let Some(kpt) = crate::kernel_page_table() {
            let mut guard = kpt.lock();
            for i in 0..self.page_count {
                let va = self.vaddr + i * PAGE_SIZE;
                let _ = guard.unmap_page(va);
            }
            arch_traits::flush_tlb();
        }
        // frames 在此处 drop，物理帧归还分配器
    }
}

#[cfg(not(test))]
impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MappedPages({}, {} pages, {:?}{})",
            self.vaddr,
            self.page_count,
            self.flags,
            if self.permanent { ", permanent" } else { "" }
        )
    }
}
