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
/// 最大深度由 `config::LOCK_STACK_DEPTH` 控制。
pub struct LockStack {
    entries: [LockStackEntry; Self::MAX_DEPTH],
    depth: usize,
}

impl LockStack {
    pub const MAX_DEPTH: usize = config::LOCK_STACK_DEPTH;

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

    /// 当前栈深度。
    #[inline]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// 检查锁顺序是否合法——新锁的级别必须严格大于栈顶级别。
    ///
    /// `unclassified` 参数为"未分级"的级别值，该级别跳过检查。
    /// 返回 `true` 表示顺序合法，`false` 表示违反顺序。
    #[inline]
    pub fn check_order(&self, new_level: u8, unclassified: u8) -> bool {
        if new_level == unclassified {
            return true;
        }
        if self.depth > 0 {
            let top = self.entries[self.depth - 1].level;
            if top != unclassified && new_level <= top {
                return false;
            }
        }
        true
    }

    /// 获取锁时压栈。
    ///
    /// # Panics
    /// 栈满时 panic。
    #[inline]
    pub fn push(&mut self, lock_ptr: *const (), level: u8) {
        assert!(
            self.depth < Self::MAX_DEPTH,
            "lock stack overflow (depth={})",
            self.depth
        );
        self.entries[self.depth] = LockStackEntry { lock_ptr, level };
        self.depth += 1;
    }

    /// 释放锁时弹栈。
    ///
    /// # Panics
    /// 栈空或栈顶不匹配时 panic。
    #[inline]
    pub fn pop(&mut self, lock_ptr: *const ()) {
        assert!(self.depth > 0, "lock stack underflow");
        assert_eq!(
            self.entries[self.depth - 1].lock_ptr,
            lock_ptr,
            "lock stack corrupted — 释放顺序与获取顺序不一致"
        );
        self.depth -= 1;
    }
}

impl Default for LockStack {
    fn default() -> Self {
        Self::new()
    }
}
