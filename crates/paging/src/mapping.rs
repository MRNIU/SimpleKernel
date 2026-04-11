//! 仿射类型所有权——move-only 的物理帧所有权 + 权限管理。
//!
//! [`OwnedPages`] 持有物理帧的独占所有权，VA 通过 identity mapping
//! 从 PA 推导（`VA = PA + PHYS_OFFSET`）。
//!
//! SAS 架构下所有物理内存始终有背景 identity mapping（kernel_rw），
//! `OwnedPages` 管理的是**所有权和权限覆盖层**——
//! `new` 接管所有权并按需调整 PTE flags，`drop` 恢复默认权限并归还帧。

use config::PAGE_SIZE;
use frame_allocator::AllocatedFrames;
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// 仿射类型帧所有权——持有物理帧的独占所有权和当前权限。
///
/// 不可 Clone、不可 Copy。Drop 时恢复 PTE 为默认权限（kernel_rw）并回收帧。
///
/// SAS 全量映射下，所有物理内存始终有 identity mapping（背景层）。
/// `OwnedPages` 不创建/删除 PTE，而是管理权限覆盖：
/// - `new`：接管帧所有权，按需更新 PTE flags
/// - `set_flags`：修改权限
/// - `drop`：恢复 kernel_rw + 归还帧（`AllocatedFrames` 自动 Drop 归还分配器）
pub struct OwnedPages {
    frames: AllocatedFrames,
    flags: PteFlags,
}

impl OwnedPages {
    /// 消费 frames 的所有权，设置指定权限。
    ///
    /// SAS 全量映射下，PTE 已由 boot 背景映射建立（kernel_rw）。
    /// 此方法接管帧所有权并按需更新 PTE flags
    ///（如从默认 kernel_rw 改为 kernel_ro）。
    pub fn new(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "OwnedPages::new: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let pa_start = frames.start_paddr();
        let va_start = pa_start.to_virt();

        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * PAGE_SIZE;
            let pa = pa_start + i * PAGE_SIZE;
            guard
                .set_page_flags(va, pa, flags)
                .expect("OwnedPages::new: set_page_flags 失败");
        }
        drop(guard);

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

    /// 返回构造时的请求权限。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr() + offset).align_down();
        let guard = crate::kernel_page_table().lock();
        guard
            .get_mapping(page_va)
            .expect("OwnedPages::pte_flags: 页表项不存在")
            .1
    }

    /// 获取区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用生命周期绑定到 `&self`——编译器保证 Drop 后无法使用。
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "OwnedPages::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证帧存活
        unsafe { &*ptr }
    }

    /// 获取区域内指定偏移处的可变类型化引用。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "OwnedPages::as_type_mut",
        );
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "OwnedPages::as_type_mut: PTE 无 WRITE 权限"
        );
        // SAFETY: 偏移和对齐已验证，PTE 可写已验证，&mut self 保证独占
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 修改权限——遍历 PTE 更新标志位，刷新 TLB。
    pub fn set_flags(&mut self, new_flags: PteFlags) {
        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            guard
                .update_flags(va, new_flags)
                .expect("set_flags: update_flags 失败");
        }
        drop(guard);
        let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        self.flags = new_flags;
    }
}

/// 验证偏移在范围内且地址对齐到 `T` 的自然边界，返回目标指针。
pub(crate) fn check_bounds_and_align<T>(
    base: usize,
    size: usize,
    offset: usize,
    fn_name: &str,
) -> *const T {
    let type_size = core::mem::size_of::<T>();
    assert!(
        type_size > 0,
        "{fn_name}: 不支持 ZST（size_of::<T>() == 0）"
    );
    assert!(
        type_size <= size && offset <= size - type_size,
        "{fn_name}: offset {:#x} + {type_size} 超出大小 {:#x}",
        offset,
        size,
    );
    let addr = base + offset;
    let align = core::mem::align_of::<T>();
    assert!(
        addr.is_multiple_of(align),
        "{fn_name}: 地址 {:#x} 未对齐到 {align} 字节",
        addr,
    );
    addr as *const T
}

/// 批量恢复 PTE 为 kernel_rw（背景默认权限）并刷新 TLB。
///
/// 持锁期间一次性更新所有页，避免逐页 lock/unlock 开销。
fn restore_default_flags(va_start: VirtAddr, page_count: usize) {
    let default_flags = PteFlags::kernel_rw();
    let pt = crate::kernel_page_table();
    let mut guard = pt.lock();
    for i in 0..page_count {
        let va = va_start + i * PAGE_SIZE;
        guard
            .update_flags(va, default_flags)
            .expect("restore_default_flags: update_flags 失败");
    }
    drop(guard);
    let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
}

impl Drop for OwnedPages {
    fn drop(&mut self) {
        restore_default_flags(self.vaddr(), self.page_count());
        // AllocatedFrames 的 Drop 自动归还帧到分配器——零 unsafe
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
