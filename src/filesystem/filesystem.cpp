/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "block_device_provider.hpp"
#include "fatfs.hpp"
#include "kernel_log.hpp"
#include "mount.hpp"
#include "ramfs.hpp"
#include "vfs.hpp"

/// @brief FatFS 逻辑驱动器号（对应 rootfs.img）
static constexpr uint8_t kRootFsDriveId = 0;

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

  // Mount FatFS on virtio-blk0 at /mnt/fat
  auto* blk = GetVirtioBlkBlockDevice();
  if (blk != nullptr) {
    static fatfs::FatFsFileSystem fat_fs(kRootFsDriveId);
    auto fat_mount = fat_fs.Mount(blk);
    if (!fat_mount.has_value()) {
      klog::Err("FileSystemInit: FatFsFileSystem::Mount failed: %s\n",
                fat_mount.error().message());
    } else {
      auto vfs_mount = vfs::GetMountTable().Mount("/mnt/fat", &fat_fs, blk);
      if (!vfs_mount.has_value()) {
        klog::Err("FileSystemInit: vfs mount at /mnt/fat failed: %s\n",
                  vfs_mount.error().message());
      } else {
        klog::Info("FileSystemInit: FatFS mounted at /mnt/fat\n");
      }
    }
  }

  klog::Info("FileSystemInit: complete\n");
}
