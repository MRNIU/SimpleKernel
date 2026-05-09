// Copyright The SimpleKernel Contributors

//! 原始锁机制——`RawLock` trait 抽象 + 各算法实现。

mod ttas;

pub use ttas::RawSpinLock;

/// 原始锁机制 trait——只负责互斥。
///
/// # Safety
///
/// 实现者必须保证：
/// 1. `acquire` 返回后当前核心独占锁
/// 2. `release` 正确释放锁，使其他核心可获取
/// 3. 所有方法在多核并发调用下无 UB
pub unsafe trait RawLock: Send + Sync {
    /// 阻塞获取。
    fn acquire(&self);

    /// 非阻塞尝试，成功返回 `true`。
    fn try_acquire(&self) -> bool;

    /// 释放锁。
    fn release(&self);

    /// 查询锁是否被持有（仅供诊断，结果可能立即过期）。
    fn is_locked(&self) -> bool;

    /// 锁名称（诊断用）。
    fn name(&self) -> &'static str;

    /// 递归加锁检测——默认空实现。
    fn check_recursive(&self) {}

    /// 设置当前核心为 owner——默认空实现。
    fn set_owner(&self) {}

    /// 清除 owner——默认空实现。
    fn clear_owner(&self) {}
}
