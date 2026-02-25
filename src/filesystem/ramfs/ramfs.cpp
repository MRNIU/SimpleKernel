/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "ramfs.hpp"

#include "kernel_log.hpp"
#include "sk_cstring"
#include "vfs.hpp"

namespace ramfs {

using namespace vfs;

// Inode 操作表
static InodeOps g_ramfs_inode_ops = {
    .lookup = RamFs::RamLookup,
    .create = RamFs::RamCreate,
    .unlink = RamFs::RamUnlink,
    .mkdir = RamFs::RamMkdir,
    .rmdir = RamFs::RamRmdir,
};

// File 操作表
static FileOps g_ramfs_file_ops = {
    .read = RamFs::RamRead,
    .write = RamFs::RamWrite,
    .seek = RamFs::RamSeek,
    .close = RamFs::RamClose,
    .readdir = RamFs::RamReadDir,
};

auto RamFs::GetFileOps() -> FileOps* { return &g_ramfs_file_ops; }

RamFs::RamFs()
    : inodes_{},
      free_list_(nullptr),
      root_inode_(nullptr),
      used_inodes_(0),
      mounted_(false) {}

RamFs::~RamFs() {
  if (mounted_) {
    Unmount();
  }
}

auto RamFs::GetName() const -> const char* { return "ramfs"; }

auto RamFs::Mount(BlockDevice* device) -> Expected<Inode*> {
  if (mounted_) {
    return std::unexpected(Error(ErrorCode::kFsAlreadyMounted));
  }

  if (device != nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  klog::Info("RamFs: mounting...\n");

  // 初始化 inode 空闲链表
  free_list_ = nullptr;
  for (int i = kMaxInodes - 1; i >= 0; --i) {
    inodes_[i].next_free = free_list_;
    free_list_ = &inodes_[i];
  }

  // 分配根目录 inode
  auto root_result = AllocateInode();
  if (!root_result.has_value()) {
    return std::unexpected(root_result.error());
  }

  root_inode_ = root_result.value();
  root_inode_->type = FileType::kDirectory;
  root_inode_->permissions = 0755;

  // 初始化根目录数据
  RamInode* ram_root = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(root_inode_) - offsetof(RamInode, inode));
  ram_root->data = nullptr;
  ram_root->capacity = 0;
  ram_root->child_count = 0;

  used_inodes_ = 1;
  mounted_ = true;

  klog::Info("RamFs: mounted successfully\n");
  return root_inode_;
}

auto RamFs::Unmount() -> Expected<void> {
  if (!mounted_) {
    return std::unexpected(Error(ErrorCode::kFsNotMounted));
  }

  klog::Info("RamFs: unmounting...\n");

  // 释放所有 inode 的数据缓冲区
  for (size_t i = 0; i < kMaxInodes; ++i) {
    if (inodes_[i].inode.type != FileType::kUnknown &&
        inodes_[i].data != nullptr) {
      delete[] static_cast<uint8_t*>(inodes_[i].data);
      inodes_[i].data = nullptr;
    }
  }

  // 重置状态
  free_list_ = nullptr;
  root_inode_ = nullptr;
  used_inodes_ = 0;
  mounted_ = false;

  klog::Info("RamFs: unmounted\n");
  return {};
}

auto RamFs::Sync() -> Expected<void> {
  // ramfs 是纯内存文件系统，无需同步
  return {};
}

auto RamFs::AllocateInode() -> Expected<Inode*> {
  if (free_list_ == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  RamInode* ram_inode = free_list_;
  free_list_ = free_list_->next_free;

  // 初始化 inode
  ram_inode->inode.ino = reinterpret_cast<uint64_t>(&ram_inode->inode);
  ram_inode->inode.type = FileType::kUnknown;
  ram_inode->inode.size = 0;
  ram_inode->inode.permissions = 0644;
  ram_inode->inode.link_count = 1;
  ram_inode->inode.fs_private = ram_inode;
  ram_inode->inode.fs = this;
  ram_inode->inode.ops = &g_ramfs_inode_ops;

  ram_inode->data = nullptr;
  ram_inode->capacity = 0;
  ram_inode->child_count = 0;
  ram_inode->next_free = nullptr;

  ++used_inodes_;
  return &ram_inode->inode;
}

auto RamFs::FreeInode(Inode* inode) -> Expected<void> {
  if (inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamInode* ram_inode = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(inode) - offsetof(RamInode, inode));

  // 释放数据缓冲区
  if (ram_inode->data != nullptr) {
    delete[] static_cast<uint8_t*>(ram_inode->data);
    ram_inode->data = nullptr;
  }

  // 重置 inode
  ram_inode->inode.type = FileType::kUnknown;
  ram_inode->inode.size = 0;
  ram_inode->inode.fs_private = nullptr;
  ram_inode->inode.ops = nullptr;

  ram_inode->capacity = 0;
  ram_inode->child_count = 0;

  // 加入空闲链表
  ram_inode->next_free = free_list_;
  free_list_ = ram_inode;

  --used_inodes_;
  return {};
}

auto RamFs::GetRootInode() const -> Inode* { return root_inode_; }

auto RamFs::FindInDirectory(RamInode* dir, const char* name)
    -> RamFs::RamDirEntry* {
  if (dir == nullptr || dir->inode.type != FileType::kDirectory ||
      dir->data == nullptr) {
    return nullptr;
  }

  RamDirEntry* entries = static_cast<RamDirEntry*>(dir->data);
  for (size_t i = 0; i < dir->child_count; ++i) {
    if (strcmp(entries[i].name, name) == 0) {
      return &entries[i];
    }
  }

  return nullptr;
}

auto RamFs::AddToDirectory(RamInode* dir, const char* name, Inode* inode)
    -> Expected<void> {
  if (dir == nullptr || name == nullptr || inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (dir->inode.type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  // 检查是否已存在
  if (FindInDirectory(dir, name) != nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileExists));
  }

  // 扩展目录容量
  size_t current_entries = dir->capacity / sizeof(RamDirEntry);
  if (dir->child_count >= current_entries) {
    size_t new_capacity = (current_entries == 0) ? 16 : current_entries * 2;
    RamDirEntry* new_data = new (std::nothrow) RamDirEntry[new_capacity];
    if (new_data == nullptr) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }

    // 复制旧数据
    if (dir->data != nullptr) {
      memcpy(new_data, dir->data, dir->child_count * sizeof(RamDirEntry));
      delete[] static_cast<RamDirEntry*>(dir->data);
    }

    dir->data = new_data;
    dir->capacity = new_capacity * sizeof(RamDirEntry);
  }

  // 添加新条目
  RamDirEntry* entries = static_cast<RamDirEntry*>(dir->data);
  RamDirEntry* new_entry = &entries[dir->child_count];
  strncpy(new_entry->name, name, sizeof(new_entry->name) - 1);
  new_entry->name[sizeof(new_entry->name) - 1] = '\0';
  new_entry->inode = inode;

  ++dir->child_count;
  ++inode->link_count;

  return {};
}

auto RamFs::RemoveFromDirectory(RamInode* dir, const char* name)
    -> Expected<void> {
  if (dir == nullptr || name == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamDirEntry* entry = FindInDirectory(dir, name);
  if (entry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  // 减少链接计数
  if (entry->inode != nullptr) {
    --entry->inode->link_count;
  }

  // 移除条目（将最后一个条目移到此处）
  RamDirEntry* entries = static_cast<RamDirEntry*>(dir->data);
  size_t last_idx = dir->child_count - 1;
  if (entry != &entries[last_idx]) {
    *entry = entries[last_idx];
  }

  --dir->child_count;
  return {};
}

auto RamFs::IsDirectoryEmpty(RamInode* dir) -> bool {
  if (dir == nullptr || dir->inode.type != FileType::kDirectory) {
    return true;
  }

  // 只包含 . 和 .. 的目录也是空的
  return dir->child_count == 0;
}

auto RamFs::ExpandFile(RamInode* inode, size_t new_size) -> Expected<void> {
  if (inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (new_size <= inode->capacity) {
    return {};
  }

  // 计算新容量（按 256 字节对齐）
  size_t new_capacity = ((new_size + 255) / 256) * 256;

  uint8_t* new_data = new (std::nothrow) uint8_t[new_capacity];
  if (new_data == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  // 复制旧数据
  if (inode->data != nullptr) {
    memcpy(new_data, inode->data, inode->inode.size);
    delete[] static_cast<uint8_t*>(inode->data);
  }

  inode->data = new_data;
  inode->capacity = new_capacity;

  return {};
}

// Static 成员函数实现

auto RamFs::RamLookup(Inode* dir, const char* name) -> Expected<Inode*> {
  if (dir == nullptr || name == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamInode* ram_dir = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(dir) - offsetof(RamInode, inode));

  RamFs* fs = static_cast<RamFs*>(dir->fs);
  RamDirEntry* entry = fs->FindInDirectory(ram_dir, name);

  if (entry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  return entry->inode;
}

auto RamFs::RamCreate(Inode* dir, const char* name, FileType type)
    -> Expected<Inode*> {
  if (dir == nullptr || name == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (type != FileType::kRegular && type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamFs* fs = static_cast<RamFs*>(dir->fs);
  RamInode* ram_dir = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(dir) - offsetof(RamInode, inode));

  // 分配新 inode
  auto alloc_result = fs->AllocateInode();
  if (!alloc_result.has_value()) {
    return std::unexpected(alloc_result.error());
  }

  Inode* new_inode = alloc_result.value();
  new_inode->type = type;

  // 添加到目录
  auto add_result = fs->AddToDirectory(ram_dir, name, new_inode);
  if (!add_result.has_value()) {
    fs->FreeInode(new_inode);
    return std::unexpected(add_result.error());
  }

  return new_inode;
}

auto RamFs::RamUnlink(Inode* dir, const char* name) -> Expected<void> {
  if (dir == nullptr || name == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamFs* fs = static_cast<RamFs*>(dir->fs);
  RamInode* ram_dir = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(dir) - offsetof(RamInode, inode));

  // 查找目标
  RamDirEntry* entry = fs->FindInDirectory(ram_dir, name);
  if (entry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  // 不能删除目录
  if (entry->inode != nullptr && entry->inode->type == FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsIsADirectory));
  }

  // 从目录中移除
  auto remove_result = fs->RemoveFromDirectory(ram_dir, name);
  if (!remove_result.has_value()) {
    return remove_result;
  }

  // 如果链接计数为 0，释放 inode
  if (entry->inode != nullptr && entry->inode->link_count == 0) {
    fs->FreeInode(entry->inode);
  }

  return {};
}

auto RamFs::RamMkdir(Inode* dir, const char* name) -> Expected<Inode*> {
  // 复用 RamCreate，但指定类型为目录
  return RamCreate(dir, name, FileType::kDirectory);
}

auto RamFs::RamRmdir(Inode* dir, const char* name) -> Expected<void> {
  if (dir == nullptr || name == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  RamFs* fs = static_cast<RamFs*>(dir->fs);
  RamInode* ram_dir = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(dir) - offsetof(RamInode, inode));

  // 查找目标
  RamDirEntry* entry = fs->FindInDirectory(ram_dir, name);
  if (entry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  // 必须是目录
  if (entry->inode == nullptr || entry->inode->type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  // 检查目录是否为空
  RamInode* target = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(entry->inode) - offsetof(RamInode, inode));
  if (!fs->IsDirectoryEmpty(target)) {
    return std::unexpected(Error(ErrorCode::kFsNotEmpty));
  }

  // 从目录中移除
  auto remove_result = fs->RemoveFromDirectory(ram_dir, name);
  if (!remove_result.has_value()) {
    return remove_result;
  }

  // 释放 inode
  if (entry->inode->link_count == 0) {
    fs->FreeInode(entry->inode);
  }

  return {};
}

auto RamFs::RamRead(File* file, void* buf, size_t count) -> Expected<size_t> {
  if (file == nullptr || buf == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  if (file->inode->type != FileType::kRegular) {
    return std::unexpected(Error(ErrorCode::kFsIsADirectory));
  }

  RamInode* ram_inode = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(file->inode) - offsetof(RamInode, inode));

  // 计算可读字节数
  if (file->offset >= file->inode->size) {
    return 0;  // EOF
  }

  size_t available = file->inode->size - file->offset;
  size_t to_read = (count < available) ? count : available;

  if (to_read == 0) {
    return 0;
  }

  // 复制数据
  memcpy(buf, static_cast<uint8_t*>(ram_inode->data) + file->offset, to_read);

  file->offset += to_read;
  return to_read;
}

auto RamFs::RamWrite(File* file, const void* buf, size_t count)
    -> Expected<size_t> {
  if (file == nullptr || buf == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  if (file->inode->type != FileType::kRegular) {
    return std::unexpected(Error(ErrorCode::kFsIsADirectory));
  }

  RamInode* ram_inode = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(file->inode) - offsetof(RamInode, inode));

  // 检查是否需要扩展文件
  size_t new_size = file->offset + count;
  if (new_size > ram_inode->capacity) {
    auto expand_result =
        static_cast<RamFs*>(file->inode->fs)->ExpandFile(ram_inode, new_size);
    if (!expand_result.has_value()) {
      return std::unexpected(expand_result.error());
    }
  }

  // 写入数据
  memcpy(static_cast<uint8_t*>(ram_inode->data) + file->offset, buf, count);

  file->offset += count;

  // 更新文件大小
  if (file->offset > file->inode->size) {
    file->inode->size = file->offset;
  }

  return count;
}

auto RamFs::RamSeek(File* file, int64_t offset, SeekWhence whence)
    -> Expected<uint64_t> {
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  uint64_t new_offset = 0;

  switch (whence) {
    case SeekWhence::kSet:
      if (offset < 0) {
        return std::unexpected(Error(ErrorCode::kInvalidArgument));
      }
      new_offset = static_cast<uint64_t>(offset);
      break;

    case SeekWhence::kCur:
      if (offset < 0 && static_cast<uint64_t>(-offset) > file->offset) {
        return std::unexpected(Error(ErrorCode::kInvalidArgument));
      }
      new_offset =
          static_cast<uint64_t>(static_cast<int64_t>(file->offset) + offset);
      break;

    case SeekWhence::kEnd:
      if (file->inode == nullptr) {
        return std::unexpected(Error(ErrorCode::kFsCorrupted));
      }
      if (offset < 0 && static_cast<uint64_t>(-offset) > file->inode->size) {
        return std::unexpected(Error(ErrorCode::kInvalidArgument));
      }
      new_offset = static_cast<uint64_t>(
          static_cast<int64_t>(file->inode->size) + offset);
      break;

    default:
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  file->offset = new_offset;
  return new_offset;
}

auto RamFs::RamClose(File* file) -> Expected<void> {
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // ramfs 没有特殊的关闭操作
  // File 对象由调用者释放
  return {};
}

auto RamFs::RamReadDir(File* file, DirEntry* dirent, size_t count)
    -> Expected<size_t> {
  if (file == nullptr || dirent == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->inode == nullptr || file->inode->type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  RamInode* ram_dir = reinterpret_cast<RamInode*>(
      reinterpret_cast<uint8_t*>(file->inode) - offsetof(RamInode, inode));

  RamDirEntry* entries = static_cast<RamDirEntry*>(ram_dir->data);

  size_t read_count = 0;
  size_t offset = file->offset;

  // 添加 . 和 .. 条目
  if (offset == 0 && read_count < count) {
    dirent[read_count].ino = file->inode->ino;
    dirent[read_count].type = static_cast<uint8_t>(FileType::kDirectory);
    strncpy(dirent[read_count].name, ".", sizeof(dirent[read_count].name));
    ++read_count;
    ++offset;
  }

  if (offset == 1 && read_count < count) {
    Inode* parent_inode =
        (file->dentry != nullptr && file->dentry->parent != nullptr)
            ? file->dentry->parent->inode
            : file->inode;
    dirent[read_count].ino = parent_inode->ino;
    dirent[read_count].type = static_cast<uint8_t>(FileType::kDirectory);
    strncpy(dirent[read_count].name, "..", sizeof(dirent[read_count].name));
    ++read_count;
    ++offset;
  }

  // 读取实际目录项（跳过 . 和 .. 的偏移）
  size_t entry_idx = (offset > 2) ? offset - 2 : 0;
  while (read_count < count && entry_idx < ram_dir->child_count) {
    dirent[read_count].ino = entries[entry_idx].inode->ino;
    dirent[read_count].type =
        static_cast<uint8_t>(entries[entry_idx].inode->type);
    strncpy(dirent[read_count].name, entries[entry_idx].name,
            sizeof(dirent[read_count].name));
    ++read_count;
    ++entry_idx;
  }

  file->offset = offset + read_count;
  return read_count;
}

}  // namespace ramfs
