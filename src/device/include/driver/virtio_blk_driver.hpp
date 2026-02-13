/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_

#include <device_framework/virtio_blk.hpp>
#include <new>

#include "device_node.hpp"
#include "driver_registry.hpp"
#include "expected.hpp"
#include "kernel_log.hpp"
#include "singleton.hpp"
#include "virtual_memory.hpp"

/// VirtIO 块设备驱动
/// @tparam Traits 满足 VirtioTraits 的平台特性类
template <typename Traits>
class VirtioBlkDriver {
 public:
  using VirtioBlkType = device_framework::virtio::blk::VirtioBlk<Traits>;

  /// 驱动匹配表
  static constexpr PlatformCompatible kPlatformMatch{"virtio,mmio"};
  static constexpr MatchEntry kMatchTable[] = {
      kPlatformMatch,
  };

  /// 驱动描述符
  static auto GetDescriptor() -> const DriverDescriptor& {
    static const DriverDescriptor desc{
        .name = "virtio-blk",
        .match_table = kMatchTable,
        .match_count = sizeof(kMatchTable) / sizeof(kMatchTable[0]),
    };
    return desc;
  }

  /// Probe — 初始化 VirtIO 块设备
  auto Probe(DeviceNode& node) -> Expected<void> {
    if (!node.TryBind()) {
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    uint64_t base = node.resource.mmio[0].base;
    if (base == 0) {
      klog::Err("VirtioBlkDriver: no MMIO base for '%s'\n", node.name);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    // 映射 MMIO 区域
    Singleton<VirtualMemory>::GetInstance().MapMMIO(
        base,
        node.resource.mmio[0].size > 0 ? node.resource.mmio[0].size : 0x1000);

    // 扫描此 MMIO 地址是否为 VirtIO 块设备
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

    // 配置所需特性
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

    // 创建设备实例
    auto result = VirtioBlkType::Create(base, dma_buf_, 1, 128, extra_features);
    if (!result.has_value()) {
      klog::Err("VirtioBlkDriver: Create failed at 0x%lX\n", base);
      node.bound.store(false, std::memory_order_release);
      return std::unexpected(Error(ErrorCode::kDeviceNotFound));
    }

    // 通过 placement new 将实例移动构造到静态存储
    new (&GetStorage()) VirtioBlkType(std::move(*result));
    instance_valid_ = true;
    node.type = DeviceType::kBlock;

    // 记录中断号供后续 PLIC 注册
    if (node.resource.irq_count > 0) {
      irq_ = node.resource.irq[0];
    }

    auto capacity = GetDevice()->GetCapacity();
    klog::Info(
        "VirtioBlkDriver: block device at 0x%lX, capacity=%lu sectors, "
        "irq=%u\n",
        base, capacity, irq_);

    return {};
  }

  /// Remove — 目前为空实现
  auto Remove([[maybe_unused]] DeviceNode& node) -> Expected<void> {
    instance_valid_ = false;
    return {};
  }

  /// 获取块设备实例（需 Probe 成功后使用）
  [[nodiscard]] static auto GetDevice() -> VirtioBlkType* {
    return instance_valid_ ? reinterpret_cast<VirtioBlkType*>(&GetStorage())
                           : nullptr;
  }

  /// 获取设备的 IRQ 号
  [[nodiscard]] static auto GetIrq() -> uint32_t { return irq_; }

  /// 中断处理入口 — 调用 HandleInterrupt(CompletionCallback&&)
  template <typename CompletionCallback>
  static auto HandleInterrupt(CompletionCallback&& on_complete) -> void {
    if (auto* dev = GetDevice(); dev != nullptr) {
      dev->HandleInterrupt(static_cast<CompletionCallback&&>(on_complete));
    }
  }

 private:
  /// 静态 DMA 缓冲区
  alignas(4096) static inline uint8_t dma_buf_[32768]{};

  /// 静态存储（未初始化，通过 placement new 构造）
  struct alignas(alignof(VirtioBlkType)) StorageBlock {
    uint8_t data[sizeof(VirtioBlkType)];
  };
  static auto GetStorage() -> StorageBlock& {
    static StorageBlock storage{};
    return storage;
  }

  static inline bool instance_valid_{false};
  static inline uint32_t irq_{0};
};

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_VIRTIO_BLK_DRIVER_HPP_ */
