//! TLB 管理——架构无关的 TLB 刷新接口 + 跨核 shootdown 回调。
//!
//! # 在内存子系统中的定位
//!
//! PTE 修改后必须刷新 TLB，否则 CPU 继续使用过期缓存。
//! 本 crate 提供 RAII 守卫模式（[`TlbFlushGuard`]），确保：
//! - 页表修改完成后才 flush（避免 race）
//! - 帧回收之前已 flush（避免 stale access）
//! - 多核环境自动触发 shootdown IPI
//!
//! ```text
//! paging::OwnedPages / PageTable (修改 PTE)
//!    │
//!    ▼ TlbFlushGuard::new(va, count)
//! tlb (本 crate)
//!    │
//!    ├── arch::flush_tlb_page() (本核)
//!    └── TLB_SHOOTDOWN_FN (跨核 IPI)
//! ```

#![no_std]

/// 跨核 TLB shootdown 请求类型。
///
/// 类型安全的替代方案，避免用 `0` / 非零 约定区分全局/单页刷新。
#[derive(Debug, Clone, Copy)]
pub enum TlbFlushRequest {
    /// 刷新整个 TLB
    All,
    /// 刷新指定虚拟地址的单条 TLB 表项
    Page(usize),
}

/// 跨核 TLB shootdown 回调函数。
///
/// 由中断子系统通过 [`register_tlb_shootdown`] 注册，实际 IPI 发送逻辑由调用方实现。
static TLB_SHOOTDOWN_FN: spin::Once<fn(TlbFlushRequest)> = spin::Once::new();

/// 注册跨核 TLB shootdown 回调。
///
/// 在中断子系统初始化 IPI 后调用一次。注册后，所有 TLB 刷新操作
/// 会在本核刷新后自动调用此回调通知其他核心。
pub fn register_tlb_shootdown(f: fn(TlbFlushRequest)) {
    TLB_SHOOTDOWN_FN.call_once(|| f);
}

/// RAII TLB 刷新守卫——drop 时自动执行 TLB 刷新。
///
/// 将 TLB 刷新延迟到守卫 drop 时执行，确保在页表修改完成后、
/// 帧回收之前统一刷新——避免"锁内 unmap、锁外 flush"的竞态窗口。
pub struct TlbFlushGuard {
    /// 起始虚拟地址
    start_vaddr: usize,
    /// 需要刷新的页数
    page_count: usize,
}

impl TlbFlushGuard {
    /// 创建连续范围的 TLB 刷新守卫。
    pub fn new(start_vaddr: usize, page_count: usize) -> Self {
        Self {
            start_vaddr,
            page_count,
        }
    }
}

impl Drop for TlbFlushGuard {
    fn drop(&mut self) {
        if self.page_count == 0 {
            return;
        }
        if self.page_count <= config::TLB_FLUSH_THRESHOLD {
            for i in 0..self.page_count {
                flush_tlb_page(self.start_vaddr + i * config::PAGE_SIZE);
            }
        } else {
            flush_tlb();
        }
    }
}

/// 刷新整个 TLB——用于批量页表操作（切换地址空间、初始化映射等）。
///
/// 单页 unmap 应使用 [`flush_tlb_page`] 避免不必要的全局刷新。
///
/// # TODO
///
/// - **ASID 支持**：当前 TLB flush 是全局的（所有 ASID），引入用户进程后
///   每次进程切换都需全局 flush，代价很高。后续应为每个地址空间分配 ASID，
///   使用 `sfence.vma rs1, rs2`（RISC-V）/ `tlbi aside1, <asid>`（AArch64）
///   实现按 ASID 刷新，避免影响其他进程的 TLB 缓存。
#[inline(always)]
pub fn flush_tlb() {
    arch::flush_tlb_all();

    if let Some(shootdown) = TLB_SHOOTDOWN_FN.get() {
        shootdown(TlbFlushRequest::All);
    }
}

/// 刷新指定虚拟地址对应的单条 TLB 表项。
///
/// 在 unmap 单页或修改单个 PTE 后调用，比 [`flush_tlb`] 精确、开销更低。
#[inline(always)]
pub fn flush_tlb_page(vaddr: usize) {
    arch::flush_tlb_page(vaddr);

    if let Some(shootdown) = TLB_SHOOTDOWN_FN.get() {
        shootdown(TlbFlushRequest::Page(vaddr));
    }
}
