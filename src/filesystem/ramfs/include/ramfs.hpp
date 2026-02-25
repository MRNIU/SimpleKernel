/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ramfs - 内存文件系统
 */

#ifndef SIMPLEKERNEL_SRC_FILESYSTEM_RAMFS_INCLUDE_RAMFS_HPP_
#define SIMPLEKERNEL_SRC_FILESYSTEM_RAMFS_INCLUDE_RAMFS_HPP_

#include "filesystem.hpp"
#include "vfs.hpp"

namespace ramfs {

/**
 * @brief ramfs 文件系统实现
 * @details 纯内存文件系统，所有数据存储在内存中，适合用作 rootfs
 */
class RamFs : public vfs::FileSystem {
 public:
  /**
   * @brief 构造函数
   */
  RamFs();

  /**
   * @brief 析构函数
   */
  ~RamFs() override;

  // 禁止拷贝和移动
  RamFs(const RamFs&) = delete;
  RamFs(RamFs&&) = delete;
  auto operator=(const RamFs&) -> RamFs& = delete;
  auto operator=(RamFs&&) -> RamFs& = delete;

  /**
   * @brief 获取文件系统类型名
   * @return "ramfs"
   */
  [[nodiscard]] auto GetName() const -> const char* override;

  /**
   * @brief 挂载 ramfs
   * @param device 必须为 nullptr（ramfs 不需要块设备）
   * @return Expected<vfs::Inode*> 根目录 inode
   * @post 返回的 inode->type == vfs::FileType::kDirectory
   */
  auto Mount(vfs::BlockDevice* device) -> Expected<vfs::Inode*> override;

  /**
   * @brief 卸载 ramfs
   * @return Expected<void> 成功或错误
   * @pre 没有打开的文件引用此文件系统
   */
  auto Unmount() -> Expected<void> override;

  /**
   * @brief 同步数据到磁盘（ramfs 无操作）
   * @return Expected<void> 成功
   */
  auto Sync() -> Expected<void> override;

  /**
   * @brief 分配新 inode
   * @return Expected<vfs::Inode*> 新分配的 inode 或错误
   */
  auto AllocateInode() -> Expected<vfs::Inode*> override;

  /**
   * @brief 释放 inode
   * @param inode 要释放的 inode
   * @return Expected<void> 成功或错误
   */
  auto FreeInode(vfs::Inode* inode) -> Expected<void> override;

  /**
   * @brief 获取根 inode
   * @return vfs::Inode* 根目录 inode
   */
  [[nodiscard]] auto GetRootInode() const -> vfs::Inode*;

  /**
   * @brief 获取 ramfs 的文件操作表
   * @return vfs::FileOps* 文件操作表指针
   */
  static auto GetFileOps() -> vfs::FileOps*;

  // inode 操作函数（static，供全局 ops 表使用）
  static auto RamLookup(vfs::Inode* dir, const char* name)
      -> Expected<vfs::Inode*>;
  static auto RamCreate(vfs::Inode* dir, const char* name, vfs::FileType type)
      -> Expected<vfs::Inode*>;
  static auto RamUnlink(vfs::Inode* dir, const char* name) -> Expected<void>;
  static auto RamMkdir(vfs::Inode* dir, const char* name)
      -> Expected<vfs::Inode*>;
  static auto RamRmdir(vfs::Inode* dir, const char* name) -> Expected<void>;

  // file 操作函数（static，供全局 ops 表使用）
  static auto RamRead(vfs::File* file, void* buf, size_t count)
      -> Expected<size_t>;
  static auto RamWrite(vfs::File* file, const void* buf, size_t count)
      -> Expected<size_t>;
  static auto RamSeek(vfs::File* file, int64_t offset, vfs::SeekWhence whence)
      -> Expected<uint64_t>;
  static auto RamClose(vfs::File* file) -> Expected<void>;
  static auto RamReadDir(vfs::File* file, vfs::DirEntry* dirent, size_t count)
      -> Expected<size_t>;

 private:
  /// @brief ramfs 内部 inode 数据
  struct RamInode {
    vfs::Inode inode;
    /// 文件数据（普通文件）或子项列表（目录）
    void* data;
    /// 数据缓冲区容量
    size_t capacity;
    /// 子项数量（仅目录）
    size_t child_count;
    /// 空闲链表指针
    RamInode* next_free;
  };

  /// @brief 目录项（存储在目录的 data 中）
  struct RamDirEntry {
    char name[256];
    vfs::Inode* inode;
  };

  static constexpr size_t kMaxInodes = 1024;
  /// 初始文件容量
  static constexpr size_t kInitialCapacity = 256;

  RamInode inodes_[kMaxInodes];
  /// 空闲 inode 链表头
  RamInode* free_list_;
  /// 根目录 inode
  vfs::Inode* root_inode_;
  /// 已使用的 inode 数量
  size_t used_inodes_;
  /// 是否已挂载
  bool mounted_;

  // 辅助函数
  auto FindInDirectory(RamInode* dir, const char* name) -> RamDirEntry*;
  auto AddToDirectory(RamInode* dir, const char* name, vfs::Inode* inode)
      -> Expected<void>;
  auto RemoveFromDirectory(RamInode* dir, const char* name) -> Expected<void>;
  auto IsDirectoryEmpty(RamInode* dir) -> bool;
  auto ExpandFile(RamInode* inode, size_t new_size) -> Expected<void>;
};

}  // namespace ramfs

#endif  // SIMPLEKERNEL_SRC_FILESYSTEM_RAMFS_INCLUDE_RAMFS_HPP_
