/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "filesystem.hpp"
#include "kernel_log.hpp"
#include "sk_cstring"
#include "vfs_internal.hpp"

namespace vfs {

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

}  // namespace vfs
