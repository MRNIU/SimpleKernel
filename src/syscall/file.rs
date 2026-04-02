/// open — 打开文件（或创建）
///
/// # Errors
///
/// 文件不存在且未指定 CREATE 标志时返回错误。
pub fn open(path: &str, flags: u32) -> Result<u32, crate::fs::vfs::FsError> {
    use crate::fs::fd_table::File;
    use crate::fs::vfs::{FileType, FsError, OpenFlags};

    let open_flags = OpenFlags(flags);
    let create = (flags & OpenFlags::CREATE.0) != 0;

    // 尝试解析路径
    let result = crate::fs::resolve_path(path);

    let (fs, inode) = match result {
        Ok((fs, inode)) => (fs, inode),
        Err(FsError::NotFound) if create => {
            // 路径不存在且指定了 CREATE——创建文件
            crate::fs::create_file(path, FileType::Regular)?
        }
        Err(e) => return Err(e),
    };

    let file = File {
        fs,
        inode,
        offset: 0,
        flags: open_flags,
    };

    // TODO: 当前使用全局 FD 表（无 per-task），待 TCB 集成后切换
    static GLOBAL_FD_TABLE: spin::Once<sync::SpinLock<crate::fs::fd_table::FileDescriptorTable>> =
        spin::Once::new();
    let table = GLOBAL_FD_TABLE.call_once(|| {
        sync::SpinLock::new(crate::fs::fd_table::FileDescriptorTable::new(), "global_fd")
    });

    let fd = table.lock().alloc(file)?;
    Ok(fd.0)
}

/// close — 关闭文件描述符
///
/// # Errors
///
/// 无效 FD 返回 `InvalidFd`。
pub fn close(fd: u32) -> Result<(), crate::fs::vfs::FsError> {
    use crate::fs::fd_table::Fd;

    // TODO: 切换为 per-task FD 表
    static GLOBAL_FD_TABLE: spin::Once<sync::SpinLock<crate::fs::fd_table::FileDescriptorTable>> =
        spin::Once::new();
    let table = GLOBAL_FD_TABLE.call_once(|| {
        sync::SpinLock::new(crate::fs::fd_table::FileDescriptorTable::new(), "global_fd")
    });

    table.lock().close(Fd(fd))
}

/// read — 从文件描述符读取
///
/// # Errors
///
/// FD 无效或 I/O 错误时返回错误。
pub fn read(fd: u32, buf: &mut [u8]) -> Result<usize, crate::fs::vfs::FsError> {
    use crate::fs::fd_table::Fd;
    use crate::fs::vfs::FsError;

    static GLOBAL_FD_TABLE: spin::Once<sync::SpinLock<crate::fs::fd_table::FileDescriptorTable>> =
        spin::Once::new();
    let table = GLOBAL_FD_TABLE.call_once(|| {
        sync::SpinLock::new(crate::fs::fd_table::FileDescriptorTable::new(), "global_fd")
    });

    let file_ref = table.lock().get(Fd(fd)).ok_or(FsError::InvalidFd)?;

    let mut file = file_ref.lock();
    let n = file.fs.read(file.inode, file.offset, buf)?;
    file.offset += n as u64;
    Ok(n)
}

/// sys_write — 写入文件描述符
///
/// # Errors
///
/// FD 无效或 I/O 错误时返回错误。
pub fn write(fd: u32, data: &[u8]) -> Result<usize, crate::fs::vfs::FsError> {
    use crate::fs::fd_table::Fd;
    use crate::fs::vfs::FsError;

    static GLOBAL_FD_TABLE: spin::Once<sync::SpinLock<crate::fs::fd_table::FileDescriptorTable>> =
        spin::Once::new();
    let table = GLOBAL_FD_TABLE.call_once(|| {
        sync::SpinLock::new(crate::fs::fd_table::FileDescriptorTable::new(), "global_fd")
    });

    let file_ref = table.lock().get(Fd(fd)).ok_or(FsError::InvalidFd)?;

    let mut file = file_ref.lock();
    let n = file.fs.write(file.inode, file.offset, data)?;
    file.offset += n as u64;
    Ok(n)
}

/// mkdir — 创建目录
///
/// # Errors
///
/// 父目录不存在或目录已存在时返回错误。
pub fn mkdir(path: &str) -> Result<(), crate::fs::vfs::FsError> {
    use crate::fs::vfs::FileType;
    crate::fs::create_file(path, FileType::Directory)?;
    Ok(())
}
