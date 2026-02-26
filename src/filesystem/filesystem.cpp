/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kernel_log.hpp"
#include "mount.hpp"
#include "ramfs.hpp"
#include "vfs.hpp"

/// @brief 文件系统子系统初始化入口
auto FileSystemInit() -> void {
  // 初始化 VFS
  auto init_result = vfs::Init();
  if (!init_result.has_value()) {
    klog::Err("FileSystemInit: vfs::Init failed: %s\n",
              init_result.error().message());
    return;
  }

  // 创建 ramfs 根文件系统并挂载到 "/"
  static ramfs::RamFs root_ramfs;
  auto mount_result = vfs::GetMountTable().Mount("/", &root_ramfs, nullptr);
  if (!mount_result.has_value()) {
    klog::Err("FileSystemInit: failed to mount ramfs at /: %s\n",
              mount_result.error().message());
    return;
  }

  klog::Info("FileSystemInit: complete\n");
}
