//! 内核阻塞互斥锁——竞争时任务进入 Blocked 状态，而非自旋等待。

#[cfg(target_os = "none")]
mod inner {
    use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use crate::task::resource_id::ResourceId;

    const NO_OWNER: usize = usize::MAX;

    /// 内核阻塞互斥锁
    ///
    /// 竞争时任务进入 Blocked 状态（让出 CPU），而非自旋等待。
    /// 适用于可能长时间持有的锁。
    pub struct KMutex {
        /// 互斥锁 ID（用于 ResourceId）
        id: u64,
        /// 当前持有者 PID（NO_OWNER 表示未持有）
        owner: AtomicUsize,
    }

    /// KMutex ID 计数器
    static NEXT_MUTEX_ID: AtomicU64 = AtomicU64::new(1);

    impl KMutex {
        /// 创建新的阻塞互斥锁。
        pub fn new() -> Self {
            Self {
                id: NEXT_MUTEX_ID.fetch_add(1, Ordering::Relaxed),
                owner: AtomicUsize::new(NO_OWNER),
            }
        }

        /// 获取锁——如果锁已被持有，当前任务进入 Blocked 状态。
        pub fn lock(&self) {
            loop {
                let current_pid = crate::task::current_task().pid();
                match self.owner.compare_exchange(
                    NO_OWNER,
                    current_pid,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return,
                    Err(owner) => {
                        if owner == current_pid {
                            panic!("KMutex: recursive lock by pid={}", current_pid);
                        }
                        crate::task::block_on(ResourceId::Mutex(self.id));
                    }
                }
            }
        }

        /// 释放锁——唤醒一个等待任务。
        pub fn unlock(&self) {
            self.owner.store(NO_OWNER, Ordering::Release);
            crate::task::wakeup_one(ResourceId::Mutex(self.id));
        }
    }
}

#[cfg(target_os = "none")]
pub use inner::KMutex;
