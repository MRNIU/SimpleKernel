/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_

#include <device_framework/virtio_blk.hpp>

#include "device_node.hpp"
#include "df_bridge.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"

template <typename Traits>
class VirtioBlkDriver {
 public:
  using VirtioBlkType = device_framework::virtio::blk::VirtioBlk<Traits>;

  static constexpr PlatformCompatible kPlatformMatch{"virtio,mmio"};
  static constexpr MatchEntry kMatchTable[] = {
      kPlatformMatch,
  };

  static auto GetDescriptor() -> const DriverDescriptor& {
    static const DriverDescriptor desc{
        .name = "virtio-blk",
        .match_table = kMatchTable,
        .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
    };
    return desc;
  }

  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = df_bridge::PrepareMmioProbe(node, 0x1000);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    auto base = ctx->base;

    auto magic = *reinterpret_cast<volatile uint32_t*>(base);
    if (magic != device_framework::virtio::kMmioMagicValue) {
      klog::Debug("VirtioBlkDriver: 0x%lX not a VirtIO device (magic=0x%X)\n",
                  base, magic);
      node.bound.store(false, std::memory_order_release);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    constexpr uint32_t kBlockDeviceId = 2;
    auto device_id = *reinterpret_cast<volatile uint32_t*>(
        base + device_framework::virtio::MmioTransport<>::MmioReg::kDeviceId);
    if (device_id != kBlockDeviceId) {
      klog::Debug("VirtioBlkDriver: 0x%lX device_id=%u (not block)\n", base,
                  device_id);
      node.bound.store(false, std::memory_order_release);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    uint64_t extra_features =
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSegMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kSizeMax) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kBlkSize) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kFlush) |
        static_cast<uint64_t>(
            device_framework::virtio::blk::BlkFeatureBit::kGeometry);

    auto result = VirtioBlkType::Create(base, dma_buf_, 1, 128, extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", base);
      node.bound.store(false, std::memory_order_release);
      return std::unexpected(df_bridge::ToKernelError(result.error()));
    }

    device_.Emplace(std::move(*result));
    node.type = DeviceType::kBlock;

    if (node.resource.irq_count > 0) {
      irq_ = node.resource.irq[0];
    }

    auto capacity = device_.Get()->GetCapacity();
    klog::Info(
        "VirtioBlkDriver: block device at 0x%lX, capacity=%lu sectors, "
        "irq=%u\n",
        base, capacity, irq_);

    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    device_.Destroy();
    return {};
  }

  [[nodiscard]] auto GetDevice() -> VirtioBlkType* { return device_.Get(); }

  [[nodiscard]] auto GetIrq() const -> uint32_t { return irq_; }

  template <typename CompletionCallback>
  auto HandleInterrupt(CompletionCallback&& on_complete) -> void {
    if (auto* dev = device_.Get(); dev != nullptr) {
      dev->HandleInterrupt(static_cast<CompletionCallback&&>(on_complete));
    }
  }

 private:
  alignas(4096) uint8_t dma_buf_[32768]{};
  df_bridge::DeviceStorage<VirtioBlkType> device_;
  uint32_t irq_{0};
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_ */
