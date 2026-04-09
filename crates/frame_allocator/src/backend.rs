//! 帧分配器后端接口——抽象底层分配算法，便于替换实现。
//!
//! 当前实现：[`BitmapAllocator`](super::bitmap::BitmapAllocator)。
//! 未来可替换为 buddy+bitmap 等高性能实现，只需实现此 trait。

/// 帧分配器后端——定义底层分配算法的统一接口。
///
/// 所有帧号均为 4K 页号（物理地址 >> 12）。
/// 实现不得在空闲帧内存中存储任何元数据（链表指针等），
/// 确保分配器状态与帧内容完全解耦——帧内容由所有者管理。
pub(crate) trait FrameAllocBackend {
    /// 向分配器注册空闲帧区域 `[start, end)`。
    ///
    /// 可多次调用以添加不连续的空闲区域。
    fn add_frames(&mut self, start: usize, end: usize);

    /// 分配 `count` 个连续帧，返回起始帧号。
    ///
    /// 失败时返回 `None`（内存不足）。
    fn alloc(&mut self, count: usize) -> Option<usize>;

    /// 归还从 `start` 开始的 `count` 个连续帧。
    fn dealloc(&mut self, start: usize, count: usize);
}
