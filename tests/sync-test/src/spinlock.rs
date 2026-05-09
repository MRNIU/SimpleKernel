// Copyright The SimpleKernel Contributors

//! SpinLock 公开 API 测试——验证基本加锁/解锁、guard 修改、try_lock 语义。

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

test_harness::test_main!(simplekernel::boot::InitLevel::Full, run_tests);

fn run_tests() {
    test_spinlock_basic();
    log::info!("test spinlock_basic ... ok");

    test_spinlock_modify();
    log::info!("test spinlock_modify ... ok");

    test_spinlock_not_held_after_drop();
    log::info!("test spinlock_not_held_after_drop ... ok");

    test_try_lock_when_free();
    log::info!("test try_lock_when_free ... ok");

    log::info!("sync-spinlock-test: all 4 tests passed");
}

/// SpinLock 基本操作：创建、加锁、读值。
fn test_spinlock_basic() {
    let lock = sync::SpinLock::new(42u32, "test_basic", sync::lock_level::UNSPECIFIED);
    let guard = lock.lock();
    assert_eq!(*guard, 42);
}

/// 通过 guard 修改值，释放后重新获取验证。
fn test_spinlock_modify() {
    let lock = sync::SpinLock::new(0u32, "test_modify", sync::lock_level::UNSPECIFIED);
    {
        let mut guard = lock.lock();
        *guard = 99;
    }
    {
        let guard = lock.lock();
        assert_eq!(*guard, 99);
    }
}

/// guard drop 后锁应释放。
fn test_spinlock_not_held_after_drop() {
    let lock = sync::SpinLock::new(0u32, "test_drop", sync::lock_level::UNSPECIFIED);
    {
        let _guard = lock.lock();
    }
    assert!(!lock.is_locked());
}

/// try_lock 在锁空闲时应成功。
fn test_try_lock_when_free() {
    let lock = sync::SpinLock::new(0u32, "test_try", sync::lock_level::UNSPECIFIED);
    let guard = lock.try_lock();
    assert!(guard.is_some());
}
