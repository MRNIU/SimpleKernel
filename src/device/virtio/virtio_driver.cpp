/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "virtio/virtio_driver.hpp"

#include <etl/io_port.h>

#include "expected.hpp"
#include "kernel_log.hpp"
#include "virtio/transport/mmio.hpp"

auto VirtioDriver::MatchStatic([[maybe_unused]] DeviceNode& node) -> bool {
  return node.mmio_base != 0 && node.mmio_size != 0;
}

auto VirtioDriver::Probe(DeviceNode& node) -> Expected<void> {
  if (node.mmio_size == 0) {
    klog::err << "VirtioDriver: FDT reg property missing size for node '"
              << node.name << "'\n";
    return std::unexpected(Error(ErrorCode::kInvalidArgument));
  }
  auto ctx = mmio_helper::Prepare(node, node.mmio_size);
  if (!ctx) {
    return std::unexpected(ctx.error());
  }

  etl::io_port_ro<uint32_t> magic_reg{reinterpret_cast<void*>(ctx->base)};
  if (magic_reg.read() != virtio::kMmioMagicValue) {
    klog::debug() << "VirtioDriver: " << klog::hex << ctx->base
                  << " not a VirtIO device";
    return std::unexpected(Error(ErrorCode::kNotSupported));
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
        klog::err << "VirtioDriver: failed to allocate DMA buffer at "
                  << klog::hex << ctx->base;
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
        klog::err << "VirtioDriver: VirtioBlk Create failed at " << klog::hex
                  << ctx->base;
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
        klog::warn << "VirtioDriver: blk adapter pool full, device at "
                   << klog::hex << ctx->base << " skipped";
      }

      klog::info << "VirtioDriver: block device at " << klog::hex << ctx->base
                 << ", capacity=" << blk_device_.value().GetCapacity()
                 << " sectors, irq=" << irq_;
      return {};
    }

    default:
      klog::warn << "VirtioDriver: unsupported device_id="
                 << static_cast<uint32_t>(device_id) << " at " << klog::hex
                 << ctx->base;
      return std::unexpected(Error(ErrorCode::kNotSupported));
  }
}
