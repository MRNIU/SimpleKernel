/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cstdint>

#include "block_device_provider.hpp"
#include "device_manager.hpp"
#include "driver/ns16550a_driver.hpp"
#include "driver/virtio_blk_driver.hpp"
#include "kernel_fdt.hpp"
#include "kernel_log.hpp"
#include "kstd_memory"
#include "platform_bus.hpp"
#include "platform_traits.hpp"
#include "singleton.hpp"

/// 设备初始化入口
auto DeviceInit() -> void {
  auto& dm = Singleton<DeviceManager>::GetInstance();
  auto& fdt = Singleton<KernelFdt>::GetInstance();

  // 注册驱动
  auto reg_uart = dm.GetRegistry().Register<Ns16550aDriver>();
  if (!reg_uart.has_value()) {
    klog::Err("DeviceInit: failed to register Ns16550aDriver: %s\n",
              reg_uart.error().message());
    return;
  }

  auto reg_blk = dm.GetRegistry().Register<VirtioBlkDriver<PlatformTraits>>();
  if (!reg_blk.has_value()) {
    klog::Err("DeviceInit: failed to register VirtioBlkDriver: %s\n",
              reg_blk.error().message());
    return;
  }

  // 注册 Platform 总线并枚举设备
  PlatformBus platform_bus(fdt);
  auto bus_result = dm.RegisterBus(platform_bus);
  if (!bus_result.has_value()) {
    klog::Err("DeviceInit: PlatformBus enumeration failed: %s\n",
              bus_result.error().message());
    return;
  }

  // 为 virtio,mmio 设备分配 DMA 缓冲区
  constexpr size_t kMaxPlatformDevices = 64;
  DeviceNode* devs[kMaxPlatformDevices];
  size_t n =
      dm.FindDevicesByType(DeviceType::kPlatform, devs, kMaxPlatformDevices);
  for (size_t i = 0; i < n; ++i) {
    if (devs[i]->resource.IsPlatform()) {
      const auto& plat = std::get<PlatformId>(devs[i]->resource.id);
      if (strcmp(plat.compatible, "virtio,mmio") == 0) {
        auto buf = kstd::make_unique<IoBuffer>(
            VirtioBlkDriver<PlatformTraits>::kMinDmaBufferSize);
        if (buf && buf->IsValid()) {
          devs[i]->dma_buffer = buf.release();
          klog::Debug("DeviceInit: allocated DMA buffer for '%s'\n",
                      devs[i]->name);
        } else {
          klog::Warn("DeviceInit: failed to allocate DMA buffer for '%s'\n",
                     devs[i]->name);
          // unique_ptr auto-deletes on scope exit
        }
      }
    }
  }

  // 匹配驱动并 Probe
  auto probe_result = dm.ProbeAll();
  if (!probe_result.has_value()) {
    klog::Err("DeviceInit: ProbeAll failed: %s\n",
              probe_result.error().message());
    return;
  }

  klog::Info("DeviceInit: complete\n");
}

namespace {

/// @brief Adapts VirtioBlk<PlatformTraits> to vfs::BlockDevice.
class VirtioBlkBlockDevice final : public vfs::BlockDevice {
 public:
  using VirtioBlkType = typename VirtioBlkDriver<PlatformTraits>::VirtioBlkType;

  explicit VirtioBlkBlockDevice(VirtioBlkType* dev) : dev_(dev) {}

  auto ReadSectors(uint64_t lba, uint32_t count, void* buf)
      -> Expected<size_t> override {
    auto* ptr = static_cast<uint8_t*>(buf);
    for (uint32_t i = 0; i < count; ++i) {
      auto result = dev_->Read(lba + i, ptr + i * kSectorSize);
      if (!result) {
        return std::unexpected(df_bridge::ToKernelError(result.error()));
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
        return std::unexpected(df_bridge::ToKernelError(result.error()));
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
  using Driver = VirtioBlkDriver<PlatformTraits>;
  auto* raw = DriverRegistry::GetDriverInstance<Driver>().GetDevice();
  if (raw == nullptr) {
    klog::Err("GetVirtioBlkBlockDevice: no virtio-blk device probed\n");
    return nullptr;
  }
  static VirtioBlkBlockDevice adapter(raw);
  return &adapter;
}
