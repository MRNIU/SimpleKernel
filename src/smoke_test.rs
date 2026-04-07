//! 系统集成冒烟测试——验证各子系统在 QEMU 上正确运行。
//!
//! 由 `main.rs` 中 bootstrap 调用，线程测试作为独立内核线程运行。
//! 引导序列中的内联冒烟测试已移至 `boot.rs::kernel_init()`。

use core::sync::atomic::{AtomicU32, Ordering};

use simplekernel::syscall;
use simplekernel::task;
use sync::SpinLock;

static TEST_COUNTER: SpinLock<u32> =
    SpinLock::new(0, "test_counter", sync::lock_level::UNSPECIFIED);
static TEST_DONE: AtomicU32 = AtomicU32::new(0);

const ITERS_PER_THREAD: u32 = 200;
const TEST_THREAD_COUNT: u32 = 4;

/// 锁竞争测试线程
fn counter_thread(id: usize) {
    let core = per_cpu::current_core_id();
    log::info!("counter_{}: start on core {}", id, core);

    for i in 0..ITERS_PER_THREAD {
        {
            let mut guard = TEST_COUNTER.lock();
            *guard += 1;
        }
        if i % 50 == 0 {
            let core = per_cpu::current_core_id();
            log::info!("counter_{}: i={} on core {}", id, i, core);
        }
        syscall::process::yield_now();
    }

    TEST_DONE.fetch_add(1, Ordering::Release);
    log::info!(
        "counter_{}: done on core {}",
        id,
        per_cpu::current_core_id()
    );
}

/// 验证线程——等所有 counter 线程完成后检查计数
fn verifier_thread(_arg: usize) {
    loop {
        if TEST_DONE.load(Ordering::Acquire) >= TEST_THREAD_COUNT {
            break;
        }
        syscall::process::yield_now();
    }

    let count = *TEST_COUNTER.lock();
    let expected = TEST_THREAD_COUNT * ITERS_PER_THREAD;
    if count == expected {
        log::info!(
            "=== LOCK TEST PASSED: counter={} (expected {}) ===",
            count,
            expected
        );
    } else {
        log::error!(
            "=== LOCK TEST FAILED: counter={} (expected {}) ===",
            count,
            expected
        );
    }
}

/// P5b 综合测试
fn p5b_test_thread(_arg: usize) {
    log::info!("P5b: testing sleep_ms(500)...");
    let tick_before = global_tick::current();
    syscall::process::nanosleep(500);
    let tick_after = global_tick::current();
    let elapsed = tick_after.saturating_sub(tick_before);
    // 500ms @ 10Hz = 5 ticks（全局计数器被双核推进，实际约 10），允许 ≥3
    log::info!("P5b: sleep_ms(500) elapsed {} ticks", elapsed);
    if elapsed >= 3 {
        log::info!("=== SLEEP TEST PASSED ===");
    } else {
        log::error!("=== SLEEP TEST FAILED: elapsed={} < 3 ===", elapsed);
    }

    log::info!("P5b: testing clone/wait...");
    match syscall::process::clone("child", child_thread, 42) {
        Ok(child_pid) => {
            log::info!("P5b: spawned child pid={}", child_pid);
            match syscall::process::waitpid(child_pid) {
                Ok((pid, code)) => {
                    if pid == child_pid && code == 7 {
                        log::info!("=== CLONE/WAIT TEST PASSED: pid={} code={} ===", pid, code);
                    } else {
                        log::error!(
                            "=== CLONE/WAIT TEST FAILED: pid={} code={} (expected {}:7) ===",
                            pid,
                            code,
                            child_pid
                        );
                    }
                }
                Err(e) => log::error!("=== CLONE/WAIT TEST FAILED: wait err={:?} ===", e),
            }
        }
        Err(e) => log::error!("=== CLONE/WAIT TEST FAILED: clone err={:?} ===", e),
    }

    log::info!("P5b: testing KMutex...");
    static KMUTEX: spin::Once<task::mutex::KMutex> = spin::Once::new();
    KMUTEX.call_once(task::mutex::KMutex::new);
    let km = KMUTEX.get().expect("KMutex not initialized");
    km.lock();
    log::info!("P5b: KMutex acquired");
    km.unlock();
    log::info!("=== KMUTEX TEST PASSED ===");

    log::info!("P5b: testing signal...");
    match syscall::process::clone("victim", victim_thread, 0) {
        Ok(victim_pid) => {
            syscall::process::yield_now();
            match syscall::process::kill(victim_pid, task::signal::Signal::SIGKILL) {
                Ok(()) => {
                    log::info!("P5b: sent SIGKILL to pid={}", victim_pid);
                    match syscall::process::waitpid(victim_pid) {
                        Ok((pid, code)) => {
                            log::info!(
                                "=== SIGNAL TEST PASSED: pid={} killed, code={} ===",
                                pid,
                                code
                            );
                        }
                        Err(e) => log::error!("=== SIGNAL TEST FAILED: wait err={:?} ===", e),
                    }
                }
                Err(e) => log::error!("=== SIGNAL TEST FAILED: send err={:?} ===", e),
            }
        }
        Err(e) => log::error!("=== SIGNAL TEST FAILED: clone err={:?} ===", e),
    }

    log::info!("=== ALL P5b TESTS COMPLETE ===");
}

fn child_thread(arg: usize) {
    log::info!("child_thread: arg={}", arg);
    syscall::process::yield_now();
    syscall::process::exit(7);
}

fn victim_thread(_arg: usize) {
    log::info!("victim_thread: running, waiting for signal...");
    loop {
        syscall::process::yield_now();
    }
}

/// P6/P7 综合测试——在内核线程中验证设备 + 文件系统操作。
fn p6p7_test_thread(_arg: usize) {
    use simplekernel::fs::vfs::{FileSystem, FileType};

    log::info!("P6P7: testing device manager...");
    let dev_count = simplekernel::device::manager::device_count();
    log::info!("P6P7: {} devices registered", dev_count);

    log::info!("P6P7: testing VFS in kernel thread...");
    let (fs, root) = simplekernel::fs::resolve_path("/").expect("resolve /");

    // 创建文件并写入
    let inode = fs
        .create(root, "thread_test.txt", FileType::Regular)
        .expect("create");
    let data = b"written from kernel thread";
    fs.write(inode, 0, data).expect("write");

    // 让出 CPU，验证上下文切换后文件仍可读
    syscall::process::yield_now();

    // 读回验证
    let mut buf = [0u8; 64];
    let n = fs.read(inode, 0, &mut buf).expect("read");
    assert_eq!(&buf[..n], data);
    log::info!("P6P7: VFS read after yield OK");

    // 通过路径解析查找
    let (_, resolved) = simplekernel::fs::resolve_path("/thread_test.txt").expect("resolve");
    assert_eq!(resolved, inode);

    // 清理
    fs.unlink(root, "thread_test.txt").expect("unlink");

    log::info!("=== P6P7 TEST PASSED ===");
}

/// 创建所有冒烟测试线程。
pub fn spawn_all() {
    log::info!(
        "Phase 5: spawning {} counter threads + 1 verifier + P5b + P6P7 tests",
        TEST_THREAD_COUNT
    );

    static NAMES: [&str; 4] = ["counter_0", "counter_1", "counter_2", "counter_3"];
    for (i, name) in NAMES.iter().enumerate() {
        task::spawn_kernel_thread(name, counter_thread, i);
    }
    task::spawn_kernel_thread("verifier", verifier_thread, 0);
    task::spawn_kernel_thread("p5b_test", p5b_test_thread, 0);
    task::spawn_kernel_thread("p6p7_test", p6p7_test_thread, 0);
}
