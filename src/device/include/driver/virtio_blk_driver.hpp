/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_

#include <etl/io_port.h>

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
  static constexpr uint32_t kMmioRegionSize = 0x1000;
  static constexpr uint32_t kDefaultQueueCount = 1;
  static constexpr uint32_t kDefaultQueueSize = 128;
  static constexpr size_t kMinDmaBufferSize = 32768;

  static auto GetDescriptor() -> const DriverDescriptor& {
    static const DriverDescriptor desc{
        .name = "virtio-blk",
        .match_table = kMatchTable,
        .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
    };
    return desc;
  }

  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = df_bridge::PrepareMmioProbe(node, kMmioRegionSize);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    auto base = ctx->base;

    etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(base)};
    auto magic = magic_reg.read();
    if (magic != device_framework::virtio::kMmioMagicValue) {
      klog::Debug("VirtioBlkDriver: 0x%lX not a VirtIO device (magic=0x%X)\n",
                  base, magic);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    constexpr uint32_t kBlockDeviceId = 2;
    etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<void*>(
        base + device_framework::virtio::MmioTransport<>::MmioReg::kDeviceId)};
    auto device_id = device_id_reg.read();
    if (device_id != kBlockDeviceId) {
      klog::Debug("VirtioBlkDriver: 0x%lX device_id=%u (not block)\n", base,
                  device_id);
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

    if (node.dma_buffer == nullptr || !node.dma_buffer->IsValid()) {
      klog::Err(
          "VirtioBlkDriver: Missing or invalid DMA buffer in DeviceNode at "
          "0x%lX\n",
          base);
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
    }

    if (node.dma_buffer->GetBuffer().second < kMinDmaBufferSize) {
      klog::Err("VirtioBlkDriver: DMA buffer too small (%zu < %u)\n",
                node.dma_buffer->GetBuffer().second, kMinDmaBufferSize);
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
    }

    // Pass the pointer to Create (ensure the size meets virtio queue
    // requirements) GetBuffer() returns std::pair<uint8_t*, size_t>
    auto result = VirtioBlkType::Create(
        base, node.dma_buffer->GetBuffer().first, kDefaultQueueCount,
        kDefaultQueueSize, extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", base);
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
  df_bridge::DeviceStorage<VirtioBlkType> device_;
  uint32_t irq_{0};
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_ */
