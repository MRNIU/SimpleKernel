/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief FatFS VFS adapter — wraps FatFS as a vfs::FileSystem
 */

#ifndef SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FATFS_FILESYSTEM_HPP_
#define SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FATFS_FILESYSTEM_HPP_

#include <cstdint>

#include "block_device.hpp"
#include "ff.h"
#include "filesystem.hpp"
#include "vfs.hpp"

namespace fatfs {

/**
 * @brief FatFS VFS adapter
 * @details Wraps FatFS (f_mount / f_open / f_read / ...) behind the
 *          vfs::FileSystem interface.  One FatFsFileSystem instance owns
 *          exactly one FatFS logical drive (volume).
 *
 * @pre FF_VOLUMES >= 1 (ffconf.h)
 */
class FatFsFileSystem : public vfs::FileSystem {
 public:
  /**
   * @brief Construct a FatFsFileSystem bound to the given FatFS volume ID.
   * @param volume_id  FatFS logical drive number (0 .. FF_VOLUMES-1)
   * @pre volume_id < FF_VOLUMES
   */
  explicit FatFsFileSystem(uint8_t volume_id);

  ~FatFsFileSystem() override;

  FatFsFileSystem(const FatFsFileSystem&) = delete;
  FatFsFileSystem(FatFsFileSystem&&) = delete;
  auto operator=(const FatFsFileSystem&) -> FatFsFileSystem& = delete;
  auto operator=(FatFsFileSystem&&) -> FatFsFileSystem& = delete;

  /**
   * @brief Returns "fatfs"
   */
  [[nodiscard]] auto GetName() const -> const char* override;

  /**
   * @brief Mount the FatFS volume.
   * @param device  Block device providing storage; must not be nullptr.
   * @return Expected<vfs::Inode*> Root directory inode or error.
   * @pre device != nullptr
   * @post Returned inode->type == vfs::FileType::kDirectory
   */
  auto Mount(vfs::BlockDevice* device) -> Expected<vfs::Inode*> override;

  /**
   * @brief Unmount the FatFS volume.
   * @return Expected<void> Success or error.
   * @pre No open file objects referencing this volume.
   */
  auto Unmount() -> Expected<void> override;

  /**
   * @brief Flush all dirty buffers.
   * @return Expected<void> Success or error.
   */
  auto Sync() -> Expected<void> override;

  /**
   * @brief Allocate a new inode (backed by a FatFS FILINFO snapshot).
   * @return Expected<vfs::Inode*> New inode or error.
   */
  auto AllocateInode() -> Expected<vfs::Inode*> override;

  /**
   * @brief Free an inode.
   * @param inode  Inode to free; must not be nullptr.
   * @return Expected<void> Success or error.
   * @pre inode != nullptr
   * @pre inode->link_count == 0
   */
  auto FreeInode(vfs::Inode* inode) -> Expected<void> override;

  /**
   * @brief Returns the FileOps instance for this filesystem.
   */
  auto GetFileOps() -> vfs::FileOps* override;

  /**
   * @brief Open the underlying FatFS FIL object for an inode.
   * @param inode      Inode to open (must not be nullptr, must be kRegular).
   * @param open_flags vfs OpenFlags bitmask.
   * @return Expected<void> Success or error.
   * @pre inode != nullptr && inode->type == vfs::FileType::kRegular
   * @post inode->fs_private->fil != nullptr on success
   */
  auto OpenFil(vfs::Inode* inode, uint32_t open_flags) -> Expected<void>;

  // ── Inner op classes ────────────────────────────────────────────────────

  class FatFsInodeOps : public vfs::InodeOps {
   public:
    explicit FatFsInodeOps(FatFsFileSystem* fs) : fs_(fs) {}
    auto Lookup(vfs::Inode* dir, const char* name)
        -> Expected<vfs::Inode*> override;
    auto Create(vfs::Inode* dir, const char* name, vfs::FileType type)
        -> Expected<vfs::Inode*> override;
    auto Unlink(vfs::Inode* dir, const char* name) -> Expected<void> override;
    auto Mkdir(vfs::Inode* dir, const char* name)
        -> Expected<vfs::Inode*> override;
    auto Rmdir(vfs::Inode* dir, const char* name) -> Expected<void> override;

   private:
    FatFsFileSystem* fs_;
  };

  class FatFsFileOps : public vfs::FileOps {
   public:
    explicit FatFsFileOps(FatFsFileSystem* fs) : fs_(fs) {}
    auto Read(vfs::File* file, void* buf, size_t count)
        -> Expected<size_t> override;
    auto Write(vfs::File* file, const void* buf, size_t count)
        -> Expected<size_t> override;
    auto Seek(vfs::File* file, int64_t offset, vfs::SeekWhence whence)
        -> Expected<uint64_t> override;
    auto Close(vfs::File* file) -> Expected<void> override;
    auto ReadDir(vfs::File* file, vfs::DirEntry* dirent, size_t count)
        -> Expected<size_t> override;

   private:
    FatFsFileSystem* fs_;
  };

  friend class FatFsInodeOps;
  friend class FatFsFileOps;

 private:
  /// FatFS logical drive number
  uint8_t volume_id_;
  /// FatFS filesystem object (one per volume)
  FATFS fatfs_obj_;
  /// Root inode (set during Mount)
  vfs::Inode* root_inode_ = nullptr;
  /// Whether the volume is currently mounted
  bool mounted_ = false;

  /// Inode pool — fixed-size, no heap
  static constexpr size_t kMaxInodes = 256;

  struct FatInode {
    vfs::Inode inode;
    /// Absolute path within the volume (for FatFS operations)
    char path[512];
    /// FIL object (for open regular files); nullptr if directory or unused
    FIL* fil = nullptr;
    /// Whether this slot is in use
    bool in_use = false;
  };

  FatInode inodes_[kMaxInodes];

  /// FIL pool — kMaxOpenFiles simultaneously-open file objects
  static constexpr size_t kMaxOpenFiles = 16;

  struct FatFileHandle {
    FIL fil;
    bool in_use = false;
  };

  FatFileHandle fil_pool_[kMaxOpenFiles];

  // Op singletons
  FatFsInodeOps inode_ops_;
  FatFsFileOps file_ops_;

  // Helpers
  auto AllocateFatInode() -> FatInode*;
  auto FreeFatInode(FatInode* fi) -> void;
  auto AllocateFil() -> FIL*;
  auto FreeFil(FIL* fil) -> void;
};

}  // namespace fatfs

#endif /* SIMPLEKERNEL_SRC_FILESYSTEM_FATFS_INCLUDE_FATFS_FILESYSTEM_HPP_ */
