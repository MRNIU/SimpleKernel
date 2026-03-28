//! 多级页表——walk / map / unmap 逻辑。
//!
//! 通过 [`FrameProvider`] trait 抽象帧分配，裸机和宿主机测试共享同一份 walk 实现。

extern crate alloc;

use alloc::vec::Vec;

use super::{Level0, PageFlags, PageTableEntry, Table, vpn_index};
use crate::error::MemoryError;
use address::{PhysAddr, VirtAddr};

const PT_LEVELS: usize = config::PT_LEVELS;

/// 帧分配抽象——页表遍历时按需分配中间节点。
///
/// 裸机使用 buddy allocator，宿主机测试使用堆分配。
pub trait FrameProvider {
    /// 持有帧所有权的类型
    type Frame;
    /// 分配一个零初始化的页帧
    fn alloc_frame(&mut self) -> Result<Self::Frame, MemoryError>;
    /// 获取帧的物理地址（裸机）或堆地址（测试）
    fn frame_paddr(frame: &Self::Frame) -> PhysAddr;
}

/// 从物理地址构造 `Table<Level0>` 用于 walker 内部。
///
/// walker 在运行时循环中无法使用编译期类型参数区分层级，
/// 统一使用 `Level0` 是因为当前所有层级的 entries 数量相同（均从 PAGE_SIZE 推导）。
/// 索引计算通过 [`vpn_index`] + [`LEVEL_INFO`] 查表实现，不依赖 `L::ENTRIES`。
///
/// # Safety
/// `paddr` 必须指向有效、页对齐的帧（裸机 identity mapping 或测试堆地址）。
#[inline]
unsafe fn table_at(paddr: PhysAddr) -> Table<Level0> {
    // SAFETY: 调用方保证 paddr 有效
    unsafe { Table::<Level0>::from_paddr(paddr) }
}

/// 泛型多级页表——通过 [`FrameProvider`] 参数化帧分配。
///
/// 拥有根帧及所有遍历过程中分配的中间帧。
/// drop 时自动归还所有帧（具体行为由 `F::Frame` 的 Drop 决定）。
pub struct GenericPageTable<F: FrameProvider> {
    root_paddr: PhysAddr,
    root: F::Frame,
    frames: Vec<F::Frame>,
    provider: F,
}

impl<F: FrameProvider> GenericPageTable<F> {
    /// 创建新页表，分配根帧。
    pub fn new(mut provider: F) -> Result<Self, MemoryError> {
        let root = provider.alloc_frame()?;
        let root_paddr = F::frame_paddr(&root);
        Ok(Self {
            root_paddr,
            root,
            frames: Vec::new(),
            provider,
        })
    }

    /// 返回根页表的物理地址（用于写入 satp / TTBR 寄存器）。
    #[inline]
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    /// 遍历到 `va` 对应的叶 PTE，必要时分配中间节点。返回 PTE 裸指针。
    fn find_or_create_pte(&mut self, va: VirtAddr) -> Result<*mut PageTableEntry, MemoryError> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self.root 或 self.frames 持有的有效帧
            let mut table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);

            if !pte.is_valid() {
                let frame = self.provider.alloc_frame()?;
                let frame_paddr = F::frame_paddr(&frame);
                table.write(idx, PageTableEntry::new_intermediate(frame_paddr));
                self.frames.push(frame);
                paddr = frame_paddr;
            } else {
                paddr = pte.paddr();
            }
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Ok(table.entry_ptr(idx))
    }

    /// 只读遍历，不分配。返回 PTE 值的拷贝。
    fn find_pte(&self, va: VirtAddr) -> Option<PageTableEntry> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Some(table.read(idx))
    }

    /// 可变遍历，不分配。返回叶 PTE 裸指针。
    fn find_pte_mut(&mut self, va: VirtAddr) -> Option<*mut PageTableEntry> {
        let mut paddr = self.root_paddr;

        for level in (1..PT_LEVELS).rev() {
            // SAFETY: paddr 指向由 self 持有的有效帧
            let table = unsafe { table_at(paddr) };
            let idx = vpn_index(va, level);
            let pte = table.read(idx);
            if !pte.is_valid() {
                return None;
            }
            paddr = pte.paddr();
        }

        // SAFETY: paddr 指向由 self 持有的有效帧
        let mut table = unsafe { table_at(paddr) };
        let idx = vpn_index(va, 0);
        Some(table.entry_ptr(idx))
    }

    /// 映射单个虚拟页到物理帧。若该 VA 已被映射则返回错误。
    pub fn map_page(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MemoryError> {
        let pte_ptr = self.find_or_create_pte(va)?;
        // SAFETY: find_or_create_pte 返回的指针指向 self 持有的帧内存
        let current = unsafe { pte_ptr.read() };
        if current.is_valid() {
            return Err(MemoryError::MapFailed);
        }
        // SAFETY: pte_ptr 指向 self 持有的帧内存，上方已检查无冲突映射
        unsafe { pte_ptr.write(PageTableEntry::new(pa, flags)) };
        Ok(())
    }

    /// 取消映射单个虚拟页，返回其原始物理地址。
    pub fn unmap_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MemoryError> {
        let pte_ptr = self.find_pte_mut(va).ok_or(MemoryError::PageNotMapped)?;
        // SAFETY: find_pte_mut 返回的指针指向 self 持有的帧内存
        let pte = unsafe { pte_ptr.read() };
        if !pte.is_valid() {
            return Err(MemoryError::PageNotMapped);
        }
        let old_pa = pte.paddr();
        // SAFETY: pte_ptr 指向 self 持有的帧内存，上方已确认该 PTE 有效
        unsafe { pte_ptr.write(PageTableEntry::empty()) };
        Ok(old_pa)
    }

    /// 查询虚拟地址的映射信息，返回物理地址和标志。
    pub fn get_mapping(&self, va: VirtAddr) -> Option<(PhysAddr, PageFlags)> {
        let pte = self.find_pte(va)?;
        if pte.is_valid() && pte.is_leaf() {
            Some((pte.paddr(), pte.flags()))
        } else {
            None
        }
    }
}

#[cfg(target_os = "none")]
use crate::frame::AllocatedFrame;

/// 裸机帧分配——委托给全局 buddy allocator。
#[cfg(target_os = "none")]
pub struct BuddyProvider;

#[cfg(target_os = "none")]
impl FrameProvider for BuddyProvider {
    type Frame = AllocatedFrame;

    fn alloc_frame(&mut self) -> Result<Self::Frame, MemoryError> {
        AllocatedFrame::alloc()
    }

    fn frame_paddr(frame: &Self::Frame) -> PhysAddr {
        frame.paddr()
    }
}

/// 裸机页表类型别名。
#[cfg(target_os = "none")]
pub type PageTable = GenericPageTable<BuddyProvider>;

/// 裸机便捷构造函数——隐藏 `BuddyProvider` 细节。
#[cfg(target_os = "none")]
impl GenericPageTable<BuddyProvider> {
    /// 创建新裸机页表，分配根帧。
    pub fn create() -> Result<Self, MemoryError> {
        Self::new(BuddyProvider)
    }
}

#[cfg(test)]
mod test_support {
    use super::*;
    use config::PAGE_SIZE;

    /// 堆分配的帧，模拟裸机环境的物理帧。
    ///
    /// 使用页对齐分配——PTE 编码会截断低 12 位，
    /// 非对齐地址经 PTE 往返后会丢失偏移量。
    pub(crate) struct TestFrame {
        ptr: *mut u8,
        layout: core::alloc::Layout,
    }

    impl TestFrame {
        fn alloc() -> Self {
            let layout = core::alloc::Layout::from_size_align(PAGE_SIZE, PAGE_SIZE)
                .expect("TestFrame: invalid layout");
            // SAFETY: layout 非零大小
            let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
            assert!(!ptr.is_null(), "TestFrame: allocation failed");
            Self { ptr, layout }
        }
    }

    impl Drop for TestFrame {
        fn drop(&mut self) {
            // SAFETY: ptr 由同 layout 的 alloc_zeroed 分配
            unsafe { std::alloc::dealloc(self.ptr, self.layout) };
        }
    }

    /// 宿主机测试帧分配——使用页对齐堆分配模拟物理帧。
    pub(crate) struct HeapProvider;

    impl FrameProvider for HeapProvider {
        type Frame = TestFrame;

        fn alloc_frame(&mut self) -> Result<Self::Frame, MemoryError> {
            Ok(TestFrame::alloc())
        }

        fn frame_paddr(frame: &Self::Frame) -> PhysAddr {
            PhysAddr::new(frame.ptr as usize)
        }
    }

    /// 测试用页表类型别名。
    pub type TestPageTable = GenericPageTable<HeapProvider>;

    impl GenericPageTable<HeapProvider> {
        /// 创建测试页表。
        pub fn create() -> Self {
            Self::new(HeapProvider).expect("TestPageTable: allocation failed")
        }
    }
}

#[cfg(test)]
pub(crate) use test_support::TestPageTable;
