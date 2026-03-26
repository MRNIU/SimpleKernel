/// 锁栈条目——记录当前持有的 SpinLock 及其级别。
#[derive(Clone, Copy)]
pub struct LockStackEntry {
    /// 指向 SpinLock 的类型擦除原始指针，仅用于诊断比较，不会解引用。
    pub lock_ptr: *const (),
    /// 所持锁的级别。
    pub level: u8,
}

// SAFETY: lock_ptr 仅用于比较、从不解引用；访问发生在中断关闭的 per-CPU 上下文中。
unsafe impl Send for LockStackEntry {}
unsafe impl Sync for LockStackEntry {}

/// Per-CPU 锁栈，用于强制锁获取顺序。
///
/// 每个核心维护一个当前持有锁的栈。
/// 获取新锁时，SpinLock 检查新锁的级别是否高于栈顶。
pub struct LockStack {
    pub entries: [LockStackEntry; Self::MAX_DEPTH],
    pub depth: usize,
}

impl LockStack {
    pub const MAX_DEPTH: usize = 8;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [LockStackEntry {
                lock_ptr: core::ptr::null(),
                level: 0,
            }; Self::MAX_DEPTH],
            depth: 0,
        }
    }
}
