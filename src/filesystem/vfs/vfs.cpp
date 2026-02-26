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
#include "vfs_internal.hpp"

namespace vfs {

auto GetVfsState() -> VfsState& {
  static VfsState state;
  return state;
}

auto SkipLeadingSlashes(const char* path) -> const char* {
  while (*path == '/') {
    ++path;
  }
  return path;
}

auto CopyPathComponent(const char* src, char* dst, size_t dst_size) -> size_t {
  size_t i = 0;
  while (*src != '\0' && *src != '/' && i < dst_size - 1) {
    dst[i++] = *src++;
  }
  dst[i] = '\0';
  return i;
}

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

auto AddChild(Dentry* parent, Dentry* child) -> void {
  if (parent == nullptr || child == nullptr) {
    return;
  }

  child->parent = parent;
  child->next_sibling = parent->children;
  parent->children = child;
}

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

  // 初始化挂载表（使用全局单例，与 GetMountTable() 统一）
  GetVfsState().mount_table = &GetMountTable();

  GetVfsState().initialized = true;
  klog::Info("VFS: initialization complete\n");
  return {};
}

auto GetRootDentry() -> Dentry* { return GetVfsState().root_dentry; }

// 内部接口：设置根 dentry
void SetRootDentry(Dentry* dentry) { GetVfsState().root_dentry = dentry; }

// 内部接口：获取挂载表
auto GetMountTableInternal() -> MountTable* {
  return GetVfsState().mount_table;
}

}  // namespace vfs
