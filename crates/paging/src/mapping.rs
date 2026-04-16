//! 仿射类型所有权——move-only 的物理帧所有权 + 权限管理 + CLAIMED 位保护。
//!
//! [`OwnedPages`] 持有物理帧的独占所有权，VA 通过 identity mapping
//! 从 PA 推导（`VA = PA + PHYS_OFFSET`）。
//!
//! SAS 架构下所有物理内存始终有背景 identity mapping（kernel_rw），
//! `OwnedPages` 管理的是**所有权和权限覆盖层**。
//!
//! PTE 中的 CLAIMED 软件位（RISC-V RSW[0] / AArch64 bit 55）提供运行时纵深防御——
//! 与编译期 typestate 互补，检测 `unsafe` 代码或分配器 bug 导致的双重分配。

use config::PAGE_SIZE;
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// 仿射类型帧所有权——持有物理帧的独占所有权和当前权限。
///
/// 不可 Clone、不可 Copy。Drop 时恢复 PTE 为默认权限（kernel_rw）、
/// 写入 poison 填充、并回收帧。
///
/// SAS 全量映射下，所有物理内存始终有 identity mapping（背景层）。
/// `OwnedPages` 不创建/删除 PTE，而是管理权限覆盖：
/// - `new`：设置 CLAIMED + 更新权限
/// - `set_flags`：修改权限（保留 CLAIMED）
/// - `drop`：恢复 kernel_rw（清 CLAIMED）→ poison → 归还帧
pub struct OwnedPages {
    frames: AllocatedFrames,
    /// 用户可见的权限标志——不含 CLAIMED 位。
    flags: PteFlags,
}

impl OwnedPages {
    /// 声明物理帧所有权——设置 CLAIMED 位 + 更新权限。
    ///
    /// SAS 全量映射下，PTE 已由 boot 背景映射建立（kernel_rw）。
    /// 此方法接管帧所有权、设置 CLAIMED 软件位、并更新 PTE flags。
    ///
    /// # Panics
    ///
    /// - 页数为 0
    /// - 任意页的 CLAIMED 位已设置（双重所有权 = 内核 bug）
    /// - PTE 不存在（背景映射未建立）
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "OwnedPages::new: 页数不能为 0");

        let va_start = frames.start_paddr().to_virt();
        claim_pages(va_start, page_count, flags);

        Self { frames, flags }
    }

    /// 返回起始虚拟地址（从 PA 推导）。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.frames.start_paddr().to_virt()
    }

    /// 返回区域总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.frames.count() * PAGE_SIZE
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frames.count()
    }

    /// 返回用户可见的权限（不含 CLAIMED 位）。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 修改权限——遍历 PTE 更新标志位（保留 CLAIMED），刷新 TLB。
    pub fn set_flags(&mut self, new_flags: PteFlags) {
        batch_update_flags(
            self.vaddr(),
            self.page_count(),
            new_flags.with_claimed(true),
        );
        self.flags = new_flags;
    }
}

/// 声明页面所有权——设置 flags + CLAIMED，检查双重所有权。
///
/// 单次锁内完成 check + write，无 TOCTOU 窗口。
fn claim_pages(va_start: VirtAddr, page_count: usize, flags: PteFlags) {
    let claimed_flags = flags.with_claimed(true);
    let pt = crate::kernel_page_table();
    let mut guard = pt.lock();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        let old = guard
            .update_flags(va, claimed_flags)
            .expect("claim_pages: PTE 不存在——背景映射未建立？");
        assert!(
            !old.is_claimed(),
            "OwnedPages::new: 页 {va} 已被声明所有权（CLAIMED 位已设置）——双重分配"
        );
    }
    drop(guard);
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}

/// 批量更新 PTE 权限并刷新 TLB。
///
/// `set_flags` / `Drop` 共用此逻辑。flags 透传（含或不含 CLAIMED 由调用方决定）。
fn batch_update_flags(va_start: VirtAddr, page_count: usize, flags: PteFlags) {
    let pt = crate::kernel_page_table();
    let mut guard = pt.lock();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        guard
            .update_flags(va, flags)
            .expect("batch_update_flags: update_flags 失败");
    }
    drop(guard);
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}

impl Drop for OwnedPages {
    fn drop(&mut self) {
        let va_start = self.vaddr();
        let page_count = self.page_count();

        // 恢复 kernel_rw（清 CLAIMED）+ flush TLB
        batch_update_flags(va_start, page_count, PteFlags::kernel_rw());

        // 写 poison——此时 PTE 已恢复 kernel_rw 且 TLB 已刷新
        // SAFETY: va_start identity-mapped，帧仍由 self.frames 持有（buddy 尚未回收）
        unsafe {
            core::ptr::write_bytes(
                va_start.as_mut_ptr::<u8>(),
                config::FREED_PAGE_POISON,
                page_count * PAGE_SIZE,
            );
        }

        // AllocatedFrames 自然 drop → buddy 回收
    }
}

impl core::fmt::Debug for OwnedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "OwnedPages({}, {} pages, {:?})",
            self.vaddr(),
            self.page_count(),
            self.flags,
        )
    }
}
