//! 虚拟文件系统（VFS）——统一的文件系统操作接口。
//!
//! 所有具体文件系统（RamFS、FatFS 等）实现 [`FileSystem`] trait，
//! 上层代码通过 trait 对象统一操作，替代 C++ 的函数指针 vtable。
//!
//! 参考：
//! - Linux VFS: `include/linux/fs.h` (struct super_operations, inode_operations)
//! - Redox OS: `scheme` 抽象

use alloc::string::String;
use alloc::vec::Vec;

use core::fmt;

/// Inode 标识符——文件系统内部唯一标识一个文件或目录。
///
/// 不同文件系统的 InodeId 空间独立，不可跨文件系统比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InodeId(pub u64);

impl fmt::Display for InodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inode#{}", self.0)
    }
}

/// 文件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// 普通文件
    Regular,
    /// 目录
    Directory,
    /// 符号链接
    SymLink,
}

/// 目录项——目录内的一个条目。
pub struct DirEntry {
    /// 条目名称
    pub name: String,
    /// 对应 inode
    pub inode: InodeId,
    /// 文件类型
    pub file_type: FileType,
}

/// Inode 元数据。
pub struct InodeStat {
    /// 文件大小（字节）
    pub size: u64,
    /// 文件类型
    pub file_type: FileType,
    /// 权限位（POSIX 风格，如 0o755）
    pub permissions: u32,
}

/// 文件打开标志。
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags(pub u32);

impl OpenFlags {
    /// 只读
    pub const RDONLY: Self = Self(0);
    /// 只写
    pub const WRONLY: Self = Self(1);
    /// 读写
    pub const RDWR: Self = Self(2);
    /// 如不存在则创建
    pub const CREATE: Self = Self(0x40);
    /// 截断为零
    pub const TRUNC: Self = Self(0x200);
    /// 追加写
    pub const APPEND: Self = Self(0x400);
}

/// 文件系统错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// 文件或目录不存在
    NotFound,
    /// 文件已存在
    AlreadyExists,
    /// 不是目录
    NotADirectory,
    /// 是目录（不能对目录执行文件操作）
    IsADirectory,
    /// 文件描述符表已满
    FdTableFull,
    /// 无效的文件描述符
    InvalidFd,
    /// I/O 错误
    IoError,
    /// 不支持的操作
    NotSupported,
    /// 权限不足
    PermissionDenied,
    /// 目录非空
    DirectoryNotEmpty,
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for FsError {}

/// 文件系统操作结果。
pub type FsResult<T> = Result<T, FsError>;

/// 文件系统操作 trait——trait 对象替代 C++ 函数指针表。
///
/// 每个具体文件系统（RamFS、FatFS 等）实现此 trait。
/// VFS 层持有 `Arc<dyn FileSystem>` 进行多态分发。
pub trait FileSystem: Send + Sync {
    /// 文件系统名称（如 "ramfs"、"fatfs"）。
    fn name(&self) -> &str;

    /// 返回根目录的 InodeId。
    fn root_inode(&self) -> InodeId;

    /// 在父目录中查找指定名称的条目。
    ///
    /// # Errors
    ///
    /// 父 inode 不是目录时返回 `NotADirectory`。
    fn lookup(&self, parent: InodeId, name: &str) -> FsResult<Option<InodeId>>;

    /// 在父目录下创建文件/目录。
    ///
    /// # Errors
    ///
    /// 同名条目已存在返回 `AlreadyExists`。
    fn create(&self, parent: InodeId, name: &str, file_type: FileType) -> FsResult<InodeId>;

    /// 读取文件内容。
    ///
    /// 从 `offset` 位置开始读取，最多填满 `buf`，返回实际读取字节数。
    fn read(&self, inode: InodeId, offset: u64, buf: &mut [u8]) -> FsResult<usize>;

    /// 写入文件内容。
    ///
    /// 从 `offset` 位置开始写入 `data`，返回实际写入字节数。
    /// 文件不足时自动扩展。
    fn write(&self, inode: InodeId, offset: u64, data: &[u8]) -> FsResult<usize>;

    /// 创建子目录。
    ///
    /// # Errors
    ///
    /// 同名条目已存在返回 `AlreadyExists`。
    fn mkdir(&self, parent: InodeId, name: &str) -> FsResult<InodeId>;

    /// 删除父目录中的指定条目。
    ///
    /// # Errors
    ///
    /// 条目不存在返回 `NotFound`，目录非空返回 `DirectoryNotEmpty`。
    fn unlink(&self, parent: InodeId, name: &str) -> FsResult<()>;

    /// 读取目录内容。
    ///
    /// # Errors
    ///
    /// inode 不是目录时返回 `NotADirectory`。
    fn readdir(&self, inode: InodeId) -> FsResult<Vec<DirEntry>>;

    /// 获取 inode 元数据。
    fn stat(&self, inode: InodeId) -> FsResult<InodeStat>;
}
