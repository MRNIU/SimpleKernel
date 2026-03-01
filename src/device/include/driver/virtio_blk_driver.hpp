/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief VirtIO block device driver
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_

#include <etl/io_port.h>
#include <etl/optional.h>
#include <etl/span.h>

#include "device_node.hpp"
#include "driver/detail/virtio/device/virtio_blk_device.hpp"
#include "driver/detail/virtio/traits.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "io_buffer.hpp"
#include "kernel_log.hpp"
#include "kstd_memory"
#include "mmio_helper.hpp"

class VirtioBlkDriver {
 public:
  using VirtioBlkType = detail::virtio::blk::VirtioBlk<>;

  static constexpr uint32_t kMmioRegionSize = 0x1000;
  static constexpr uint32_t kDefaultQueueCount = 1;
  static constexpr uint32_t kDefaultQueueSize = 128;
  static constexpr size_t kMinDmaBufferSize = 32768;

  // --- Registration API ---

  static auto Instance() -> VirtioBlkDriver& {
    static VirtioBlkDriver inst;
    return inst;
  }

  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .name = "virtio-blk",
        .match_table = etl::span<const MatchEntry>(kMatchTable),
        .match = etl::delegate<bool(
            DeviceNode&)>::create<&VirtioBlkDriver::MatchStatic>(),
        .probe = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioBlkDriver, &VirtioBlkDriver::Probe>(Instance()),
        .remove = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioBlkDriver, &VirtioBlkDriver::Remove>(Instance()),
    };
    return entry;
  }

  // --- Driver lifecycle ---

  /**
   * @brief Hardware detection: read VirtIO magic + device_id registers.
   *
   * Maps MMIO temporarily to inspect the VirtIO header. Returns true only
   * if the device is a VirtIO block device (magic == 0x74726976, id == 2).
   *
   * @pre  node.mmio_base != 0 (already validated by FindDriver path)
   */
  static auto MatchStatic(DeviceNode& node) -> bool {
    if (node.mmio_base == 0) return false;

    // Map the MMIO region so we can read registers
    auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
    if (!ctx) return false;

    etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(ctx->base)};
    if (magic_reg.read() != detail::virtio::kMmioMagicValue) {
      klog::Debug("VirtioBlkDriver: 0x%lX not a VirtIO device\n", ctx->base);
      return false;
    }

    constexpr uint32_t kBlockDeviceId = 2;
    etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<void*>(
        ctx->base + detail::virtio::MmioTransport::MmioReg::kDeviceId)};
    if (device_id_reg.read() != kBlockDeviceId) {
      klog::Debug("VirtioBlkDriver: 0x%lX is not a block device\n", ctx->base);
      return false;
    }

    return true;
  }

  /**
   * @brief Initialize VirtIO block device.
   *
   * Allocates DMA buffer internally (was previously pre-allocated in
   * device.cpp's DMA loop). Maps MMIO, negotiates features, creates device.
   *
   * @pre  node.mmio_base != 0
   * @post device_ is valid; node.type == DeviceType::kBlock
   */
  auto Probe(DeviceNode& node) -> Expected<void> {
    auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
    if (!ctx) {
      return std::unexpected(ctx.error());
    }

    // Allocate DMA buffer (driver-owned, not stored in DeviceNode)
    dma_buffer_ = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
    if (!dma_buffer_ || !dma_buffer_->IsValid()) {
      klog::Err("VirtioBlkDriver: failed to allocate DMA buffer at 0x%lX\n",
                ctx->base);
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }
    if (dma_buffer_->GetBuffer().size() < kMinDmaBufferSize) {
      klog::Err("VirtioBlkDriver: DMA buffer too small (%zu < %u)\n",
                dma_buffer_->GetBuffer().size(), kMinDmaBufferSize);
      return std::unexpected(Error(ErrorCode::kInvalidArgument));
    }

    uint64_t extra_features =
        static_cast<uint64_t>(detail::virtio::blk::BlkFeatureBit::kSegMax) |
        static_cast<uint64_t>(detail::virtio::blk::BlkFeatureBit::kSizeMax) |
        static_cast<uint64_t>(detail::virtio::blk::BlkFeatureBit::kBlkSize) |
        static_cast<uint64_t>(detail::virtio::blk::BlkFeatureBit::kFlush) |
        static_cast<uint64_t>(detail::virtio::blk::BlkFeatureBit::kGeometry);

    auto result = VirtioBlkType::Create(
        ctx->base, dma_buffer_->GetBuffer().data(), kDefaultQueueCount,
        kDefaultQueueSize, extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", ctx->base);
      return std::unexpected(Error(result.error().code));
    }

    device_.emplace(std::move(*result));
    node.type = DeviceType::kBlock;
    irq_ = node.irq;

    klog::Info(
        "VirtioBlkDriver: block device at 0x%lX, capacity=%lu sectors, "
        "irq=%u\n",
        ctx->base, device_.value().GetCapacity(), irq_);

    return {};
  }

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    device_.reset();
    dma_buffer_.reset();
    return {};
  }

  [[nodiscard]] auto GetDevice() -> VirtioBlkType* {
    return device_.has_value() ? &device_.value() : nullptr;
  }

  [[nodiscard]] auto GetIrq() const -> uint32_t { return irq_; }

  template <typename CompletionCallback>
  auto HandleInterrupt(CompletionCallback&& on_complete) -> void {
    if (device_.has_value()) {
      device_.value().HandleInterrupt(
          static_cast<CompletionCallback&&>(on_complete));
    }
  }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "virtio,mmio"},
  };

  etl::optional<VirtioBlkType> device_;
  etl::unique_ptr<IoBuffer> dma_buffer_;
  uint32_t irq_{0};
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
