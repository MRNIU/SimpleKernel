//! 仿射类型映射——move-only 的 VA->PA 映射所有权。
//!
//! ## EXCLUSIVE 帧生命周期
//!
//! `map_alloc` 分配的帧通过 PTE 的 EXCLUSIVE 位追踪所有权，
//! Rust 类型系统无法感知这段"隐形"生命周期。完整流转如下：
//!
//! ```text
//! AllocatedFrames::alloc_one()          ← 帧分配器分配
//!   → frame.into_mapped() → MappedFrames
//!   → core::mem::forget(mapped)         ← 所有权编码到 PTE EXCLUSIVE 位
//!   → ... (映射使用期间) ...
//!   → unmap_page_with_flags()           ← 从 PTE 读回 PA + EXCLUSIVE 标志
//!   → UnmappedFrames::from_range(pa)    ← 重建帧所有权（unsafe）
//!   → Drop                              ← 帧归还分配器
//! ```
//!
//! `map_identity` 不设置 EXCLUSIVE 位——drop 时仅清除 PTE，不回收帧。
//!
//! ## 类型区分
//!
//! - [`MappedPages`]：可回收映射——Drop 时 unmap 并回收 EXCLUSIVE 帧。
//! - [`PermanentMapping`]：永久映射——Drop 时不执行任何操作。
//!
//! 两者通过 `Deref` 共享 [`MappedPagesInner`] 的只读方法。
//!
// TODO: 实现 `split` / `merge` 操作——`munmap` 部分区域和 `mremap` 需要。
//
// TODO: 支持 COW（Copy-on-Write）共享映射——`fork()` 需要多个进程共享同一物理帧
// + 引用计数。需引入 `Frames` 的共享状态或 per-frame 引用计数器。

use alloc::sync::Arc;
use core::ops::{Deref, DerefMut};

use address::{FrameRange, PhysAddr, PhysPageNum, VirtAddr};
use config::PAGE_SIZE;
use frame_allocator::{AllocatedFrames, UnmappedFrames};
use sync_crate::SpinLock;

use crate::error::PagingError;
use crate::{PageTable, PteFlags, PteFlagsOps};

/// map_alloc 每次分配并映射的最大页数。
const MAP_CHUNK: usize = config::MAP_CHUNK_SIZE;

/// unmap_and_reclaim 每次处理的最大页数。
const UNMAP_CHUNK: usize = config::UNMAP_CHUNK_SIZE;

/// 映射的内部共享数据——`MappedPages` 和 `PermanentMapping` 通过 `Deref` 共享。
///
/// 不可直接构造——只能通过 `MappedPages` 或 `PermanentMapping` 的工厂方法创建。
pub struct MappedPagesInner {
    /// 映射起始虚拟地址
    vaddr: VirtAddr,
    /// 映射的页数
    page_count: usize,
    /// 用户请求的原始权限（不含 EXCLUSIVE 等内部标志位）
    flags: PteFlags,
    /// EXCLUSIVE 帧所有权——drop 时是否回收物理帧
    exclusive: bool,
    /// 所属页表的 `Arc` 引用——Drop 时通过此引用 unmap。
    page_table: Arc<SpinLock<PageTable>>,
}

impl MappedPagesInner {
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

    /// 返回构造时的请求权限。
    ///
    /// **注意**：返回的是映射创建时的原始 flags（不含 EXCLUSIVE），
    /// 可能与当前 PTE 实际标志不同（例如 COW 场景中 PTE 被降级为只读）。
    /// 需要检查实际 PTE 状态时使用 [`pte_flags`]。
    #[must_use]
    pub fn flags(&self) -> PteFlags {
        self.flags
    }

    /// 读取指定偏移所在页的实际 PTE 标志。
    ///
    /// 需要获取页表锁，比 [`flags`] 更重但反映真实状态。
    ///
    /// # Panics
    ///
    /// 映射不存在时 panic（不应在正常使用中发生）。
    #[must_use]
    pub fn pte_flags(&self, offset: usize) -> PteFlags {
        let page_va = (self.vaddr + offset).align_down();
        let guard = self.page_table.lock();
        guard
            .get_mapping(page_va)
            .expect("MappedPagesInner::pte_flags: 映射不存在")
            .1
    }

    /// 获取映射区域内指定偏移处的类型化引用。
    ///
    /// 返回的引用**生命周期绑定到 `&self`**——
    /// 编译器保证映射 drop 后无法使用该引用（use-after-unmap 防护）。
    ///
    /// # Panics
    ///
    /// 在以下条件不满足时 panic：
    /// 1. `offset + size_of::<T>()` 不超过映射大小
    /// 2. 偏移对齐到 `T` 的自然对齐边界
    #[inline]
    pub fn as_type<T: zerocopy::FromBytes>(&self, offset: usize) -> &T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr.as_usize(),
            self.size(),
            offset,
            "MappedPagesInner::as_type",
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // FromBytes 保证任意位模式均为合法 T；
        // &self 保证映射存活，引用生命周期绑定到 self
        unsafe { &*ptr }
    }

    /// 获取映射区域内指定偏移处的可变类型化引用。
    ///
    /// # Panics
    ///
    /// 在以下条件不满足时 panic：
    /// 1. `offset + size_of::<T>()` 不超过映射大小
    /// 2. 映射具有 WRITE 权限（检查实际 PTE，非缓存 flags）
    /// 3. 偏移对齐到 `T` 的自然对齐边界
    ///
    /// **注意**：此方法内部获取页表锁以检查 PTE 可写性。
    /// 调用方不得在已持有页表锁时调用此方法，否则会死锁。
    #[inline]
    pub fn as_type_mut<T: zerocopy::FromBytes + zerocopy::IntoBytes>(
        &mut self,
        offset: usize,
    ) -> &mut T {
        let ptr: *const T = check_bounds_and_align::<T>(
            self.vaddr.as_usize(),
            self.size(),
            offset,
            "MappedPagesInner::as_type_mut",
        );
        // 检查实际 PTE 可写性（而非缓存 flags），COW 安全
        let pte_flags = self.pte_flags(offset);
        assert!(
            pte_flags.is_writable(),
            "MappedPagesInner::as_type_mut: PTE 无 WRITE 权限（可能已被 COW 降级）"
        );
        // SAFETY: check_bounds_and_align 已验证偏移在映射范围内且地址对齐；
        // PTE 可写已验证；FromBytes 保证任意位模式均为合法 T；
        // &mut self 保证映射存活且独占访问，引用生命周期绑定到 self
        unsafe { &mut *(ptr as *mut T) }
    }

    /// 从所属页表中 unmap 所有页，EXCLUSIVE 帧自动回收。
    ///
    /// 使用栈上固定大小数组分块处理，避免堆分配（Drop 可能在中断上下文执行）。
    ///
    /// 每个 chunk 内的操作顺序（SMP 安全）：
    /// 1. 清除 PTE 并收集 EXCLUSIVE 帧地址（在页表锁内）
    /// 2. TLB flush（确保所有核心的 stale TLB 失效）
    /// 3. 回收物理帧（此时没有核心持有指向这些帧的 TLB 条目）
    fn unmap_and_reclaim(&self) {
        let mut offset = 0;
        while offset < self.page_count {
            let n = (self.page_count - offset).min(UNMAP_CHUNK);
            let mut exclusive_pas: heapless::Vec<PhysAddr, UNMAP_CHUNK> = heapless::Vec::new();

            {
                let mut guard = self.page_table.lock();
                for i in 0..n {
                    let va = self.vaddr + (offset + i) * PAGE_SIZE;
                    match guard.unmap_page_with_flags(va) {
                        Ok((pa, flags)) => {
                            if flags.is_exclusive() {
                                exclusive_pas
                                    .push(pa)
                                    .expect("exclusive 帧数不超过 UNMAP_CHUNK");
                            }
                        }
                        Err(e) => {
                            panic!(
                                "MappedPages::unmap_and_reclaim: unmap {va} 失败: {e}——\
                                 MappedPages 保证映射存在，此错误说明内核状态已损坏"
                            );
                        }
                    }
                }
            } // 页表锁释放

            // TLB flush——必须在帧回收之前完成
            {
                let flush_va = self.vaddr + offset * PAGE_SIZE;
                let _flush = tlb::TlbFlushGuard::new(flush_va.as_usize(), n);
            } // TlbFlushGuard drop 触发刷新

            // 所有核心的 TLB 已刷新，安全回收帧
            for pa in &exclusive_pas {
                reclaim_exclusive_frame(*pa);
            }

            offset += n;
        }
    }
}

/// 可回收映射——Drop 时 unmap 并回收 EXCLUSIVE 帧。
///
/// 不可 Clone、不可 Copy（仿射类型约束）。
pub struct MappedPages(MappedPagesInner);

/// 永久映射——Drop 时不执行任何操作。
///
/// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
pub struct PermanentMapping(MappedPagesInner);

impl Deref for MappedPages {
    type Target = MappedPagesInner;
    fn deref(&self) -> &MappedPagesInner {
        &self.0
    }
}

impl DerefMut for MappedPages {
    fn deref_mut(&mut self) -> &mut MappedPagesInner {
        &mut self.0
    }
}

impl Deref for PermanentMapping {
    type Target = MappedPagesInner;
    fn deref(&self) -> &MappedPagesInner {
        &self.0
    }
}

impl DerefMut for PermanentMapping {
    fn deref_mut(&mut self) -> &mut MappedPagesInner {
        &mut self.0
    }
}

impl MappedPages {
    /// Identity-map 一段物理地址区间（VA == PA）。
    ///
    /// **不设置 EXCLUSIVE 位**——drop 时仅 unmap PTE，不释放帧。
    /// 虚拟地址不经过页分配器（VA == PA，由物理布局决定）。
    /// 使用场景：内核启动时的 RAM identity mapping、MMIO 映射。
    ///
    /// # Panics
    ///
    /// - `page_count` 为 0 时 panic。
    /// - 映射冲突时 panic（`identity_map_range` 内部不可恢复）。
    pub fn map_identity(
        pt_ref: Arc<SpinLock<PageTable>>,
        pa_start: PhysAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Result<Self, PagingError> {
        assert!(
            page_count > 0,
            "MappedPages::map_identity: page_count 不能为 0"
        );
        let va_start = VirtAddr::new(pa_start.as_usize());
        let pa_end = pa_start + page_count * PAGE_SIZE;
        {
            let mut pt = pt_ref.lock();
            pt.identity_map_range(pa_start, pa_end, flags);
        }
        Ok(Self(MappedPagesInner {
            vaddr: va_start,
            page_count,
            flags,
            exclusive: false,
            page_table: pt_ref,
        }))
    }

    /// 包装已建立的映射——仅测试使用。
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn wrap_existing(
        pt_ref: Arc<SpinLock<PageTable>>,
        vaddr: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Self {
        debug_assert!(page_count > 0);
        Self(MappedPagesInner {
            vaddr,
            page_count,
            flags,
            exclusive: false,
            page_table: pt_ref,
        })
    }

    /// 分配新帧并建立映射——**设置 EXCLUSIVE 位**。
    ///
    /// 分块处理：每轮先分配一批帧到栈缓冲（不持页表锁），再获取锁批量映射。
    /// 避免堆分配（`Vec` 扩容可能触发页表映射导致递归）。
    /// 帧所有权通过 PTE 的 EXCLUSIVE 位追踪。
    /// 虚拟地址由调用方指定（通常通过 `AddressSpace` 的 VMA 管理）。
    ///
    /// # Panics
    ///
    /// - `page_count` 为 0 时 panic。
    /// - 帧分配失败时 panic（物理内存耗尽不可恢复）。
    /// - 映射冲突时 panic（说明调用方 VA 管理有 bug）。
    pub fn map_alloc(
        pt_ref: Arc<SpinLock<PageTable>>,
        va_start: VirtAddr,
        page_count: usize,
        flags: PteFlags,
    ) -> Self {
        assert!(
            page_count > 0,
            "MappedPages::map_alloc: page_count 不能为 0"
        );
        let exclusive_flags = flags.with_exclusive();
        let mut offset = 0;

        while offset < page_count {
            let n = (page_count - offset).min(MAP_CHUNK);

            // 1. 分配帧到栈缓冲（不持页表锁，不用堆分配）
            let mut frames: heapless::Vec<AllocatedFrames, MAP_CHUNK> = heapless::Vec::new();
            for _ in 0..n {
                let frame =
                    AllocatedFrames::alloc_one().expect("map_alloc: 帧分配失败（物理内存耗尽）");
                frames
                    .push(frame)
                    .unwrap_or_else(|_| panic!("map_alloc: 帧数不超过 MAP_CHUNK"));
            }

            // 2. 获取锁，批量映射
            let mut pt = pt_ref.lock();
            for (i, frame) in frames.into_iter().enumerate() {
                let pa = frame.start_paddr();
                let va = va_start + (offset + i) * PAGE_SIZE;
                pt.map_page(va, pa, exclusive_flags)
                    .expect("map_alloc: map_page 失败（VA 冲突说明调用方 VMA 管理有 bug）");
                // 帧所有权转移到 PTE：forget 阻止 drop 回收
                let mapped = frame.into_mapped();
                core::mem::forget(mapped);
            }
            drop(pt);

            offset += n;
        }

        Self(MappedPagesInner {
            vaddr: va_start,
            page_count,
            flags,
            exclusive: true,
            page_table: pt_ref,
        })
    }

    /// 消耗 self，返回永久映射（drop 时不 unmap）。
    ///
    /// 用于内核 identity mapping、MMIO 等永远不会释放的映射。
    #[must_use]
    pub fn into_permanent(self) -> PermanentMapping {
        let md = core::mem::ManuallyDrop::new(self);
        // SAFETY: self 已被 ManuallyDrop 包装，不会 double-drop。
        // 读取内部 Inner 并转移所有权到 PermanentMapping。
        let inner = unsafe { core::ptr::read(&md.0) };
        PermanentMapping(inner)
    }
}

/// 验证偏移在映射范围内且地址对齐到 `T` 的自然边界，返回目标指针。
///
/// # Panics
///
/// - `T` 为 ZST 时 panic（对映射内存取 ZST 引用无意义）。
/// - 越界或未对齐时 panic。
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

/// 从物理地址重建 `UnmappedFrames` 并 drop 回收——EXCLUSIVE unmap 的核心路径。
fn reclaim_exclusive_frame(pa: PhysAddr) {
    let ppn = PhysPageNum::from(pa);
    let range = FrameRange::new(ppn, ppn + 1);
    // SAFETY: 帧刚从页表 unmap，EXCLUSIVE 保证我们拥有该帧的唯一引用。
    // 构造 UnmappedFrames 使其 Drop 自动归还分配器。
    let _reclaimed = unsafe { UnmappedFrames::from_range(range) };
}

impl Drop for MappedPages {
    fn drop(&mut self) {
        self.0.unmap_and_reclaim();
    }
}

// PermanentMapping 不实现 Drop（unmap）——默认 Drop 仅释放 Arc 引用。

impl core::fmt::Debug for MappedPages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let kind = if self.0.exclusive {
            "exclusive"
        } else {
            "borrowed"
        };
        write!(
            f,
            "MappedPages({}, {} pages, {:?}, {})",
            self.0.vaddr, self.0.page_count, self.0.flags, kind
        )
    }
}

impl core::fmt::Debug for PermanentMapping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PermanentMapping({}, {} pages, {:?})",
            self.0.vaddr, self.0.page_count, self.0.flags
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_pt;
    type Mp = MappedPages;

    /// map_identity 应建立正确的映射并可查询。
    #[test]
    fn map_identity_basic() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x1_0000);
        let mp = Mp::map_identity(pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect("map_identity 应成功");
        assert_eq!(mp.vaddr(), VirtAddr::new(0x1_0000));
        assert_eq!(mp.size(), PAGE_SIZE);
        {
            let guard = pt_ref.lock();
            let (got_pa, got_flags) = guard
                .get_mapping(VirtAddr::new(0x1_0000))
                .expect("应能查到映射");
            assert_eq!(got_pa, pa);
            assert!(!got_flags.is_exclusive());
        }
        drop(mp);
    }

    /// map_identity 重复映射同一 VA 应 panic（identity_map_range 不可恢复）。
    #[test]
    #[should_panic(expected = "映射失败")]
    fn map_identity_conflict_panics() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x2_0000);
        // _mp1 必须保持 permanent，否则 drop 会 unmap 使后续映射不冲突
        let _mp1 = Mp::map_identity(pt_ref.clone(), pa, 1, PteFlags::kernel_rw())
            .expect("首次 map 应成功")
            .into_permanent();
        let _ = Mp::map_identity(pt_ref, pa, 1, PteFlags::kernel_rw());
    }

    /// wrap_existing 应创建非 EXCLUSIVE 映射。
    #[test]
    fn wrap_existing_ownership() {
        let pt_ref = test_pt();
        let mp = Mp::wrap_existing(pt_ref, VirtAddr::new(0x3_0000), 2, PteFlags::kernel_rw());
        assert_eq!(mp.vaddr(), VirtAddr::new(0x3_0000));
        assert_eq!(mp.size(), 2 * PAGE_SIZE);
        assert!(mp.flags().is_writable());
        let dbg = alloc::format!("{:?}", mp);
        assert!(dbg.contains("borrowed"));
        // wrap_existing 没有真实 PTE，转为 PermanentMapping 避免 drop 时 unmap panic
        let _pm = mp.into_permanent();
    }

    /// into_permanent 应返回 PermanentMapping 类型。
    #[test]
    fn into_permanent_marks_permanent() {
        let pt_ref = test_pt();
        let mp = Mp::wrap_existing(pt_ref, VirtAddr::new(0x4_0000), 1, PteFlags::kernel_rw());
        let pm = mp.into_permanent();
        let dbg = alloc::format!("{:?}", pm);
        assert!(dbg.contains("PermanentMapping"));
    }

    /// 多页 map_identity 后逐页查询应都有效。
    #[test]
    fn map_identity_multi_page() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x5_0000);
        let _mp = Mp::map_identity(pt_ref.clone(), pa, 3, PteFlags::kernel_ro())
            .expect("多页 map 应成功");
        let guard = pt_ref.lock();
        for i in 0..3 {
            let va = VirtAddr::new(0x5_0000 + i * PAGE_SIZE);
            assert!(guard.get_mapping(va).is_some(), "第 {} 页应已映射", i);
        }
    }

    /// map_alloc 应分配帧并设置 EXCLUSIVE 位（PTE 级别）。
    #[test]
    fn map_alloc_sets_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x10_0000);
        let mp = Mp::map_alloc(pt_ref.clone(), va, 1, PteFlags::kernel_rw());

        let guard = pt_ref.lock();
        let (_, got_flags) = guard.get_mapping(va).expect("应能查到映射");
        assert!(
            got_flags.is_exclusive(),
            "map_alloc 映射应设置 EXCLUSIVE 位"
        );
        drop(guard);
        // flags() 返回用户原始权限（不含 EXCLUSIVE）
        assert!(!mp.flags().is_exclusive());
        // 通过 pte_flags 查询 PTE 级别的 EXCLUSIVE
        assert!(mp.pte_flags(0).is_exclusive());
        assert_eq!(mp.size(), PAGE_SIZE);
    }

    /// map_alloc 多页后逐页应都有 EXCLUSIVE 位（PTE 级别）。
    #[test]
    fn map_alloc_multi_page_exclusive() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x20_0000);
        let _mp = Mp::map_alloc(pt_ref.clone(), va, 3, PteFlags::kernel_rw());
        let guard = pt_ref.lock();
        for i in 0..3 {
            let page_va = VirtAddr::new(0x20_0000 + i * PAGE_SIZE);
            let (_, flags) = guard.get_mapping(page_va).expect("应已映射");
            assert!(flags.is_exclusive(), "第 {} 页应有 EXCLUSIVE 位", i);
        }
    }

    /// as_type 应返回映射区域内正确偏移处的引用。
    ///
    /// 使用 `wrap_existing` 将堆上真实内存包装为 `PermanentMapping`，
    /// 避免在主机测试中解引用未映射的虚拟地址。
    #[test]
    fn as_type_reads_mapped_memory() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        // wrap_existing 没有真实 PTE，转为 PermanentMapping 避免 drop 时 unmap panic
        let pm = Mp::wrap_existing(pt_ref, va, 1, PteFlags::kernel_rw()).into_permanent();

        let val: &u32 = pm.as_type::<u32>(0);
        assert_eq!(*val, 0);
    }

    /// as_type_mut 应能写入映射区域。
    #[test]
    fn as_type_mut_writes_mapped_memory() {
        let pt_ref = test_pt();
        // 分配 2 页缓冲区以确保能找到页对齐的地址
        let buf = alloc::vec![0u8; 2 * PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize).align_up();
        // 先在页表中建立映射，使 pte_flags 可读
        {
            let mut guard = pt_ref.lock();
            guard
                .map_page(va, PhysAddr::new(va.as_usize()), PteFlags::kernel_rw())
                .expect("map_page 应成功");
        }
        let mut mp = Mp::wrap_existing(pt_ref, va, 1, PteFlags::kernel_rw());

        let val: &mut u32 = mp.as_type_mut::<u32>(0);
        *val = 0xDEAD_BEEF;
        let readback: &u32 = mp.as_type::<u32>(0);
        assert_eq!(*readback, 0xDEAD_BEEF);
    }

    /// as_type 偏移越界应 panic。
    #[test]
    #[should_panic(expected = "超出映射大小")]
    fn as_type_out_of_bounds_panics() {
        let pt_ref = test_pt();
        let buf = alloc::vec![0u8; PAGE_SIZE];
        let va = VirtAddr::new(buf.as_ptr() as usize);
        // wrap_existing 没有真实 PTE，转为 PermanentMapping 避免 drop 时 double-panic
        let pm = Mp::wrap_existing(pt_ref, va, 1, PteFlags::kernel_rw()).into_permanent();
        let _: &u32 = pm.as_type::<u32>(PAGE_SIZE);
    }

    /// page_count 为 0 应 panic。
    #[test]
    #[should_panic(expected = "page_count 不能为 0")]
    fn map_identity_zero_pages_panics() {
        let pt_ref = test_pt();
        let _ = Mp::map_identity(pt_ref, PhysAddr::new(0x1000), 0, PteFlags::kernel_rw());
    }

    /// page_count 为 0 应 panic。
    #[test]
    #[should_panic(expected = "page_count 不能为 0")]
    fn map_alloc_zero_pages_panics() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let _ = Mp::map_alloc(pt_ref, VirtAddr::new(0x1000), 0, PteFlags::kernel_rw());
    }

    /// Drop 非 permanent 的 map_identity 映射应正确 unmap PTE。
    #[test]
    fn drop_identity_unmaps_pte() {
        let pt_ref = test_pt();
        let pa = PhysAddr::new(0x6_0000);
        let va = VirtAddr::new(0x6_0000);
        let mp = Mp::map_identity(pt_ref.clone(), pa, 2, PteFlags::kernel_rw())
            .expect("map_identity 应成功");

        // 确认映射存在
        {
            let guard = pt_ref.lock();
            assert!(guard.get_mapping(va).is_some());
            assert!(guard.get_mapping(va + PAGE_SIZE).is_some());
        }

        // drop 触发 unmap_and_reclaim
        drop(mp);

        // 映射应已被清除
        let guard = pt_ref.lock();
        assert!(guard.get_mapping(va).is_none(), "drop 后映射应已清除");
        assert!(
            guard.get_mapping(va + PAGE_SIZE).is_none(),
            "drop 后第 2 页映射应已清除"
        );
    }

    /// Drop 非 permanent 的 map_alloc 映射应 unmap PTE 并回收 EXCLUSIVE 帧。
    #[test]
    fn drop_alloc_unmaps_and_reclaims() {
        frame_allocator::ensure_test_init();
        let pt_ref = test_pt();
        let va = VirtAddr::new(0x30_0000);
        let mp = Mp::map_alloc(pt_ref.clone(), va, 2, PteFlags::kernel_rw());

        // 确认映射存在且有 EXCLUSIVE 位
        {
            let guard = pt_ref.lock();
            let (_, flags) = guard.get_mapping(va).expect("应能查到映射");
            assert!(flags.is_exclusive());
        }

        // drop 触发 unmap + 帧回收
        drop(mp);

        // 映射应已被清除
        let guard = pt_ref.lock();
        assert!(guard.get_mapping(va).is_none(), "drop 后映射应已清除");
        assert!(
            guard.get_mapping(va + PAGE_SIZE).is_none(),
            "drop 后第 2 页映射应已清除"
        );
    }
}
