/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "filesystem.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "spinlock.hpp"
#include "vfs_internal.hpp"

namespace vfs {

auto Open(const char* path, uint32_t flags) -> Expected<File*> {
  if (!GetVfsState().initialized) {
    return std::unexpected(Error(ErrorCode::kFsNotMounted));
  }

  if (path == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  LockGuard<SpinLock> guard(GetVfsState().vfs_lock_);
  // 查找或创建 dentry
  auto lookup_result = Lookup(path);
  Dentry* dentry = nullptr;

  if (!lookup_result.has_value()) {
    // 文件不存在，检查是否需要创建
    if ((flags & kOCreate) == 0) {
      return std::unexpected(lookup_result.error());
    }

    // 创建新文件
    // 找到父目录路径
    char parent_path[512];
    char file_name[256];
    const char* last_slash = strrchr(path, '/');
    if (last_slash == nullptr || last_slash == path) {
      strncpy(parent_path, "/", sizeof(parent_path));
      strncpy(file_name, path[0] == '/' ? path + 1 : path, sizeof(file_name));
    } else {
      size_t parent_len = last_slash - path;
      if (parent_len >= sizeof(parent_path)) {
        parent_len = sizeof(parent_path) - 1;
      }
      strncpy(parent_path, path, parent_len);
      parent_path[parent_len] = '\0';
      strncpy(file_name, last_slash + 1, sizeof(file_name));
    }
    file_name[sizeof(file_name) - 1] = '\0';

    // 查找父目录
    auto parent_result = Lookup(parent_path);
    if (!parent_result.has_value()) {
      return std::unexpected(parent_result.error());
    }

    Dentry* parent_dentry = parent_result.value();
    if (parent_dentry->inode == nullptr ||
        parent_dentry->inode->type != FileType::kDirectory) {
      return std::unexpected(Error(ErrorCode::kFsNotADirectory));
    }

    // 创建文件
    if (parent_dentry->inode->ops == nullptr) {
      return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
    }

    auto create_result = parent_dentry->inode->ops->Create(
        parent_dentry->inode, file_name, FileType::kRegular);
    if (!create_result.has_value()) {
      return std::unexpected(create_result.error());
    }

    // 创建 dentry
    dentry = new Dentry();
    if (dentry == nullptr) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }

    strncpy(dentry->name, file_name, sizeof(dentry->name) - 1);
    dentry->name[sizeof(dentry->name) - 1] = '\0';
    dentry->inode = create_result.value();
    AddChild(parent_dentry, dentry);
  } else {
    dentry = lookup_result.value();
  }

  if (dentry == nullptr || dentry->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  // 检查打开模式
  if ((flags & kODirectory) != 0 &&
      dentry->inode->type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  // 创建 File 对象
  File* file = new File();
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  file->inode = dentry->inode;
  file->dentry = dentry;
  file->offset = 0;
  file->flags = flags;

  // 从文件系统获取 FileOps
  if (file->inode != nullptr && file->inode->fs != nullptr) {
    file->ops = file->inode->fs->GetFileOps();
  }

  // 处理 O_TRUNC
  if ((flags & kOTruncate) != 0 && dentry->inode->type == FileType::kRegular) {
    // 截断文件，由具体文件系统处理
    // 这里不直接操作，而是通过后续的 write 来处理
  }

  klog::Debug("VFS: opened '%s', flags=0x%x\n", path, flags);
  return file;
}

}  // namespace vfs
