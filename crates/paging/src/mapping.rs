//! 仿射类型映射——move-only 的 VA 所有权追踪。
//!
//! SAS 架构下所有物理内存在 init 阶段被永久 identity-map（VA == PA），
//! PTE 创建后不再删除。[`MappedPages`] 不操作 PTE 的创建/删除，
//! 而是通过 PTE 中的软件 CLAIMED 位追踪所有权：
//!
//! - [`MappedPages::claim`]：设置 CLAIMED 位 + 更新权限（不创建 PTE）
//! - [`MappedPages::release`]：写 poison + 恢复 `kernel_rw` + 清 CLAIMED（不删 PTE）
//! - Drop：等同 release，帧通过 `into_unmapped()` 自动归还 buddy

use core::mem::ManuallyDrop;

use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, MappedFrames, UnmappedFrames};
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// release 分块大小——每次持锁处理的最大页数。
const RELEASE_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 仿射类型映射——通过 CLAIMED 位追踪物理帧所有权。
///
/// 不可 Clone、不可 Copy。Drop 时写 poison、清 CLAIMED 并回收帧。
///
/// 帧以 `MappedFrames` 状态持有——typestate 保证：
/// - 持有期间帧不会被意外释放（`MappedFrames::Drop` 会 panic）
/// - Drop 时先 release，再将帧转为 `UnmappedFrames`（安全归还 buddy）
pub struct MappedPages {
    /// `ManuallyDrop` 阻止 `MappedFrames::Drop`（会 panic）在 `MappedPages::Drop` 前运行。
    /// Drop 中手动 take + `into_unmapped()` 安全归还。
    frames: ManuallyDrop<MappedFrames>,
    flags: PteFlags,
}

impl MappedPages {
    /// 声明物理帧所有权——设置 CLAIMED 位 + 更新权限。
    ///
    /// 不创建 PTE（init 阶段已 identity-map 全部物理内存）。
    /// 如果目标页的 CLAIMED 位已设置，说明另一个 `MappedPages` 已拥有该页，
    /// 立即 panic（双重所有权 = bug）。
    ///
    /// # Panics
    ///
    /// - 页数为 0
    /// - 任意页的 CLAIMED 位已设置（双重声明）
    /// - `update_flags` 失败（PTE 不存在）
    pub fn claim(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "MappedPages::claim: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let pa_start = frames.start_paddr();
        let va_start = pa_start.to_virt();

        // 带 CLAIMED 位的目标权限
        let claimed_flags = flags.with_claimed(true);

        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * PAGE_SIZE;
            let old_flags = guard.update_flags(va, claimed_flags).unwrap_or_else(|e| {
                panic!(
                    "MappedPages::claim: update_flags({va}) 失败: {e}——\
                         该页未被 identity-map？"
                );
            });

            assert!(
                !old_flags.is_claimed(),
                "MappedPages::claim: 页 {va} 的 CLAIMED 位已设置——双重所有权"
            );
        }
        drop(guard);

        // 刷新 TLB——权限已变更
        {
            let _flush = tlb::TlbFlushGuard::new(va_start.as_usize(), page_count);
        }

        let mapped_frames = frames.into_mapped();

        Self {
            frames: ManuallyDrop::new(mapped_frames),
            flags,
        }
    }

    /// init 阶段专用构造——跳过 CLAIMED 检查，直接包装已设置 CLAIMED 的帧。
    ///
    /// # Safety
    ///
    /// 调用方必须保证：
    /// - `frames` 对应的 PTE 已存在且 CLAIMED 位已设置（由 `identity_map_range` 完成）
    /// - 没有其他 `MappedPages` 持有这些帧
    pub unsafe fn from_claimed(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let mapped_frames = frames.into_mapped();
        Self {
            frames: ManuallyDrop::new(mapped_frames),
            flags,
        }
    }

    /// 返回起始虚拟地址（从 PA 推导）。
    #[must_use]
    pub fn vaddr(&self) -> VirtAddr {
        self.frames.start_paddr().to_virt()
    }

    /// 返回映射总大小（字节）。
    #[must_use]
    pub fn size(&self) -> usize {
        self.frames.count() * PAGE_SIZE
    }

    /// 返回页数。
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.frames.count()
    }

    /// 返回 claim 时的请求权限（不含 CLAIMED 位）。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 返回持有的物理帧引用（Mapped 状态）。
    #[must_use]
    pub fn frames(&self) -> &MappedFrames {
        &self.frames
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr() + offset).align_down();
        let guard = crate::kernel_page_table().lock();
        guard
            .get_mapping(page_va)
            .expect("MappedPages::pte_flags: 映射不存在")
            .1
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用生命周期绑定到 `&self`——编译器保证映射 drop 后无法使用。
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证映射存活
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr().as_usize(),
            self.size(),
            offset,
            "MappedPages::as_type_mut",
        );
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "MappedPages::as_type_mut: PTE 无 WRITE 权限"
        );
        // SAFETY: 偏移和对齐已验证，PTE 可写已验证，&mut self 保证独占
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 修改映射权限——遍历 PTE 更新标志位（保留 CLAIMED），刷新 TLB。
    pub fn mprotect(&mut self, new_flags: PteFlags) {
        // 保留 CLAIMED 位——mprotect 不改变所有权
        let claimed_flags = new_flags.with_claimed(true);

        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            guard
                .update_flags(va, claimed_flags)
                .expect("mprotect: update_flags 失败");
        }
        drop(guard);
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        }
        self.flags = new_flags;
    }

    /// 手动释放所有权并取回帧——写 poison、恢复 kernel_rw、清 CLAIMED。
    pub fn release(self) -> UnmappedFrames {
        let mut md = ManuallyDrop::new(self);
        // SAFETY: md 不会 Drop，手动接管字段所有权
        let mapped_frames = unsafe { ManuallyDrop::take(&mut md.frames) };
        release_frames_chunked(&mapped_frames, "MappedPages::release");
        mapped_frames.into_unmapped()
    }
}

/// 分块释放——写 poison、恢复 kernel_rw、清 CLAIMED。
///
/// 每次持锁最多处理 `RELEASE_CHUNK` 页，避免长时间关中断。
fn release_frames_chunked(frames: &MappedFrames, caller: &str) {
    let page_count = frames.count();
    let va_start = frames.start_paddr().to_virt();
    let pt = crate::kernel_page_table();

    // 恢复默认权限——kernel_rw 且 CLAIMED = false
    let restore_flags = PteFlags::kernel_rw();

    let mut offset = 0;
    while offset < page_count {
        let n = (page_count - offset).min(RELEASE_CHUNK);

        // 步骤 1：恢复 kernel_rw 权限（清 CLAIMED）——必须在写 poison 之前，
        // 因为页面可能是只读的（如 mprotect 设为 RO），直接写会 page fault。
        let mut need_tlb_flush = false;
        {
            let mut guard = pt.lock();
            for i in 0..n {
                let va = va_start + (offset + i) * PAGE_SIZE;
                let old_flags = guard.update_flags(va, restore_flags).unwrap_or_else(|e| {
                    panic!("{caller}: update_flags({va}) 失败: {e}");
                });

                // 断言：旧 PTE 必须有 CLAIMED 位——否则说明 PTE 被外部篡改
                assert!(
                    old_flags.is_claimed(),
                    "{caller}: 页 {va} 的 CLAIMED 位未设置——PTE 被外部篡改？"
                );

                // 如果旧权限（除 CLAIMED 外）与 kernel_rw 不同，需要 TLB flush
                if old_flags.with_claimed(false) != restore_flags {
                    need_tlb_flush = true;
                }
            }
        }

        // 步骤 2：TLB flush（必须在写 poison 之前——旧的只读 TLB 条目可能仍在缓存中）
        if need_tlb_flush {
            let flush_va = va_start + offset * PAGE_SIZE;
            let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
        }

        // 步骤 3：写 poison pattern（此时页面已恢复为 kernel_rw，可安全写入）
        for i in 0..n {
            let va = va_start + (offset + i) * PAGE_SIZE;
            // SAFETY: PTE 已恢复为 kernel_rw（可写），TLB 已刷新，
            // 写入 poison pattern 用于检测 use-after-free
            unsafe {
                core::ptr::write_bytes(va.as_mut_ptr::<u8>(), config::FREED_PAGE_POISON, PAGE_SIZE);
            }
        }

        offset += n;
    }
}

/// 验证偏移在映射范围内且地址对齐到 `T` 的自然边界，返回目标指针。
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
        "{fn_name}: offset {:#x} + {type_size} 超出映射大小 {:#x}",
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

impl Drop for MappedPages {
    fn drop(&mut self) {
        release_frames_chunked(&self.frames, "MappedPages::drop");
        // SAFETY: Drop 执行中，self.frames 不会再被访问。
        // 将 MappedFrames 转换为 UnmappedFrames，后者的 Drop 安全归还 buddy。
        let mapped_frames = unsafe { ManuallyDrop::take(&mut self.frames) };
        let _unmapped = mapped_frames.into_unmapped();
    }
}

impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MappedPages({}, {} pages, {:?})",
            self.vaddr(),
            self.page_count(),
            self.flags,
        )
    }
}
