//! 系统测试框架——轻量 TestRunner，cargo test 风格输出。

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

/// 单个测试用例
pub struct TestCase {
    /// 测试名称
    pub name: &'static str,
    /// 测试执行函数
    pub run: fn(),
}

/// 测试组——逻辑相关的一组测试用例
pub struct TestGroup {
    /// 组名称
    pub name: &'static str,
    /// 组内测试用例列表
    pub tests: &'static [TestCase],
}

/// 标记当前是否在测试执行上下文中（panic handler 据此判断是否标记失败而非终止）
pub static IN_TEST: AtomicBool = AtomicBool::new(false);

/// 标记当前测试用例是否已失败（由 panic handler 设置）
pub static CURRENT_TEST_FAILED: AtomicBool = AtomicBool::new(false);

/// 测试运行器——收集所有测试组并执行，输出 cargo test 风格的结果。
pub struct TestRunner {
    groups: Vec<TestGroup>,
    total_passed: usize,
    total_failed: usize,
    failures: Vec<(&'static str, &'static str)>,
}

impl TestRunner {
    /// 创建新的测试运行器
    #[must_use]
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            total_passed: 0,
            total_failed: 0,
            failures: Vec::new(),
        }
    }

    /// 添加一个测试组
    pub fn add_group(&mut self, group: TestGroup) {
        self.groups.push(group);
    }

    /// 执行所有测试组，返回 `true` 表示全部通过。
    pub fn run(&mut self) -> bool {
        log::info!("=== SimpleKernel System Tests ===");
        log::info!("");

        for group in &self.groups {
            let count = group.tests.len();
            log::info!("running {} tests in group \"{}\"", count, group.name);

            let mut group_passed = 0usize;
            let mut group_failed = 0usize;

            for test in group.tests {
                // 准备测试上下文
                CURRENT_TEST_FAILED.store(false, Ordering::SeqCst);
                IN_TEST.store(true, Ordering::SeqCst);

                // 执行测试
                (test.run)();

                // 收集结果
                IN_TEST.store(false, Ordering::SeqCst);
                let failed = CURRENT_TEST_FAILED.load(Ordering::SeqCst);

                if failed {
                    log::info!("test {} ... FAILED", test.name);
                    group_failed += 1;
                    self.failures.push((group.name, test.name));
                } else {
                    log::info!("test {} ... ok", test.name);
                    group_passed += 1;
                }
            }

            let status = if group_failed == 0 { "ok" } else { "FAILED" };
            log::info!(
                "group result: {}. {} passed; {} failed",
                status,
                group_passed,
                group_failed
            );
            log::info!("");

            self.total_passed += group_passed;
            self.total_failed += group_failed;
        }

        log::info!("================================");

        if !self.failures.is_empty() {
            log::info!("");
            log::info!("failures:");
            for (group, test) in &self.failures {
                log::info!("    {}::{}", group, test);
            }
            log::info!("");
        }

        let status = if self.total_failed == 0 {
            "ok"
        } else {
            "FAILED"
        };
        log::info!(
            "test result: {}. {} passed; {} failed",
            status,
            self.total_passed,
            self.total_failed
        );

        self.total_failed == 0
    }
}
