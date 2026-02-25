/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "filesystem.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "vfs_internal.hpp"

namespace vfs {

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

}  // namespace vfs
