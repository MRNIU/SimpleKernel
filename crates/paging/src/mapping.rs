//! 仿射类型映射——move-only 的 VA->PA 映射所有权。
//!
//! [`MappedPages`] 持有物理帧（[`MappedFrames`]）的所有权，VA 通过 identity mapping
//! 从 PA 推导（`VA = PA + PHYS_OFFSET`）。Drop 时自动 unmap PTE 并回收帧。
//!
//! SAS 架构下只有 identity mapping——VA 与 PA 一一对应，不存在独立的虚拟地址分配。

use core::mem::ManuallyDrop;

use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, MappedFrames, UnmappedFrames};
use memory_types::VirtAddr;

use crate::{PteFlags, PteFlagsOps};

/// unmap 分块大小——每次持锁 unmap 的最大页数。
const UNMAP_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 仿射类型映射——持有物理帧所有权，VA 从 PA 推导。
///
/// 不可 Clone、不可 Copy。Drop 时 unmap PTE 并回收帧。
///
/// 帧以 `MappedFrames` 状态持有——typestate 保证：
/// - 映射期间帧不会被意外释放（`MappedFrames::Drop` 会 panic）
/// - Drop 时先 unmap，再将帧转为 `UnmappedFrames`（安全归还 buddy）
pub struct MappedPages {
    /// `ManuallyDrop` 阻止 `MappedFrames::Drop`（会 panic）在 `MappedPages::Drop` 前运行。
    /// Drop 中手动 take + `into_unmapped()` 安全归还。
    frames: ManuallyDrop<MappedFrames>,
    flags: PteFlags,
}

impl MappedPages {
    /// 消费 frames 的所有权，建立 identity mapping（VA == PA）。
    ///
    /// VA 通过 `PA + PHYS_OFFSET` 推导，调用方无需指定虚拟地址。
    pub fn map(frames: AllocatedFrames, flags: PteFlags) -> Self {
        let page_count = frames.count();
        assert!(page_count > 0, "MappedPages::map: 页数不能为 0");

        let pt = crate::kernel_page_table();
        let pa_start = frames.start_paddr();
        let va_start = pa_start.to_virt();

        let mut guard = pt.lock();
        for i in 0..page_count {
            let va = va_start + i * PAGE_SIZE;
            let pa = pa_start + i * PAGE_SIZE;
            guard
                .map_page(va, pa, flags)
                .expect("MappedPages::map: map_page 失败");
        }
        drop(guard);

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

    /// 返回构造时的请求权限。
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

    /// 修改映射权限——遍历 PTE 更新标志位，刷新 TLB。
    pub fn mprotect(&mut self, new_flags: PteFlags) {
        let pt = crate::kernel_page_table();
        let mut guard = pt.lock();
        for i in 0..self.page_count() {
            let va = self.vaddr() + i * PAGE_SIZE;
            guard
                .update_flags(va, new_flags)
                .expect("mprotect: update_flags 失败");
        }
        drop(guard);
        {
            let _flush = tlb::TlbFlushGuard::new(self.vaddr().as_usize(), self.page_count());
        }
        self.flags = new_flags;
    }

    /// 手动解除映射并取回帧所有权。
    pub fn unmap(self) -> UnmappedFrames {
        let mut md = ManuallyDrop::new(self);
        // SAFETY: md 不会 Drop，手动接管字段所有权
        let mapped_frames = unsafe { ManuallyDrop::take(&mut md.frames) };
        unmap_frames_chunked(&mapped_frames, "MappedPages::unmap");
        mapped_frames.into_unmapped()
    }
}

/// 分块 unmap——每次持锁最多 unmap `UNMAP_CHUNK` 页，避免长时间关中断。
fn unmap_frames_chunked(frames: &MappedFrames, caller: &str) {
    let page_count = frames.count();
    let va_start = frames.start_paddr().to_virt();
    let pt = crate::kernel_page_table();

    let mut offset = 0;
    while offset < page_count {
        let n = (page_count - offset).min(UNMAP_CHUNK);
        {
            let mut guard = pt.lock();
            for i in 0..n {
                let va = va_start + (offset + i) * PAGE_SIZE;
                guard.unmap_page(va).unwrap_or_else(|e| {
                    panic!("{caller}: unmap {va} 失败: {e}");
                });
            }
        }
        {
            let flush_va = va_start + offset * PAGE_SIZE;
            let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
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
        unmap_frames_chunked(&self.frames, "MappedPages::drop");
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
