//! 资源标识符——标记任务阻塞等待的具体资源类型。

/// 资源标识符
///
/// 每种资源携带正确类型（Pid/VirtAddr/IrqNumber 等），
/// 替代 C++ 的 bit-packing 方式，编译期类型安全。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceId {
    /// 互斥锁（ID）
    Mutex(u64),
    /// 信号量（ID）
    Semaphore(u64),
    /// 条件变量（ID）
    CondVar(u64),
    /// 等待子进程退出（子进程 PID，0 表示任意子进程）
    ChildExit(usize),
    /// I/O 完成
    IoComplete(u64),
    /// 定时器唤醒
    Timer(u64),
    /// 中断（IRQ 号）
    Interrupt(u32),
    /// 自定义资源
    Custom(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_id_equality() {
        assert_eq!(ResourceId::Mutex(1), ResourceId::Mutex(1));
        assert_ne!(ResourceId::Mutex(1), ResourceId::Mutex(2));
        assert_ne!(ResourceId::Mutex(1), ResourceId::Semaphore(1));
    }

    #[test]
    fn resource_id_debug() {
        let r = ResourceId::ChildExit(42);
        let s = format!("{:?}", r);
        assert!(s.contains("ChildExit"));
        assert!(s.contains("42"));
    }
}
