// Copyright The SimpleKernel Contributors

//! 不可恢复错误的停机入口。

/// 输出消息并永久停机。
#[cfg(not(test))]
#[cold]
#[inline(never)]
pub fn halt(msg: &str) -> ! {
    crate::logging::raw_put(msg);
    loop {
        core::hint::spin_loop();
    }
}

/// 测试配置下以 panic 代替永久停机。
///
/// # Panics
///
/// 始终 panic，并把停机消息写入 panic 文本。
#[cfg(test)]
pub fn halt(msg: &str) -> ! {
    panic!("测试模式调用 halt: msg={msg}");
}
