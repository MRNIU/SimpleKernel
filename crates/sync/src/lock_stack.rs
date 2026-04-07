//! 锁级别常量 + per-CPU 锁栈——强制锁获取顺序，防止 ABBA 死锁。
//!
//! 每个核心维护一个 [`LockStack`]，记录当前持有锁的级别。
//! 获取新锁时检查级别是否严格递增，违反则 panic。

use core::fmt;

/// 锁级别常量——数值小的必须先获取。
///
/// 持有级别 N 的锁时，只能获取级别 > N 的锁。违反则 panic。
///
/// ```text
/// SCHED(0) -> TASK_TABLE(1) -> INTERRUPT_THREADS(2)
///          -> KERNEL_AS(3) -> KERNEL_PT(4) -> DMA(5)
///          -> FRAME_ALLOC(10) -> PAGE_ALLOC(11) -> HEAP(12)
///          -> PANIC(100) -> CONSOLE(200)
/// ```
pub mod lock_level {
    /// 调度锁——级别最低，必须最先获取
    pub const SCHED: u8 = 0;
    /// 任务表锁
    pub const TASK_TABLE: u8 = 1;
    /// 中断线程锁
    pub const INTERRUPT_THREADS: u8 = 2;
    /// 内核地址空间锁——VMA 操作持有时可能获取 KERNEL_PT
    pub const KERNEL_AS: u8 = 3;
    /// 内核页表锁——map/unmap 持有时可能获取 FRAME_ALLOC / HEAP
    pub const KERNEL_PT: u8 = 4;
    /// DMA 追踪表锁——释放 DMA 缓冲区时可能获取 FRAME_ALLOC
    pub const DMA: u8 = 5;
    /// 帧分配器锁
    pub const FRAME_ALLOC: u8 = 10;
    /// 页分配器锁
    pub const PAGE_ALLOC: u8 = 11;
    /// 堆分配器锁
    pub const HEAP: u8 = 12;
    /// Panic observer 锁——panic 路径可能获取 CONSOLE
    pub const PANIC: u8 = 100;
    /// 控制台锁——级别最高，几乎可在任何上下文获取
    pub const CONSOLE: u8 = 200;
    /// 不参与锁序检查——调试 / 诊断锁专用。
    /// 锁栈对 UNSPECIFIED 级别跳过顺序校验，但仍记录用于诊断。
    pub const UNSPECIFIED: u8 = 255;
}

/// 锁栈条目——记录当前持有的 SpinLock 及其级别。
#[derive(Clone, Copy, Debug)]
pub struct LockStackEntry {
    /// 指向 SpinLock 的类型擦除原始指针，仅用于诊断比较，不会解引用。
    pub lock_ptr: *const (),
    /// 所持锁的级别。
    pub level: u8,
}

// SAFETY: lock_ptr 仅用于比较、从不解引用；访问发生在中断关闭的 per-CPU 上下文中。
unsafe impl Send for LockStackEntry {}
unsafe impl Sync for LockStackEntry {}

/// Per-CPU 锁栈——获取新锁时检查级别是否严格大于栈顶。
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
    /// 特殊处理 [`lock_level::UNSPECIFIED`]：
    /// - 新锁级别为 UNSPECIFIED 时，跳过顺序检查（始终返回 `true`）。
    /// - 栈顶级别为 UNSPECIFIED 时，同样跳过检查。
    ///
    /// 返回 `true` 表示顺序合法，`false` 表示违反顺序。
    #[inline]
    pub fn check_order(&self, new_level: u8) -> bool {
        if new_level == lock_level::UNSPECIFIED {
            return true;
        }
        if self.depth == 0 {
            return true;
        }
        let top = self.entries[self.depth - 1].level;
        if top == lock_level::UNSPECIFIED {
            return true;
        }
        new_level > top
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

impl fmt::Debug for LockStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockStack")
            .field("depth", &self.depth)
            .field("entries", &&self.entries[..self.depth])
            .finish()
    }
}
