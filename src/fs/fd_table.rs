//! 文件描述符表——每任务独立的文件句柄管理。
//!
//! 每个任务（[`TaskControlBlock`]）持有自己的 `FileDescriptorTable`，
//! 管理打开文件的引用。使用 `Arc` 实现文件引用共享（如 dup/fork）。
//!
//! 设计参考：
//! - Linux: `struct files_struct` (`include/linux/fdtable.h`)
//! - Redox OS: `FileDescriptor` scheme

use alloc::sync::Arc;
use alloc::vec::Vec;

use core::fmt;

use sync::SpinLock;

use super::vfs::{FileSystem, FsError, FsResult, InodeId, OpenFlags};

/// 文件描述符——newtype 防止与其他整型混淆。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fd(pub u32);

impl fmt::Display for Fd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fd{}", self.0)
    }
}

/// 打开的文件——关联 inode 与读写状态。
pub struct File {
    /// 所属文件系统
    pub fs: Arc<dyn FileSystem>,
    /// 文件 inode
    pub inode: InodeId,
    /// 当前读写偏移量
    pub offset: u64,
    /// 打开标志
    pub flags: OpenFlags,
}

/// 文件描述符表——管理一个任务的所有打开文件。
///
/// 使用 `Vec<Option<...>>` 实现稀疏数组，支持快速分配最小可用 FD。
pub struct FileDescriptorTable {
    entries: Vec<Option<Arc<SpinLock<File>>>>,
}

/// 初始 FD 表容量
const INITIAL_FD_CAPACITY: usize = 16;

impl FileDescriptorTable {
    /// 创建空的文件描述符表。
    pub fn new() -> Self {
        let mut entries = Vec::with_capacity(INITIAL_FD_CAPACITY);
        // 预留 fd 0/1/2（stdin/stdout/stderr），暂设为 None
        entries.resize_with(3, || None);
        Self { entries }
    }

    /// 分配最小可用 FD，关联到指定文件。
    ///
    /// # Errors
    ///
    /// FD 表达到上限时返回 `FdTableFull`。
    pub fn alloc(&mut self, file: File) -> FsResult<Fd> {
        let wrapped = Arc::new(SpinLock::new(file, "fd"));

        // 查找第一个空槽
        for (i, slot) in self.entries.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(wrapped);
                return Ok(Fd(i as u32));
            }
        }

        // 没有空槽，扩展表
        let fd = self.entries.len() as u32;
        if fd >= 1024 {
            return Err(FsError::FdTableFull);
        }
        self.entries.push(Some(wrapped));
        Ok(Fd(fd))
    }

    /// 获取文件描述符对应的文件引用。
    ///
    /// 返回 `None` 表示 FD 无效或已关闭。
    pub fn get(&self, fd: Fd) -> Option<Arc<SpinLock<File>>> {
        self.entries.get(fd.0 as usize)?.clone()
    }

    /// 关闭文件描述符。
    ///
    /// # Errors
    ///
    /// FD 无效时返回 `InvalidFd`。
    pub fn close(&mut self, fd: Fd) -> FsResult<()> {
        let slot = self
            .entries
            .get_mut(fd.0 as usize)
            .ok_or(FsError::InvalidFd)?;
        if slot.is_none() {
            return Err(FsError::InvalidFd);
        }
        *slot = None; // Arc 引用计数自动递减
        Ok(())
    }
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}
