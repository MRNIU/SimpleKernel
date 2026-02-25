/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "vfs.hpp"

#include "filesystem.hpp"
#include "kernel_log.hpp"
#include "mount.hpp"
#include "singleton.hpp"
#include "sk_cstdio"
#include "sk_cstring"

namespace vfs {

namespace {

// VFS 全局状态结构体
struct VfsState {
  bool initialized = false;
  MountTable* mount_table = nullptr;
  Dentry* root_dentry = nullptr;
};

// 使用 Singleton 管理 VFS 状态
auto GetVfsState() -> VfsState& {
  static VfsState state;
  return state;
}

/**
 * @brief 跳过路径中的前导斜杠
 * @param path 输入路径
 * @return 跳过前导斜杠后的路径指针
 */
auto SkipLeadingSlashes(const char* path) -> const char* {
  while (*path == '/') {
    ++path;
  }
  return path;
}

/**
 * @brief 复制路径组件到缓冲区
 * @param src 源路径（当前位置）
 * @param dst 目标缓冲区
 * @param dst_size 缓冲区大小
 * @return 复制的长度
 */
auto CopyPathComponent(const char* src, char* dst, size_t dst_size) -> size_t {
  size_t i = 0;
  while (*src != '\0' && *src != '/' && i < dst_size - 1) {
    dst[i++] = *src++;
  }
  dst[i] = '\0';
  return i;
}

/**
 * @brief 在 dentry 的子节点中查找指定名称
 */
auto FindChild(Dentry* parent, const char* name) -> Dentry* {
  if (parent == nullptr || name == nullptr) {
    return nullptr;
  }

  Dentry* child = parent->children;
  while (child != nullptr) {
    if (strcmp(child->name, name) == 0) {
      return child;
    }
    child = child->next_sibling;
  }
  return nullptr;
}

/**
 * @brief 添加子 dentry
 */
auto AddChild(Dentry* parent, Dentry* child) -> void {
  if (parent == nullptr || child == nullptr) {
    return;
  }

  child->parent = parent;
  child->next_sibling = parent->children;
  parent->children = child;
}

/**
 * @brief 从父 dentry 中移除子 dentry
 */
auto RemoveChild(Dentry* parent, Dentry* child) -> void {
  if (parent == nullptr || child == nullptr) {
    return;
  }

  Dentry** current = &parent->children;
  while (*current != nullptr) {
    if (*current == child) {
      *current = child->next_sibling;
      child->parent = nullptr;
      child->next_sibling = nullptr;
      return;
    }
    current = &(*current)->next_sibling;
  }
}

}  // anonymous namespace

// Inode 构造函数实现
Inode::Inode()
    : ino(0),
      type(FileType::kUnknown),
      size(0),
      permissions(0644),
      link_count(1),
      fs_private(nullptr),
      fs(nullptr),
      ops(nullptr) {}

// Dentry 构造函数实现
Dentry::Dentry()
    : inode(nullptr),
      parent(nullptr),
      children(nullptr),
      next_sibling(nullptr),
      fs_private(nullptr) {
  name[0] = '\0';
}

// File 构造函数实现
File::File()
    : inode(nullptr), dentry(nullptr), offset(0), flags(0), ops(nullptr) {}

auto Init() -> Expected<void> {
  if (GetVfsState().initialized) {
    return {};
  }

  klog::Info("VFS: initializing...\n");

  // 初始化挂载表
  GetVfsState().mount_table = new (std::nothrow) MountTable();
  if (GetVfsState().mount_table == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  GetVfsState().initialized = true;
  klog::Info("VFS: initialization complete\n");
  return {};
}

auto Lookup(const char* path) -> Expected<Dentry*> {
  if (!GetVfsState().initialized) {
    return std::unexpected(Error(ErrorCode::kFsNotMounted));
  }

  if (path == nullptr || path[0] != '/') {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // 空路径或根目录
  if (path[0] == '/' && (path[1] == '\0' || path[1] == '/')) {
    if (GetVfsState().root_dentry == nullptr) {
      return std::unexpected(Error(ErrorCode::kFsNotMounted));
    }
    return GetVfsState().root_dentry;
  }

  // 查找路径对应的挂载点
  MountPoint* mp = GetVfsState().mount_table->Lookup(path);
  if (mp == nullptr || mp->root_dentry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsNotMounted));
  }

  // 从挂载点根开始解析路径
  Dentry* current = mp->root_dentry;
  const char* p = SkipLeadingSlashes(path);

  // 如果挂载路径不是根，需要跳过挂载路径部分
  if (mp->mount_path != nullptr && strcmp(mp->mount_path, "/") != 0) {
    const char* mount_path = SkipLeadingSlashes(mp->mount_path);
    while (*mount_path != '\0') {
      char component[256];
      size_t len = CopyPathComponent(mount_path, component, sizeof(component));
      if (len == 0) {
        break;
      }
      mount_path += len;
      mount_path = SkipLeadingSlashes(mount_path);

      // 跳过挂载路径的组件
      len = CopyPathComponent(p, component, sizeof(component));
      if (len == 0) {
        break;
      }
      p += len;
      p = SkipLeadingSlashes(p);
    }
  }

  // 逐级解析路径组件
  char component[256];
  while (*p != '\0') {
    // 检查当前 dentry 是否是目录
    if (current->inode == nullptr ||
        current->inode->type != FileType::kDirectory) {
      return std::unexpected(Error(ErrorCode::kFsNotADirectory));
    }

    // 提取路径组件
    size_t len = CopyPathComponent(p, component, sizeof(component));
    if (len == 0) {
      break;
    }
    p += len;
    p = SkipLeadingSlashes(p);

    // 处理 "." 和 ".."
    if (strcmp(component, ".") == 0) {
      continue;
    }
    if (strcmp(component, "..") == 0) {
      if (current->parent != nullptr) {
        current = current->parent;
      }
      continue;
    }

    // 在子节点中查找
    Dentry* child = FindChild(current, component);
    if (child == nullptr) {
      // 尝试通过 inode ops 查找
      if (current->inode->ops != nullptr &&
          current->inode->ops->lookup != nullptr) {
        auto result = current->inode->ops->lookup(current->inode, component);
        if (!result.has_value()) {
          return std::unexpected(Error(ErrorCode::kFsFileNotFound));
        }

        // 创建新的 dentry
        child = new (std::nothrow) Dentry();
        if (child == nullptr) {
          return std::unexpected(Error(ErrorCode::kOutOfMemory));
        }

        strncpy(child->name, component, sizeof(child->name) - 1);
        child->name[sizeof(child->name) - 1] = '\0';
        child->inode = result.value();
        AddChild(current, child);
      } else {
        return std::unexpected(Error(ErrorCode::kFsFileNotFound));
      }
    }

    current = child;

    // 检查是否遇到挂载点
    if (current->inode != nullptr) {
      // 检查是否有文件系统挂载在此 dentry 上
      for (size_t i = 0; i < MountTable::kMaxMounts; ++i) {
        MountPoint* next_mp = GetVfsState().mount_table->Lookup(p);
        if (next_mp != nullptr && next_mp != mp &&
            next_mp->mount_dentry == current) {
          mp = next_mp;
          current = next_mp->root_dentry;
          break;
        }
      }
    }
  }

  return current;
}

auto Open(const char* path, uint32_t flags) -> Expected<File*> {
  if (!GetVfsState().initialized) {
    return std::unexpected(Error(ErrorCode::kFsNotMounted));
  }

  if (path == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

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
    if (parent_dentry->inode->ops == nullptr ||
        parent_dentry->inode->ops->create == nullptr) {
      return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
    }

    auto create_result = parent_dentry->inode->ops->create(
        parent_dentry->inode, file_name, FileType::kRegular);
    if (!create_result.has_value()) {
      return std::unexpected(create_result.error());
    }

    // 创建 dentry
    dentry = new (std::nothrow) Dentry();
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
  File* file = new (std::nothrow) File();
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  file->inode = dentry->inode;
  file->dentry = dentry;
  file->offset = 0;
  file->flags = flags;

  // 设置文件操作
  // 由具体文件系统设置 FileOps
  // 这里只是创建 File 结构，实际的 ops 由文件系统填充

  // 处理 O_TRUNC
  if ((flags & kOTruncate) != 0 && dentry->inode->type == FileType::kRegular) {
    // 截断文件，由具体文件系统处理
    // 这里不直接操作，而是通过后续的 write 来处理
  }

  klog::Debug("VFS: opened '%s', flags=0x%x\n", path, flags);
  return file;
}

auto Close(File* file) -> Expected<void> {
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->ops != nullptr && file->ops->close != nullptr) {
    auto result = file->ops->close(file);
    if (!result.has_value()) {
      return result;
    }
  }

  delete file;
  return {};
}

auto Read(File* file, void* buf, size_t count) -> Expected<size_t> {
  if (file == nullptr || buf == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->ops == nullptr || file->ops->read == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  return file->ops->read(file, buf, count);
}

auto Write(File* file, const void* buf, size_t count) -> Expected<size_t> {
  if (file == nullptr || buf == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // 检查写入权限
  if ((file->flags & kOWriteOnly) == 0 && (file->flags & kOReadWrite) == 0) {
    return std::unexpected(Error(ErrorCode::kFsPermissionDenied));
  }

  if (file->ops == nullptr || file->ops->write == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  return file->ops->write(file, buf, count);
}

auto Seek(File* file, int64_t offset, SeekWhence whence) -> Expected<uint64_t> {
  if (file == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->ops != nullptr && file->ops->seek != nullptr) {
    return file->ops->seek(file, offset, whence);
  }

  // 默认实现
  uint64_t new_offset = file->offset;

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

auto MkDir(const char* path) -> Expected<void> {
  if (path == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // 解析父目录路径和目录名
  char parent_path[512];
  char dir_name[256];
  const char* last_slash = strrchr(path, '/');
  if (last_slash == nullptr || last_slash == path) {
    strncpy(parent_path, "/", sizeof(parent_path));
    strncpy(dir_name, path[0] == '/' ? path + 1 : path, sizeof(dir_name));
  } else {
    size_t parent_len = last_slash - path;
    if (parent_len >= sizeof(parent_path)) {
      parent_len = sizeof(parent_path) - 1;
    }
    strncpy(parent_path, path, parent_len);
    parent_path[parent_len] = '\0';
    strncpy(dir_name, last_slash + 1, sizeof(dir_name));
  }
  dir_name[sizeof(dir_name) - 1] = '\0';

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

  // 检查目录是否已存在
  if (FindChild(parent_dentry, dir_name) != nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileExists));
  }

  // 创建目录
  if (parent_dentry->inode->ops == nullptr ||
      parent_dentry->inode->ops->mkdir == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  auto result =
      parent_dentry->inode->ops->mkdir(parent_dentry->inode, dir_name);
  if (!result.has_value()) {
    return std::unexpected(result.error());
  }

  // 创建 dentry
  Dentry* new_dentry = new (std::nothrow) Dentry();
  if (new_dentry == nullptr) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }

  strncpy(new_dentry->name, dir_name, sizeof(new_dentry->name) - 1);
  new_dentry->inode = result.value();
  AddChild(parent_dentry, new_dentry);

  klog::Debug("VFS: created directory '%s'\n", path);
  return {};
}

auto RmDir(const char* path) -> Expected<void> {
  if (path == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // 解析父目录路径和目录名
  char parent_path[512];
  char dir_name[256];
  const char* last_slash = strrchr(path, '/');
  if (last_slash == nullptr || last_slash == path) {
    strncpy(parent_path, "/", sizeof(parent_path));
    strncpy(dir_name, path[0] == '/' ? path + 1 : path, sizeof(dir_name));
  } else {
    size_t parent_len = last_slash - path;
    if (parent_len >= sizeof(parent_path)) {
      parent_len = sizeof(parent_path) - 1;
    }
    strncpy(parent_path, path, parent_len);
    parent_path[parent_len] = '\0';
    strncpy(dir_name, last_slash + 1, sizeof(dir_name));
  }
  dir_name[sizeof(dir_name) - 1] = '\0';

  // 查找父目录
  auto parent_result = Lookup(parent_path);
  if (!parent_result.has_value()) {
    return std::unexpected(parent_result.error());
  }

  Dentry* parent_dentry = parent_result.value();
  if (parent_dentry->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  // 查找目标目录
  Dentry* target_dentry = FindChild(parent_dentry, dir_name);
  if (target_dentry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  if (target_dentry->inode == nullptr ||
      target_dentry->inode->type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  // 检查目录是否为空
  if (target_dentry->children != nullptr) {
    return std::unexpected(Error(ErrorCode::kFsNotEmpty));
  }

  // 删除目录
  if (parent_dentry->inode->ops == nullptr ||
      parent_dentry->inode->ops->rmdir == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  auto result =
      parent_dentry->inode->ops->rmdir(parent_dentry->inode, dir_name);
  if (!result.has_value()) {
    return std::unexpected(result.error());
  }

  // 从父目录中移除 dentry
  RemoveChild(parent_dentry, target_dentry);
  delete target_dentry;

  klog::Debug("VFS: removed directory '%s'\n", path);
  return {};
}

auto Unlink(const char* path) -> Expected<void> {
  if (path == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  // 解析父目录路径和文件名
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
  if (parent_dentry->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  // 查找目标文件
  Dentry* target_dentry = FindChild(parent_dentry, file_name);
  if (target_dentry == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsFileNotFound));
  }

  if (target_dentry->inode == nullptr) {
    return std::unexpected(Error(ErrorCode::kFsCorrupted));
  }

  // 不能删除目录
  if (target_dentry->inode->type == FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsIsADirectory));
  }

  // 删除文件
  if (parent_dentry->inode->ops == nullptr ||
      parent_dentry->inode->ops->unlink == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  auto result =
      parent_dentry->inode->ops->unlink(parent_dentry->inode, file_name);
  if (!result.has_value()) {
    return std::unexpected(result.error());
  }

  // 从父目录中移除 dentry
  RemoveChild(parent_dentry, target_dentry);
  delete target_dentry;

  klog::Debug("VFS: unlinked '%s'\n", path);
  return {};
}

auto ReadDir(File* file, DirEntry* dirent, size_t count) -> Expected<size_t> {
  if (file == nullptr || dirent == nullptr) {
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }

  if (file->inode == nullptr || file->inode->type != FileType::kDirectory) {
    return std::unexpected(Error(ErrorCode::kFsNotADirectory));
  }

  if (file->ops == nullptr || file->ops->readdir == nullptr) {
    return std::unexpected(Error(ErrorCode::kDeviceNotSupported));
  }

  return file->ops->readdir(file, dirent, count);
}

auto GetRootDentry() -> Dentry* { return GetVfsState().root_dentry; }

// 内部接口：设置根 dentry
void SetRootDentry(Dentry* dentry) { GetVfsState().root_dentry = dentry; }

// 内部接口：获取挂载表
auto GetMountTableInternal() -> MountTable* {
  return GetVfsState().mount_table;
}

}  // namespace vfs
