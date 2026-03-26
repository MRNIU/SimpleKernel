/// sys_exit — 终止当前任务
pub fn sys_exit(code: u64) -> i64 {
    crate::task::exit(code as i32);
}

/// sys_yield — 主动让出 CPU
pub fn sys_yield() -> i64 {
    crate::task::yield_now();
    0
}

/// sys_clone — 创建子内核线程（简化版，entry 和 arg 需为合法函数指针）
pub fn sys_clone(_entry: u64, _arg: u64) -> i64 {
    // P5b 简化版：仅在内核态创建线程，用户态 clone 在 P6 实现
    // 此处返回 -1 表示暂不支持通过 syscall 创建线程
    -1
}

/// sys_waitpid — 等待子进程退出
pub fn sys_waitpid(pid: u64) -> i64 {
    match crate::task::wait_child(pid as usize) {
        Ok((child_pid, code)) => {
            // 将 pid 和 code 编码在返回值中：高 32 位 = pid，低 32 位 = code
            ((child_pid as i64) << 32) | ((code as i64) & 0xFFFF_FFFF)
        }
        Err(_) => -1,
    }
}

/// sys_nanosleep — 睡眠指定毫秒数
pub fn sys_nanosleep(ms: u64) -> i64 {
    crate::task::sleep_ms(ms);
    0
}

/// sys_kill — 发送信号
pub fn sys_kill(pid: u64, sig: u64) -> i64 {
    use crate::task::signal::Signal;
    let signal = match Signal::from_u8(sig as u8) {
        Some(s) => s,
        None => return -1,
    };
    match crate::task::send_signal(pid as usize, signal) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
