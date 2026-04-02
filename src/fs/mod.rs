//! 文件系统子系统——VFS 层、RamFS、挂载管理。
//!
//! 架构：
//! - `vfs.rs`：`FileSystem` trait + 基础类型（InodeId, DirEntry, FileType 等）
//! - `fd_table.rs`：每任务文件描述符表
//! - `ramfs.rs`：内存文件系统实现
//! - `fatfs_adapter.rs`：VirtIO 块设备 → fatfs crate 适配
//! - `mod.rs`：挂载表、路径解析、全局初始化

pub mod fatfs_adapter;
pub mod fd_table;
pub mod ramfs;
pub mod vfs;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use sync::SpinLock;

use vfs::{FileSystem, FsError, FsResult, InodeId};

/// 挂载表条目。
struct MountEntry {
    /// 挂载点路径（如 "/"、"/mnt"）
    path: String,
    /// 挂载的文件系统
    fs: Arc<dyn FileSystem>,
}

/// 全局挂载表。
static MOUNT_TABLE: SpinLock<Vec<MountEntry>> = SpinLock::new(Vec::new(), "mount_table");

/// 挂载文件系统到指定路径。
///
/// # Errors
///
/// 当前实现不检查重复挂载。
pub fn mount(path: &str, fs: Arc<dyn FileSystem>) {
    log::info!("VFS: mounting {} at {}", fs.name(), path);
    MOUNT_TABLE.lock().push(MountEntry {
        path: String::from(path),
        fs,
    });
}

/// 路径解析——从路径找到对应的文件系统和 inode。
///
/// 1. 在挂载表中找到最长匹配前缀
/// 2. 逐级 lookup 目录
/// 3. 返回 (fs, inode)
///
/// # Errors
///
/// 路径不存在时返回 `NotFound`。
pub fn resolve_path(path: &str) -> FsResult<(Arc<dyn FileSystem>, InodeId)> {
    let mount_table = MOUNT_TABLE.lock();

    // 找到最长匹配前缀的挂载点
    let entry = mount_table
        .iter()
        .filter(|e| path.starts_with(&e.path))
        .max_by_key(|e| e.path.len())
        .ok_or(FsError::NotFound)?;

    let fs = entry.fs.clone();
    let mount_path_len = entry.path.len();
    drop(mount_table); // 释放锁

    // 获取挂载点之后的相对路径
    let relative = &path[mount_path_len..];
    let relative = relative.trim_start_matches('/');

    // 从根目录逐级查找
    let mut current = fs.root_inode();

    if relative.is_empty() {
        return Ok((fs, current));
    }

    for component in relative.split('/') {
        if component.is_empty() {
            continue;
        }
        current = fs.lookup(current, component)?.ok_or(FsError::NotFound)?;
    }

    Ok((fs, current))
}

/// 在指定路径创建文件。
///
/// 解析父目录路径后在其中创建新条目。
///
/// # Errors
///
/// 父目录不存在或文件已存在时返回错误。
pub fn create_file(
    path: &str,
    file_type: vfs::FileType,
) -> FsResult<(Arc<dyn FileSystem>, InodeId)> {
    let (parent_path, name) = split_parent_name(path)?;
    let (fs, parent_inode) = resolve_path(parent_path)?;
    let inode = fs.create(parent_inode, name, file_type)?;
    Ok((fs, inode))
}

/// 分离路径的父目录和文件名。
fn split_parent_name(path: &str) -> FsResult<(&str, &str)> {
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(pos) => {
            let parent = if pos == 0 { "/" } else { &path[..pos] };
            let name = &path[pos + 1..];
            if name.is_empty() {
                return Err(FsError::NotFound);
            }
            Ok((parent, name))
        }
        None => Err(FsError::NotFound), // 无绝对路径
    }
}

/// 初始化文件系统子系统——挂载 RamFS 到根目录。
pub fn fs_init() {
    log::info!("FileSystemInit: mounting RamFS at /");
    let ramfs = Arc::new(ramfs::RamFs::new());
    mount("/", ramfs);

    // VFS 冒烟测试
    vfs_smoke_test();

    log::info!("FileSystemInit complete");
}

/// VFS 冒烟测试——验证基本文件操作。
fn vfs_smoke_test() {
    // mkdir /tmp
    let (fs, root) = resolve_path("/").expect("resolve / 应成功");
    let tmp_id = fs.mkdir(root, "tmp").expect("mkdir /tmp 应成功");
    log::info!("VFS test: mkdir /tmp OK");

    // create /tmp/hello.txt
    let file_id = fs
        .create(tmp_id, "hello.txt", vfs::FileType::Regular)
        .expect("create /tmp/hello.txt 应成功");
    log::info!("VFS test: create /tmp/hello.txt OK");

    // write
    let data = b"Hello, world!";
    let written = fs.write(file_id, 0, data).expect("write 应成功");
    log::info!("VFS test: write {} bytes OK", written);

    // read
    let mut buf = [0u8; 32];
    let read = fs.read(file_id, 0, &mut buf).expect("read 应成功");
    let content = core::str::from_utf8(&buf[..read]).expect("UTF-8 解码失败");
    log::info!("VFS test: read \"{}\" OK", content);

    // unlink
    fs.unlink(tmp_id, "hello.txt").expect("unlink 应成功");
    log::info!("VFS test: unlink /tmp/hello.txt OK");

    // 验证路径解析
    let (_, resolved) = resolve_path("/tmp").expect("resolve /tmp 应成功");
    assert_eq!(resolved, tmp_id);
}
