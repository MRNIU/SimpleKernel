//! 文件系统测试——验证 VFS、RamFS、FD 表基本操作。

use crate::framework::TestCase;
use alloc::sync::Arc;

pub fn tests() -> &'static [TestCase] {
    &[
        TestCase {
            name: "vfs_resolve_root",
            run: test_vfs_resolve_root,
        },
        TestCase {
            name: "ramfs_create_write_read",
            run: test_ramfs_create_write_read,
        },
        TestCase {
            name: "ramfs_mkdir_readdir",
            run: test_ramfs_mkdir_readdir,
        },
        TestCase {
            name: "ramfs_unlink",
            run: test_ramfs_unlink,
        },
        TestCase {
            name: "vfs_path_resolution",
            run: test_vfs_path_resolution,
        },
    ]
}

/// 根路径 "/" 应能解析到 RamFS 根目录。
fn test_vfs_resolve_root() {
    let (fs, inode) = simplekernel::fs::resolve_path("/").expect("resolve / 应成功");
    assert_eq!(fs.name(), "ramfs");
    let stat = fs.stat(inode).expect("stat root 应成功");
    assert_eq!(stat.file_type, simplekernel::fs::vfs::FileType::Directory);
}

/// RamFS 创建文件 → 写入 → 读取 全流程。
fn test_ramfs_create_write_read() {
    use simplekernel::fs::vfs::{FileSystem, FileType};

    let (fs, root) = simplekernel::fs::resolve_path("/").expect("resolve /");

    // 创建文件
    let inode = fs
        .create(root, "test_rw.txt", FileType::Regular)
        .expect("create 应成功");

    // 写入
    let data = b"system test data";
    let written = fs.write(inode, 0, data).expect("write 应成功");
    assert_eq!(written, data.len());

    // 读取
    let mut buf = [0u8; 64];
    let read = fs.read(inode, 0, &mut buf).expect("read 应成功");
    assert_eq!(read, data.len());
    assert_eq!(&buf[..read], data);

    // 清理
    fs.unlink(root, "test_rw.txt").expect("unlink");
}

/// RamFS mkdir + readdir。
fn test_ramfs_mkdir_readdir() {
    use simplekernel::fs::vfs::{FileSystem, FileType};

    let (fs, root) = simplekernel::fs::resolve_path("/").expect("resolve /");
    let dir_id = fs.mkdir(root, "test_dir").expect("mkdir 应成功");

    // 在目录中创建文件
    fs.create(dir_id, "a.txt", FileType::Regular)
        .expect("create a.txt");
    fs.create(dir_id, "b.txt", FileType::Regular)
        .expect("create b.txt");

    let entries = fs.readdir(dir_id).expect("readdir 应成功");
    assert_eq!(entries.len(), 2);

    // 清理
    fs.unlink(dir_id, "a.txt").expect("unlink a");
    fs.unlink(dir_id, "b.txt").expect("unlink b");
    fs.unlink(root, "test_dir").expect("unlink dir");
}

/// RamFS unlink——删除后 lookup 应返回 None。
fn test_ramfs_unlink() {
    use simplekernel::fs::vfs::{FileSystem, FileType};

    let (fs, root) = simplekernel::fs::resolve_path("/").expect("resolve /");
    fs.create(root, "to_delete.txt", FileType::Regular)
        .expect("create");

    // lookup 应找到
    assert!(fs.lookup(root, "to_delete.txt").expect("lookup").is_some());

    // 删除
    fs.unlink(root, "to_delete.txt").expect("unlink");

    // lookup 应返回 None
    assert!(
        fs.lookup(root, "to_delete.txt")
            .expect("lookup after unlink")
            .is_none()
    );
}

/// VFS 路径解析——多级目录。
fn test_vfs_path_resolution() {
    use simplekernel::fs::vfs::{FileSystem, FileType};

    let (fs, root) = simplekernel::fs::resolve_path("/").expect("resolve /");

    // 创建多级目录
    let dir_a = fs.mkdir(root, "path_a").expect("mkdir path_a");
    let dir_b = fs.mkdir(dir_a, "path_b").expect("mkdir path_b");
    let file_id = fs
        .create(dir_b, "deep.txt", FileType::Regular)
        .expect("create deep.txt");

    // 路径解析
    let (_, resolved) =
        simplekernel::fs::resolve_path("/path_a/path_b/deep.txt").expect("resolve deep path");
    assert_eq!(resolved, file_id);

    // 清理
    fs.unlink(dir_b, "deep.txt").expect("unlink");
    fs.unlink(dir_a, "path_b").expect("unlink dir_b");
    fs.unlink(root, "path_a").expect("unlink dir_a");
}
