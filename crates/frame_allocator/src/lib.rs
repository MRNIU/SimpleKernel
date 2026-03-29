//! 物理帧分配器 + typestate 生命周期追踪。
//!
//! 参考 Theseus OS 的 `Frames<const S: MemoryState>` 设计：
//! - 帧以**范围**（[`FrameRange`]）为单位管理，而非单个地址
//! - 完整状态机：`Free → Allocated → Mapped → Unmapped → Free → …`
//! - 状态转换消费 self，编译期强制正确的生命周期路径
//! - 只有 `Free` 状态直接与 buddy allocator 交互——所有解分配路径汇聚到 `Free`
//! - Drop 按状态分派：`Mapped` 状态 panic（必须经 unmap），其余经由 `Free` 归还

#![cfg_attr(not(any(test, feature = "test-support")), no_std)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

mod alloc;
mod error;
mod state;
mod transitions;

pub use alloc::init;
pub use error::FrameAllocError;
pub use state::{AllocatedFrames, Frames, FreeFrames, MappedFrames, MemoryState, UnmappedFrames};

/// 测试用帧分配器初始化——分配堆内存模拟物理内存区域。
///
/// 全局 static 只能 init 一次，用 `std::sync::Once` 保证幂等。
///
/// 通过 `test-support` feature 或 `cfg(test)` 启用。
#[cfg(any(test, feature = "test-support"))]
pub fn ensure_test_init() {
    use address::PhysAddr;
    use config::PAGE_SIZE;

    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let layout =
            std::alloc::Layout::from_size_align(64 * PAGE_SIZE, PAGE_SIZE).expect("layout");
        // SAFETY: layout 有效且非零大小
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        assert!(!ptr.is_null());
        let start = PhysAddr::new(ptr as usize);
        // SAFETY: 测试专用内存区域，不与其他分配重叠
        unsafe { init(start, 64 * PAGE_SIZE) };
    });
}
