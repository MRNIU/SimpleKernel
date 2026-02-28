/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_FILESYSTEM_VFS_VFS_INTERNAL_HPP_
#define SIMPLEKERNEL_SRC_FILESYSTEM_VFS_VFS_INTERNAL_HPP_

#include "mount.hpp"
#include "spinlock.hpp"
#include "vfs.hpp"

namespace vfs {

// VFS 全局状态结构体
struct VfsState {
  bool initialized = false;
  MountTable* mount_table = nullptr;
  Dentry* root_dentry = nullptr;
  SpinLock vfs_lock_{"vfs"};
};

// 获取 VFS 状态
auto GetVfsState() -> VfsState&;

/**
 * @brief 跳过路径中的前导斜杠
 * @param path 输入路径
 * @return 跳过前导斜杠后的路径指针
 */
auto SkipLeadingSlashes(const char* path) -> const char*;

/**
 * @brief 复制路径组件到缓冲区
 * @param src 源路径（当前位置）
 * @param dst 目标缓冲区
 * @param dst_size 缓冲区大小
 * @return 复制的长度
 */
auto CopyPathComponent(const char* src, char* dst, size_t dst_size) -> size_t;

/**
 * @brief 在 dentry 的子节点中查找指定名称
 */
auto FindChild(Dentry* parent, const char* name) -> Dentry*;

/**
 * @brief 添加子 dentry
 */
auto AddChild(Dentry* parent, Dentry* child) -> void;

/**
 * @brief 从父 dentry 中移除子 dentry
 */
auto RemoveChild(Dentry* parent, Dentry* child) -> void;

}  // namespace vfs

#endif  // SIMPLEKERNEL_SRC_FILESYSTEM_VFS_VFS_INTERNAL_HPP_
