/// sys_write — 写文件描述符（当前为存根）
pub fn sys_write(_fd: u64, _buf: u64, _len: u64) -> i64 {
    0
}
