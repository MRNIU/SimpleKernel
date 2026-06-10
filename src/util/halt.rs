// Copyright The SimpleKernel Contributors

//! 不可恢复错误的停机入口。

#[cfg(not(test))]
#[cold]
#[inline(never)]
pub fn halt(msg: &str) -> ! {
    crate::logging::raw_put(msg);
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
pub fn halt(msg: &str) -> ! {
    panic!("{}", msg);
}
