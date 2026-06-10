// Copyright The SimpleKernel Contributors

//! 作用域退出时执行清理的 RAII guard。

/// RAII 清理 guard，`Drop` 时运行清理函数，`dismiss()` 取消清理。
/// 模式参考 Linux kernel Rust，用于初始化失败回滚。
pub struct ScopeGuard<F: FnOnce()> {
    cleanup: Option<F>,
}

impl<F: FnOnce()> ScopeGuard<F> {
    pub fn new(cleanup: F) -> Self {
        Self {
            cleanup: Some(cleanup),
        }
    }

    /// 取消清理函数，通常在成功路径调用。
    pub fn dismiss(mut self) {
        self.cleanup = None;
    }
}

impl<F: FnOnce()> Drop for ScopeGuard<F> {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn runs_cleanup_on_drop() {
        let ran = Cell::new(false);
        {
            let _guard = ScopeGuard::new(|| ran.set(true));
        }
        assert!(ran.get());
    }

    #[test]
    fn dismiss_prevents_cleanup() {
        let ran = Cell::new(false);
        {
            let guard = ScopeGuard::new(|| ran.set(true));
            guard.dismiss();
        }
        assert!(!ran.get());
    }
}
