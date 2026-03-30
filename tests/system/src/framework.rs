//! 系统测试框架——轻量 TestRunner，cargo test 风格输出。
//!
//! 当前限制：测试中的 panic 会终止整个测试内核（bare-metal 环境无法捕获 panic）。
//! 所有 assert 失败都是致命的，QEMU 将以非零退出码退出。

use alloc::vec::Vec;

pub struct TestCase {
    pub name: &'static str,
    pub run: fn(),
}

pub struct TestGroup {
    pub name: &'static str,
    pub tests: &'static [TestCase],
}

/// 测试运行器——收集所有测试组并执行，输出 cargo test 风格的结果。
///
/// 测试中的 panic 会直接终止测试内核。如果所有测试正常返回，
/// 则视为全部通过并输出汇总结果。
pub struct TestRunner {
    groups: Vec<TestGroup>,
    total_passed: usize,
}

impl TestRunner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            total_passed: 0,
        }
    }

    pub fn add_group(&mut self, group: TestGroup) {
        self.groups.push(group);
    }

    /// 执行所有测试组，返回 `true` 表示全部通过。
    ///
    /// 如果某个测试 panic，panic handler 会终止 QEMU，此函数不会返回。
    pub fn run(&mut self) -> bool {
        log::info!("=== SimpleKernel System Tests ===");
        log::info!("");

        for group in &self.groups {
            let count = group.tests.len();
            log::info!("running {} tests in group \"{}\"", count, group.name);

            for test in group.tests {
                (test.run)();
                log::info!("test {} ... ok", test.name);
                self.total_passed += 1;
            }

            log::info!("group result: ok. {} passed; 0 failed", count);
            log::info!("");
        }

        log::info!("================================");
        log::info!("test result: ok. {} passed; 0 failed", self.total_passed,);

        true
    }
}
