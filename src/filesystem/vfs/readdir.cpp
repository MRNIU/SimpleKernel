/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "filesystem.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "vfs_internal.hpp"

namespace vfs {

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
}  // namespace vfs
