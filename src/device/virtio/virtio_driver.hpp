/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_VIRTIO_VIRTIO_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_VIRTIO_VIRTIO_DRIVER_HPP_

#include <etl/io_port.h>
#include <etl/optional.h>
#include <etl/span.h>

#include <variant>

#include "device_manager.hpp"
#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "io_buffer.hpp"
#include "kernel_log.hpp"
#include "kstd_memory"
#include "virtio/device/virtio_blk.hpp"
#include "virtio/device/virtio_blk_vfs_adapter.hpp"

/**
 * @brief 统一 VirtIO 驱动
 *
 * 匹配所有 virtio,mmio 和 virtio,pci 兼容设备。
 * Probe() 运行期读取 device_id 寄存器，自动分发到对应设备实现
 * 并注册到 DeviceManager。调用方无需关心具体 VirtIO 设备类型。
 *
 * @see docs/plans/2026-03-01-device-refactor-design.md
 */
class VirtioDriver {
 public:
  /// VirtIO 设备类型枚举（来自 VirtIO 1.2 规范）
  enum class DeviceId : uint32_t {
    kNet = 1,
    kBlock = 2,
    kConsole = 3,
    kEntropy = 4,
    kGpu = 16,
    kInput = 18,
  };

  static constexpr uint32_t kMmioRegionSize = 0x1000;
  static constexpr uint32_t kDefaultQueueCount = 1;
  static constexpr uint32_t kDefaultQueueSize = 128;
  static constexpr size_t kMinDmaBufferSize = 32768;

  static auto Instance() -> VirtioDriver& {
    static VirtioDriver inst;
    return inst;
  }

  /**
   * @brief 返回驱动注册入口
   *
   * 匹配 virtio,mmio 兼容字符串；MatchStatic 检查 VirtIO magic number。
   */
  static auto GetEntry() -> const DriverEntry& {
    static const DriverEntry entry{
        .name = "virtio",
        .match_table = etl::span<const MatchEntry>(kMatchTable),
        .match = etl::delegate<bool(
            DeviceNode&)>::create<&VirtioDriver::MatchStatic>(),
        .probe = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioDriver, &VirtioDriver::Probe>(Instance()),
        .remove = etl::delegate<Expected<void>(DeviceNode&)>::create<
            VirtioDriver, &VirtioDriver::Remove>(Instance()),
    };
    return entry;
  }

  /**
   * @brief 硬件检测：验证 VirtIO magic number
   *
   * 只检查 magic number（0x74726976），不检查 device_id——
   * device_id 的分发在 Probe() 中完成。
   *
   * @pre  node.mmio_base != 0
   * @return true 如果设备响应 VirtIO magic
   */
  static auto MatchStatic(DeviceNode& node) -> bool;

  /**
   * @brief 初始化 VirtIO 设备
   *
   * 运行期读取 device_id，分发到对应设备实现，创建并注册设备。
   *
   * @pre  node.mmio_base != 0，MatchStatic() 已返回 true
   * @post 设备已注册到 DeviceManager（node.block_device 已填入适配器指针）
   */
  auto Probe(DeviceNode& node) -> Expected<void>;

  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    blk_device_.reset();
    dma_buffer_.reset();
    return {};
  }

  [[nodiscard]] auto GetBlkDevice() -> detail::virtio::blk::VirtioBlk<>* {
    return blk_device_.has_value() ? &blk_device_.value() : nullptr;
  }

  [[nodiscard]] auto GetIrq() const -> uint32_t { return irq_; }

  template <typename CompletionCallback>
  auto HandleInterrupt(CompletionCallback&& on_complete) -> void {
    if (blk_device_.has_value()) {
      blk_device_.value().HandleInterrupt(
          static_cast<CompletionCallback&&>(on_complete));
    }
  }

 private:
  static constexpr MatchEntry kMatchTable[] = {
      {BusType::kPlatform, "virtio,mmio"},
  };

  etl::optional<detail::virtio::blk::VirtioBlk<>> blk_device_;
  etl::unique_ptr<IoBuffer> dma_buffer_;
  uint32_t irq_{0};

  // Static adapter pool — one slot per probed blk device (kernel lifetime).
  static constexpr size_t kMaxBlkDevices = 4;
  etl::optional<detail::virtio::blk::VirtioBlkVfsAdapter>
      blk_adapters_[kMaxBlkDevices];
  size_t blk_adapter_count_{0};
};

#endif  // SIMPLEKERNEL_SRC_DEVICE_VIRTIO_VIRTIO_DRIVER_HPP_
