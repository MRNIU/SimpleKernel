//! 内存文件系统（RamFS）——数据完全存储在内核堆中。
//!
//! 适用于 `/tmp`、`/proc` 等不需要持久化的挂载点。
//! 所有数据在关机后丢失。
//!
//! 参考：
//! - Linux tmpfs: `mm/shmem.c`
//! - Theseus OS: `RamFS` crate

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use core::sync::atomic::{AtomicU64, Ordering};

use sync::SpinLock;

use super::vfs::{DirEntry, FileSystem, FileType, FsError, FsResult, InodeId, InodeStat};

/// RamFS 内部 inode 数据。
enum RamInodeData {
    /// 普通文件——内容存储在 `Vec<u8>` 中
    File(Vec<u8>),
    /// 目录——子条目列表 (名称, inode ID, 文件类型)
    Directory(Vec<(String, InodeId, FileType)>),
}

/// RamFS inode。
struct RamInode {
    data: RamInodeData,
    permissions: u32,
}

/// 内存文件系统实现。
pub struct RamFs {
    /// inode 存储（BTreeMap 保证有序遍历）
    inodes: SpinLock<BTreeMap<InodeId, RamInode>>,
    /// 下一个可用 inode ID（原子递增）
    next_id: AtomicU64,
    /// 根目录 inode ID
    root: InodeId,
}

impl RamFs {
    /// 创建新的 RamFS 实例，自动创建根目录。
    pub fn new() -> Self {
        let root_id = InodeId(0);
        let root_inode = RamInode {
            data: RamInodeData::Directory(Vec::new()),
            permissions: 0o755,
        };
        let mut inodes = BTreeMap::new();
        inodes.insert(root_id, root_inode);

        Self {
            inodes: SpinLock::new(inodes, "ramfs", sync::lock_level::UNSPECIFIED),
            next_id: AtomicU64::new(1),
            root: root_id,
        }
    }

    /// 分配新的 inode ID。
    fn alloc_inode_id(&self) -> InodeId {
        InodeId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

impl FileSystem for RamFs {
    fn name(&self) -> &str {
        "ramfs"
    }

    fn root_inode(&self) -> InodeId {
        self.root
    }

    fn lookup(&self, parent: InodeId, name: &str) -> FsResult<Option<InodeId>> {
        let inodes = self.inodes.lock();
        let parent_inode = inodes.get(&parent).ok_or(FsError::NotFound)?;
        match &parent_inode.data {
            RamInodeData::Directory(entries) => {
                let found = entries
                    .iter()
                    .find(|(n, _, _)| n == name)
                    .map(|(_, id, _)| *id);
                Ok(found)
            }
            RamInodeData::File(_) => Err(FsError::NotADirectory),
        }
    }

    fn create(&self, parent: InodeId, name: &str, file_type: FileType) -> FsResult<InodeId> {
        let new_id = self.alloc_inode_id();
        let mut inodes = self.inodes.lock();

        // 检查父目录存在且是目录
        let parent_inode = inodes.get_mut(&parent).ok_or(FsError::NotFound)?;
        let entries = match &mut parent_inode.data {
            RamInodeData::Directory(entries) => entries,
            RamInodeData::File(_) => return Err(FsError::NotADirectory),
        };

        // 检查是否已存在
        if entries.iter().any(|(n, _, _)| n == name) {
            return Err(FsError::AlreadyExists);
        }

        // 添加目录项
        entries.push((String::from(name), new_id, file_type));

        // 创建新 inode
        let new_inode = match file_type {
            FileType::Regular => RamInode {
                data: RamInodeData::File(Vec::new()),
                permissions: 0o644,
            },
            FileType::Directory => RamInode {
                data: RamInodeData::Directory(Vec::new()),
                permissions: 0o755,
            },
            FileType::SymLink => RamInode {
                data: RamInodeData::File(Vec::new()),
                permissions: 0o777,
            },
        };
        inodes.insert(new_id, new_inode);

        Ok(new_id)
    }

    fn read(&self, inode: InodeId, offset: u64, buf: &mut [u8]) -> FsResult<usize> {
        let inodes = self.inodes.lock();
        let node = inodes.get(&inode).ok_or(FsError::NotFound)?;
        match &node.data {
            RamInodeData::File(content) => {
                let offset = offset as usize;
                if offset >= content.len() {
                    return Ok(0);
                }
                let available = content.len() - offset;
                let n = buf.len().min(available);
                buf[..n].copy_from_slice(&content[offset..offset + n]);
                Ok(n)
            }
            RamInodeData::Directory(_) => Err(FsError::IsADirectory),
        }
    }

    fn write(&self, inode: InodeId, offset: u64, data: &[u8]) -> FsResult<usize> {
        let mut inodes = self.inodes.lock();
        let node = inodes.get_mut(&inode).ok_or(FsError::NotFound)?;
        match &mut node.data {
            RamInodeData::File(content) => {
                let offset = offset as usize;
                let end = offset + data.len();
                // 自动扩展文件
                if end > content.len() {
                    content.resize(end, 0);
                }
                content[offset..end].copy_from_slice(data);
                Ok(data.len())
            }
            RamInodeData::Directory(_) => Err(FsError::IsADirectory),
        }
    }

    fn mkdir(&self, parent: InodeId, name: &str) -> FsResult<InodeId> {
        self.create(parent, name, FileType::Directory)
    }

    fn unlink(&self, parent: InodeId, name: &str) -> FsResult<()> {
        let mut inodes = self.inodes.lock();

        // 从父目录中查找并移除条目
        let parent_inode = inodes.get_mut(&parent).ok_or(FsError::NotFound)?;
        let entries = match &mut parent_inode.data {
            RamInodeData::Directory(entries) => entries,
            RamInodeData::File(_) => return Err(FsError::NotADirectory),
        };

        let pos = entries
            .iter()
            .position(|(n, _, _)| n == name)
            .ok_or(FsError::NotFound)?;

        let (_, child_id, child_type) = entries[pos].clone();

        // 如果是目录，检查是否为空
        if child_type == FileType::Directory {
            if let Some(child_inode) = inodes.get(&child_id) {
                if let RamInodeData::Directory(child_entries) = &child_inode.data {
                    if !child_entries.is_empty() {
                        return Err(FsError::DirectoryNotEmpty);
                    }
                }
            }
        }

        // 移除目录项和 inode
        let parent_inode = inodes.get_mut(&parent).expect("父目录刚刚验证过");
        if let RamInodeData::Directory(entries) = &mut parent_inode.data {
            entries.swap_remove(pos);
        }
        inodes.remove(&child_id);

        Ok(())
    }

    fn readdir(&self, inode: InodeId) -> FsResult<Vec<DirEntry>> {
        let inodes = self.inodes.lock();
        let node = inodes.get(&inode).ok_or(FsError::NotFound)?;
        match &node.data {
            RamInodeData::Directory(entries) => {
                let result = entries
                    .iter()
                    .map(|(name, id, ft)| DirEntry {
                        name: name.clone(),
                        inode: *id,
                        file_type: *ft,
                    })
                    .collect();
                Ok(result)
            }
            RamInodeData::File(_) => Err(FsError::NotADirectory),
        }
    }

    fn stat(&self, inode: InodeId) -> FsResult<InodeStat> {
        let inodes = self.inodes.lock();
        let node = inodes.get(&inode).ok_or(FsError::NotFound)?;
        match &node.data {
            RamInodeData::File(content) => Ok(InodeStat {
                size: content.len() as u64,
                file_type: FileType::Regular,
                permissions: node.permissions,
            }),
            RamInodeData::Directory(entries) => Ok(InodeStat {
                size: entries.len() as u64,
                file_type: FileType::Directory,
                permissions: node.permissions,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建 RamFS 后根目录应存在且为空。
    #[test]
    fn ramfs_root_exists() {
        let fs = RamFs::new();
        let root = fs.root_inode();
        let stat = fs.stat(root).expect("root stat 应成功");
        assert_eq!(stat.file_type, FileType::Directory);
        assert_eq!(stat.size, 0); // 空目录
    }

    /// 创建文件、写入、读取、stat 全流程。
    #[test]
    fn ramfs_create_write_read() {
        let fs = RamFs::new();
        let root = fs.root_inode();

        // 创建文件
        let inode = fs
            .create(root, "hello.txt", FileType::Regular)
            .expect("create 应成功");

        // 写入
        let data = b"Hello, world!";
        let written = fs.write(inode, 0, data).expect("write 应成功");
        assert_eq!(written, data.len());

        // 读取
        let mut buf = [0u8; 32];
        let read = fs.read(inode, 0, &mut buf).expect("read 应成功");
        assert_eq!(read, data.len());
        assert_eq!(&buf[..read], data);

        // stat
        let stat = fs.stat(inode).expect("stat 应成功");
        assert_eq!(stat.size, data.len() as u64);
        assert_eq!(stat.file_type, FileType::Regular);
    }

    /// mkdir + readdir + unlink 测试。
    #[test]
    fn ramfs_mkdir_readdir_unlink() {
        let fs = RamFs::new();
        let root = fs.root_inode();

        // 创建目录
        let dir_id = fs.mkdir(root, "tmp").expect("mkdir 应成功");
        assert_ne!(dir_id, root);

        // readdir 应包含 "tmp"
        let entries = fs.readdir(root).expect("readdir 应成功");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "tmp");
        assert_eq!(entries[0].file_type, FileType::Directory);

        // 在子目录中创建文件
        let file_id = fs
            .create(dir_id, "foo.txt", FileType::Regular)
            .expect("create in subdir 应成功");

        // 删除非空目录应失败
        assert_eq!(fs.unlink(root, "tmp"), Err(FsError::DirectoryNotEmpty));

        // 先删除文件，再删除目录
        fs.unlink(dir_id, "foo.txt").expect("unlink file 应成功");
        fs.unlink(root, "tmp").expect("unlink empty dir 应成功");

        // readdir 应为空
        let entries = fs.readdir(root).expect("readdir after unlink");
        assert!(entries.is_empty());
    }

    /// lookup 测试——存在和不存在的条目。
    #[test]
    fn ramfs_lookup() {
        let fs = RamFs::new();
        let root = fs.root_inode();

        // 不存在
        assert_eq!(fs.lookup(root, "nope").expect("lookup 应成功"), None);

        // 创建后存在
        let inode = fs
            .create(root, "file", FileType::Regular)
            .expect("create 应成功");
        assert_eq!(fs.lookup(root, "file").expect("lookup 应成功"), Some(inode));
    }

    /// 重复创建同名文件应返回 AlreadyExists。
    #[test]
    fn ramfs_duplicate_create() {
        let fs = RamFs::new();
        let root = fs.root_inode();
        fs.create(root, "dup", FileType::Regular)
            .expect("first create");
        assert_eq!(
            fs.create(root, "dup", FileType::Regular),
            Err(FsError::AlreadyExists)
        );
    }
}
