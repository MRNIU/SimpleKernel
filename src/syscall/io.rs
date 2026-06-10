// Copyright The SimpleKernel Contributors

//! SAS 模式 I/O syscall 网关。

/// write — 写文件描述符（当前为存根）
pub fn write(_fd: usize, _buf: *const u8, _len: usize) -> isize {
    0
}
