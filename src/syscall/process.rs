// Copyright The SimpleKernel Contributors

//! SAS 模式进程与任务相关 syscall 网关。

/// exit — 终止当前任务。
///
/// # Panics
///
/// 当前任务或调度器状态未初始化时 panic。
pub fn exit(code: i32) -> ! {
    crate::task::exit(code)
}

/// yield_now — 主动让出 CPU。
///
/// # Panics
///
/// 当前任务或调度器状态未初始化时 panic。
pub fn yield_now() {
    crate::task::yield_now();
}

/// clone — 创建子内核线程
///
/// # Errors
///
/// 创建失败时返回 `TaskError`。
///
/// # Panics
///
/// 当前任务或调度器状态未初始化时 panic。
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
///
/// # Panics
///
/// 当前任务或调度器状态未初始化时 panic。
pub fn waitpid(pid: usize) -> Result<(usize, i32), crate::task::TaskError> {
    crate::task::wait_child(pid)
}

/// nanosleep — 睡眠指定毫秒数。
///
/// # Panics
///
/// 当前任务或调度器状态未初始化时 panic。
pub fn nanosleep(ms: u64) {
    crate::task::sleep_ms(ms);
}

/// kill — 发送信号
///
/// # Errors
///
/// 找不到目标任务时返回 `TaskError::TaskNotFound`。
///
/// # Panics
///
/// 当前核心调度状态未初始化时 panic。
pub fn kill(pid: usize, sig: crate::task::signal::Signal) -> Result<(), crate::task::TaskError> {
    crate::task::send_signal(pid, sig)
}
