// Copyright The SimpleKernel Contributors

//! SAS 模式 I/O syscall 网关。

/// 写文件描述符（当前为存根）。
///
/// 当前 SAS syscall 网关尚未接入裸指针 I/O；返回 0 表示未写入任何字节。
pub fn write(_fd: usize, _buf: *const u8, _len: usize) -> isize {
    0
}
