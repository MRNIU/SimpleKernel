/// exit — 终止当前任务
pub fn exit(code: i32) -> ! {
    crate::task::exit(code)
}

/// yield_now — 主动让出 CPU
pub fn yield_now() {
    crate::task::yield_now();
}

/// clone — 创建子内核线程
///
/// # Errors
///
/// 创建失败时返回 `TaskError`。
pub fn clone(
    name: &'static str,
    entry: fn(usize),
    arg: usize,
) -> Result<usize, crate::task::TaskError> {
    crate::task::clone_kernel_thread(name, entry, arg)
}

/// waitpid — 等待子进程退出
///
/// # Errors
///
/// 找不到子进程时返回 `TaskError::NoChildFound`。
pub fn waitpid(pid: usize) -> Result<(usize, i32), crate::task::TaskError> {
    crate::task::wait_child(pid)
}

/// nanosleep — 睡眠指定毫秒数
pub fn nanosleep(ms: u64) {
    crate::task::sleep_ms(ms);
}

/// kill — 发送信号
///
/// # Errors
///
/// 找不到目标任务时返回 `TaskError::TaskNotFound`。
pub fn kill(pid: usize, sig: crate::task::signal::Signal) -> Result<(), crate::task::TaskError> {
    crate::task::send_signal(pid, sig)
}
