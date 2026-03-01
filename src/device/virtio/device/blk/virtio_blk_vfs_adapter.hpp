/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_VFS_ADAPTER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_VFS_ADAPTER_HPP_

#include <array>

#include "block_device.hpp"
#include "expected.hpp"
#include "virtio/device/blk/virtio_blk.hpp"

namespace virtio::blk {

/**
 * @brief Adapts VirtioBlk<> to vfs::BlockDevice.
 *
 * Wraps a VirtioBlk instance and forwards ReadSectors/WriteSectors
 * to the underlying VirtIO block device.
 */
class VirtioBlkVfsAdapter final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = VirtioBlk<>;

  explicit VirtioBlkVfsAdapter(VirtioBlkType* dev, uint32_t index = 0)
      : dev_(dev), index_(index) {}

  auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  auto WriteSectors(uint64_t lba, uint32_t count, const void* buf)
      -> Expected<size_t> override {
    const auto* ptr = static_cast<const uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Write(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(Error(result.error().code));
      }
    }
    return static_cast<size_t>(count) * kSectorSize;
  }

  [[nodiscard]] auto GetSectorSize() const -> uint32_t override {
    return kSectorSize;
  }

  [[nodiscard]] auto GetSectorCount() const -> uint64_t override {
    return dev_->GetCapacity();
  }

  [[nodiscard]] auto GetName() const -> const char* override {
    static constexpr std::array<const char*, 4> kNames = {
        "virtio-blk0", "virtio-blk1", "virtio-blk2", "virtio-blk3"};
    return (index_ < kNames.size()) ? kNames[index_] : "virtio-blk?";
  }

 private:
  static constexpr uint32_t kSectorSize = 512;
  VirtioBlkType* dev_;
  uint32_t index_{0};
};

}  // namespace virtio::blk

#endif  // SIMPLEKERNEL_SRC_DEVICE_VIRTIO_DEVICE_BLK_VIRTIO_BLK_VFS_ADAPTER_HPP_
