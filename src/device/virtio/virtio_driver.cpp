/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "virtio/virtio_driver.hpp"

#include <etl/io_port.h>

#include "expected.hpp"
#include "kernel_log.hpp"
#include "virtio/transport/mmio.hpp"

auto VirtioDriver::MatchStatic(DeviceNode& node) -> bool {
  if (node.mmio_base == 0) return false;

  auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
  if (!ctx) return false;

  etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(ctx->base)};
  if (magic_reg.read() != virtio::kMmioMagicValue) {
    klog::Debug("VirtioDriver: {:#x} not a VirtIO device\n", ctx->base);
    return false;
  }
  return true;
}

auto VirtioDriver::Probe(DeviceNode& node) -> Expected<void> {
  auto ctx = mmio_helper::Prepare(node, kMmioRegionSize);
  if (!ctx) {
    return std::unexpected(ctx.error());
  }

  // 读取 device_id
  etl::io_port_ro<uint32_t> device_id_reg{reinterpret_cast<void*>(
      ctx->base + virtio::MmioTransport::MmioReg::kDeviceId)};
  const auto device_id = static_cast<DeviceId>(device_id_reg.read());

  switch (device_id) {
    case DeviceId::kBlock: {
      // 分配 DMA buffer
      dma_buffer_ = kstd::make_unique<IoBuffer>(kMinDmaBufferSize);
      if (!dma_buffer_ || !dma_buffer_->IsValid() ||
          dma_buffer_->GetBuffer().size() < kMinDmaBufferSize) {
        klog::Err("VirtioDriver: failed to allocate DMA buffer at {:#x}\n",
                  ctx->base);
        return std::unexpected(Error(ErrorCode::kOutOfMemory));
      }

      uint64_t extra_features =
          static_cast<uint64_t>(virtio::blk::BlkFeatureBit::kSegMax) |
          static_cast<uint64_t>(virtio::blk::BlkFeatureBit::kSizeMax) |
          static_cast<uint64_t>(virtio::blk::BlkFeatureBit::kBlkSize) |
          static_cast<uint64_t>(virtio::blk::BlkFeatureBit::kFlush) |
          static_cast<uint64_t>(virtio::blk::BlkFeatureBit::kGeometry);

      auto result = virtio::blk::VirtioBlk<>::Create(
          ctx->base, dma_buffer_->GetBuffer().data(), kDefaultQueueCount,
          kDefaultQueueSize, extra_features);
      if (!result.has_value()) {
        klog::Err("VirtioDriver: VirtioBlk Create failed at {:#x}\n",
                  ctx->base);
        return std::unexpected(Error(result.error().code));
      }

      blk_device_.emplace(std::move(*result));
      node.type = DeviceType::kBlock;
      irq_ = node.irq;

      // Register adapter in pool and expose via DeviceNode.
      if (blk_adapter_count_ < kMaxBlkDevices) {
        const auto idx = static_cast<uint32_t>(blk_adapter_count_);
        blk_adapters_[blk_adapter_count_].emplace(&blk_device_.value(), idx);
        node.block_device = &blk_adapters_[blk_adapter_count_].value();
        ++blk_adapter_count_;
      } else {
        klog::Warn(
            "VirtioDriver: blk adapter pool full, device at {:#x} skipped\n",
            ctx->base);
      }

      klog::Info(
          "VirtioDriver: block device at {:#x}, capacity={} sectors, "
          "irq={}\n",
          ctx->base, blk_device_.value().GetCapacity(), irq_);
      return {};
    }

    default:
      klog::Warn("VirtioDriver: unsupported device_id={} at {:#x}\n",
                 static_cast<uint32_t>(device_id), ctx->base);
      return std::unexpected(Error(ErrorCode::kNotSupported));
  }
}
