//! 同步原语测试——验证 SpinLock 基本功能。

use crate::framework::TestCase;
use sync::SpinLock;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "spinlock_basic",
            run: test_spinlock_basic,
        },
        TestCase {
            name: "spinlock_modify",
            run: test_spinlock_modify,
        },
        TestCase {
            name: "spinlock_not_held_after_drop",
            run: test_spinlock_not_held_after_drop,
        },
    ]
}

fn test_spinlock_basic() {
    let lock = SpinLock::new(42u32, "test_basic");
    let guard = lock.lock();
    assert_eq!(*guard, 42);
}

fn test_spinlock_modify() {
    let lock = SpinLock::new(0u32, "test_modify");
    {
        let mut guard = lock.lock();
        *guard = 99;
    }
    {
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }
}

fn test_spinlock_not_held_after_drop() {
    let lock = SpinLock::new(0u32, "test_drop");
    {
        let _guard = lock.lock();
    }
    assert!(!lock.is_locked());
}
