/**
 * @copyright Copyright The SimpleKernel Contributors
 * @file    fatfs_filesystem.cpp
 * @brief   FatFsFileSystem implementation — vfs::FileSystem adapter for FatFS
 */

#include "fatfs_filesystem.hpp"

#include "ff.h"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "sk_stdio.h"
#include "vfs.hpp"

// Defined in diskio.cpp; set by Mount() before calling f_mount().
extern vfs::BlockDevice* g_block_devices[FF_VOLUMES];

namespace fatfs {

// ── Helpers
// ───────────────────────────────────────────────────────────────────

namespace {

/// Map FatFS FRESULT to Expected<void>.
auto FresultToExpected(FRESULT fr) -> Expected<void> {
  if (fr == FR_OK) {
    return {};
  }
  return std::unexpected(Error{ErrorCode::kFsCorrupted});
}

/// Convert FILINFO attribute to vfs::FileType.
auto FilInfoToFileType(const FILINFO& fi) -> vfs::FileType {
  if ((fi.fattrib & AM_DIR) != 0) {
    return vfs::FileType::kDirectory;
  }
  return vfs::FileType::kRegular;
}

}  // namespace

// ── Constructor / Destructor
// ──────────────────────────────────────────────────

FatFsFileSystem::FatFsFileSystem(uint8_t volume_id)
    : volume_id_(volume_id),
      fatfs_obj_{},
      inodes_{},
      fil_pool_{},
      inode_ops_(this),
      file_ops_(this) {}

FatFsFileSystem::~FatFsFileSystem() {
  if (mounted_) {
    (void)Unmount();
  }
}

// ── vfs::FileSystem interface
// ─────────────────────────────────────────────────

auto FatFsFileSystem::GetName() const -> const char* { return "fatfs"; }

auto FatFsFileSystem::Mount(vfs::BlockDevice* device) -> Expected<vfs::Inode*> {
  if (device == nullptr) {
    klog::Err("FatFsFileSystem::Mount: device is nullptr\n");
    return std::unexpected(Error{ErrorCode::kInvalidArgument});
  }
  if (volume_id_ >= FF_VOLUMES) {
    return std::unexpected(Error{ErrorCode::kInvalidArgument});
  }

  // Register block device so diskio.cpp callbacks can reach it.
  g_block_devices[volume_id_] = device;

  // Build FatFS mount path: "0:/" or "1:/" etc.
  char path[4] = {'0' + static_cast<char>(volume_id_), ':', '/', '\0'};

  FRESULT fr = f_mount(&fatfs_obj_, path, 1);
  if (fr != FR_OK) {
    g_block_devices[volume_id_] = nullptr;
    klog::Err("FatFsFileSystem::Mount: f_mount failed (%d)\n",
              static_cast<int>(fr));
    return std::unexpected(Error{ErrorCode::kFsMountFailed});
  }

  // Build root inode.
  FatInode* fi = AllocateFatInode();
  if (fi == nullptr) {
    (void)f_mount(nullptr, path, 0);
    g_block_devices[volume_id_] = nullptr;
    return std::unexpected(Error{ErrorCode::kOutOfMemory});
  }
  fi->inode.ino = 0;
  fi->inode.type = vfs::FileType::kDirectory;
  fi->inode.size = 0;
  fi->inode.permissions = 0755;
  fi->inode.link_count = 1;
  fi->inode.fs = this;
  fi->inode.ops = &inode_ops_;
  fi->inode.fs_private = fi;
  strncpy(fi->path, path, sizeof(fi->path) - 1);
  fi->path[sizeof(fi->path) - 1] = '\0';

  root_inode_ = &fi->inode;
  mounted_ = true;
  return root_inode_;
}

auto FatFsFileSystem::Unmount() -> Expected<void> {
  if (!mounted_) {
    return {};
  }
  char path[4] = {'0' + static_cast<char>(volume_id_), ':', '/', '\0'};
  FRESULT fr = f_mount(nullptr, path, 0);
  g_block_devices[volume_id_] = nullptr;
  mounted_ = false;
  root_inode_ = nullptr;
  // Free all inodes
  for (auto& node : inodes_) {
    node.in_use = false;
  }
  return FresultToExpected(fr);
}

auto FatFsFileSystem::Sync() -> Expected<void> {
  if (volume_id_ < FF_VOLUMES && g_block_devices[volume_id_] != nullptr) {
    return g_block_devices[volume_id_]->Flush();
  }
  return {};
}

auto FatFsFileSystem::AllocateInode() -> Expected<vfs::Inode*> {
  FatInode* fi = AllocateFatInode();
  if (fi == nullptr) {
    return std::unexpected(Error{ErrorCode::kOutOfMemory});
  }
  fi->inode.fs = this;
  fi->inode.ops = &inode_ops_;
  fi->inode.fs_private = fi;
  return &fi->inode;
}

auto FatFsFileSystem::FreeInode(vfs::Inode* inode) -> Expected<void> {
  if (inode == nullptr) {
    return std::unexpected(Error{ErrorCode::kInvalidArgument});
  }
  auto* fi = static_cast<FatInode*>(inode->fs_private);
  FreeFatInode(fi);
  return {};
}

auto FatFsFileSystem::GetFileOps() -> vfs::FileOps* { return &file_ops_; }

// ── Inode pool helpers
// ────────────────────────────────────────────────────────

auto FatFsFileSystem::AllocateFatInode() -> FatInode* {
  for (auto& fi : inodes_) {
    if (!fi.in_use) {
      fi = FatInode{};
      fi.in_use = true;
      return &fi;
    }
  }
  return nullptr;
}

auto FatFsFileSystem::FreeFatInode(FatInode* fi) -> void {
  if (fi != nullptr) {
    fi->in_use = false;
  }
}

// ── FIL pool helpers
// ──────────────────────────────────────────────────────────

auto FatFsFileSystem::AllocateFil() -> FIL* {
  for (auto& fh : fil_pool_) {
    if (!fh.in_use) {
      fh = FatFileHandle{};
      fh.in_use = true;
      return &fh.fil;
    }
  }
  return nullptr;
}

auto FatFsFileSystem::FreeFil(FIL* fil) -> void {
  for (auto& fh : fil_pool_) {
    if (&fh.fil == fil) {
      fh.in_use = false;
      return;
    }
  }
}

auto FatFsFileSystem::OpenFil(vfs::Inode* inode, uint32_t open_flags)
    -> Expected<void> {
  auto* fi = static_cast<FatInode*>(inode->fs_private);
  if (fi->fil != nullptr) {
    return {};  // already open
  }
  FIL* fil = AllocateFil();
  if (fil == nullptr) {
    return std::unexpected(Error{ErrorCode::kFsFdTableFull});
  }
  BYTE fa_mode = 0;
  // kOReadOnly == 0, so check it specially
  if (open_flags == vfs::kOReadOnly) {
    fa_mode = FA_READ;
  }
  if ((open_flags & vfs::kOWriteOnly) != 0u) {
    fa_mode = FA_WRITE;
  }
  if ((open_flags & vfs::kOReadWrite) != 0u) {
    fa_mode = FA_READ | FA_WRITE;
  }
  if ((open_flags & vfs::kOCreate) != 0u) {
    fa_mode |= FA_OPEN_ALWAYS;
  }
  if ((open_flags & vfs::kOTruncate) != 0u) {
    fa_mode |= FA_CREATE_ALWAYS;
  }
  FRESULT fr = f_open(fil, fi->path, fa_mode);
  if (fr != FR_OK) {
    FreeFil(fil);
    return std::unexpected(Error{ErrorCode::kFsInvalidFd});
  }
  fi->fil = fil;
  return {};
}

// ── FatFsInodeOps
// ─────────────────────────────────────────────────────────────

auto FatFsFileSystem::FatFsInodeOps::Lookup(vfs::Inode* dir, const char* name)
    -> Expected<vfs::Inode*> {
  auto* dir_fi = static_cast<FatInode*>(dir->fs_private);

  // Build full path: dir_path + name
  char full_path[512];
  sk_snprintf(full_path, sizeof(full_path), "%s%s", dir_fi->path, name);

  FILINFO fi_info;
  FRESULT fr = f_stat(full_path, &fi_info);
  if (fr != FR_OK) {
    return std::unexpected(Error{ErrorCode::kFsFileNotFound});
  }

  auto inode_result = fs_->AllocateInode();
  if (!inode_result) {
    return std::unexpected(inode_result.error());
  }
  vfs::Inode* inode = *inode_result;
  auto* new_fi = static_cast<FatInode*>(inode->fs_private);
  inode->ino = 0;
  inode->type = FilInfoToFileType(fi_info);
  inode->size = fi_info.fsize;
  inode->permissions = 0644;
  inode->link_count = 1;
  strncpy(new_fi->path, full_path, sizeof(new_fi->path) - 1);
  new_fi->path[sizeof(new_fi->path) - 1] = '\0';
  return inode;
}

auto FatFsFileSystem::FatFsInodeOps::Create(vfs::Inode* dir, const char* name,
                                            vfs::FileType type)
    -> Expected<vfs::Inode*> {
  auto* dir_fi = static_cast<FatInode*>(dir->fs_private);
  char full_path[512];
  sk_snprintf(full_path, sizeof(full_path), "%s%s", dir_fi->path, name);

  if (type == vfs::FileType::kDirectory) {
    FRESULT fr = f_mkdir(full_path);
    if (fr != FR_OK) {
      return std::unexpected(Error{ErrorCode::kFsCorrupted});
    }
  } else {
    FIL fil;
    FRESULT fr = f_open(&fil, full_path, FA_CREATE_NEW | FA_WRITE);
    if (fr != FR_OK) {
      return std::unexpected(Error{ErrorCode::kFsFileExists});
    }
    (void)f_close(&fil);
  }

  auto inode_result = fs_->AllocateInode();
  if (!inode_result) {
    return std::unexpected(inode_result.error());
  }
  vfs::Inode* inode = *inode_result;
  auto* new_fi = static_cast<FatInode*>(inode->fs_private);
  inode->ino = 0;
  inode->type = type;
  inode->size = 0;
  inode->permissions = 0644;
  inode->link_count = 1;
  strncpy(new_fi->path, full_path, sizeof(new_fi->path) - 1);
  new_fi->path[sizeof(new_fi->path) - 1] = '\0';
  return inode;
}

auto FatFsFileSystem::FatFsInodeOps::Unlink(vfs::Inode* dir, const char* name)
    -> Expected<void> {
  auto* dir_fi = static_cast<FatInode*>(dir->fs_private);
  char full_path[512];
  sk_snprintf(full_path, sizeof(full_path), "%s%s", dir_fi->path, name);
  return FresultToExpected(f_unlink(full_path));
}

auto FatFsFileSystem::FatFsInodeOps::Mkdir(vfs::Inode* dir, const char* name)
    -> Expected<vfs::Inode*> {
  return Create(dir, name, vfs::FileType::kDirectory);
}

auto FatFsFileSystem::FatFsInodeOps::Rmdir(vfs::Inode* dir, const char* name)
    -> Expected<void> {
  return Unlink(dir, name);  // f_unlink handles both files and empty dirs
}

// ── FatFsFileOps
// ──────────────────────────────────────────────────────────────

auto FatFsFileSystem::FatFsFileOps::Read(vfs::File* file, void* buf,
                                         size_t count) -> Expected<size_t> {
  auto* fi = static_cast<FatInode*>(file->inode->fs_private);
  if (fi->fil == nullptr) {
    return std::unexpected(Error{ErrorCode::kFsInvalidFd});
  }
  UINT bytes_read = 0;
  FRESULT fr = f_read(fi->fil, buf, static_cast<UINT>(count), &bytes_read);
  if (fr != FR_OK) {
    klog::Err("FatFsFileOps::Read: f_read failed (%d)\n", static_cast<int>(fr));
    return std::unexpected(Error{ErrorCode::kFsCorrupted});
  }
  file->offset += bytes_read;
  return static_cast<size_t>(bytes_read);
}

auto FatFsFileSystem::FatFsFileOps::Write(vfs::File* file, const void* buf,
                                          size_t count) -> Expected<size_t> {
  auto* fi = static_cast<FatInode*>(file->inode->fs_private);
  if (fi->fil == nullptr) {
    return std::unexpected(Error{ErrorCode::kFsInvalidFd});
  }
  UINT bytes_written = 0;
  FRESULT fr = f_write(fi->fil, buf, static_cast<UINT>(count), &bytes_written);
  if (fr != FR_OK) {
    klog::Err("FatFsFileOps::Write: f_write failed (%d)\n",
              static_cast<int>(fr));
    return std::unexpected(Error{ErrorCode::kFsCorrupted});
  }
  file->offset += bytes_written;
  file->inode->size = static_cast<uint64_t>(f_size(fi->fil));
  return static_cast<size_t>(bytes_written);
}

auto FatFsFileSystem::FatFsFileOps::Seek(vfs::File* file, int64_t offset,
                                         vfs::SeekWhence whence)
    -> Expected<uint64_t> {
  auto* fi = static_cast<FatInode*>(file->inode->fs_private);
  if (fi->fil == nullptr) {
    return std::unexpected(Error{ErrorCode::kFsInvalidFd});
  }
  FSIZE_t new_pos = 0;
  switch (whence) {
    case vfs::SeekWhence::kSet:
      new_pos = static_cast<FSIZE_t>(offset);
      break;
    case vfs::SeekWhence::kCur:
      new_pos =
          static_cast<FSIZE_t>(static_cast<int64_t>(f_tell(fi->fil)) + offset);
      break;
    case vfs::SeekWhence::kEnd:
      new_pos =
          static_cast<FSIZE_t>(static_cast<int64_t>(f_size(fi->fil)) + offset);
      break;
  }
  FRESULT fr = f_lseek(fi->fil, new_pos);
  if (fr != FR_OK) {
    return std::unexpected(Error{ErrorCode::kFsCorrupted});
  }
  file->offset = static_cast<uint64_t>(new_pos);
  return static_cast<uint64_t>(new_pos);
}

auto FatFsFileSystem::FatFsFileOps::Close(vfs::File* file) -> Expected<void> {
  auto* fi = static_cast<FatInode*>(file->inode->fs_private);
  if (fi->fil == nullptr) {
    return {};  // already closed
  }
  FRESULT fr = f_close(fi->fil);
  fs_->FreeFil(fi->fil);
  fi->fil = nullptr;
  return FresultToExpected(fr);
}

auto FatFsFileSystem::FatFsFileOps::ReadDir(vfs::File* file,
                                            vfs::DirEntry* dirent, size_t count)
    -> Expected<size_t> {
  auto* fi = static_cast<FatInode*>(file->inode->fs_private);
  DIR dir;
  FRESULT fr = f_opendir(&dir, fi->path);
  if (fr != FR_OK) {
    return std::unexpected(Error{ErrorCode::kFsCorrupted});
  }

  // Skip already-read entries (file->offset used as entry index).
  for (uint64_t i = 0; i < file->offset; ++i) {
    FILINFO fi_info;
    fr = f_readdir(&dir, &fi_info);
    if (fr != FR_OK || fi_info.fname[0] == '\0') {
      (void)f_closedir(&dir);
      return static_cast<size_t>(0);
    }
  }

  size_t read_count = 0;
  for (size_t i = 0; i < count; ++i) {
    FILINFO fi_info;
    fr = f_readdir(&dir, &fi_info);
    if (fr != FR_OK || fi_info.fname[0] == '\0') {
      break;
    }
    dirent[i].ino = 0;
    dirent[i].type = static_cast<uint8_t>((fi_info.fattrib & AM_DIR) != 0
                                              ? vfs::FileType::kDirectory
                                              : vfs::FileType::kRegular);
    strncpy(dirent[i].name, fi_info.fname, sizeof(dirent[i].name) - 1);
    dirent[i].name[sizeof(dirent[i].name) - 1] = '\0';
    ++read_count;
  }
  file->offset += static_cast<uint64_t>(read_count);
  (void)f_closedir(&dir);
  return read_count;
}

}  // namespace fatfs
