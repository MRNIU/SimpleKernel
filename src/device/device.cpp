/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "block_device_provider.hpp"
#include "device_manager.hpp"
#include "driver/ns16550a_driver.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "kernel.h"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "platform_bus.hpp"

/// Device subsystem initialisation entry point
auto DeviceInit() -> void {
  DeviceManagerSingleton::create();
  auto& dm = DeviceManagerSingleton::instance();

  if (auto r = dm.GetRegistry().Register(Ns16550aDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register Ns16550aDriver failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.GetRegistry().Register(VirtioBlkDriver::GetEntry()); !r) {
    klog::Err("DeviceInit: register VirtioBlkDriver failed: %s\n",
              r.error().message());
    return;
  }

  PlatformBus platform_bus(KernelFdtSingleton::instance());
  if (auto r = dm.RegisterBus(platform_bus); !r) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              r.error().message());
    return;
  }

  if (auto r = dm.ProbeAll(); !r) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n", r.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}

namespace {

/// Adapts VirtioBlk<> to vfs::BlockDevice.
class VirtioBlkBlockDevice final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = VirtioBlkDriver::VirtioBlkType;

  explicit VirtioBlkBlockDevice(VirtioBlkType* dev) : dev_(dev) {}

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
    return "virtio-blk0";
  }

 private:
  static constexpr uint32_t kSectorSize = 512;
  VirtioBlkType* dev_;
};

}  // namespace

auto GetVirtioBlkBlockDevice() -> vfs::BlockDevice* {
  auto* raw = VirtioBlkDriver::Instance().GetDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: no virtio-blk device probed\n");
    return nullptr;
  }
  static VirtioBlkBlockDevice adapter(raw);
  return &adapter;
}
