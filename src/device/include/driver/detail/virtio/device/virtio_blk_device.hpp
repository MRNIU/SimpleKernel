/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_DEVICE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_DEVICE_HPP_

#include <cstdint>
#include <span>
#include <utility>

#include "driver/detail/virtio/device/virtio_blk.hpp"
#include "driver/ops/block_device_ops.hpp"
#include "expected.hpp"

namespace detail::virtio::blk {

/**
 * @brief VirtIO 块设备统一接口适配器
 *
 * 将底层 VirtioBlk 驱动适配到统一的 BlockDevice 接口。
 * 用户可通过
 * Open/Read/Write/Release、ReadBlock/WriteBlock/ReadBlocks/WriteBlocks
 * 等标准接口操作 VirtIO 块设备，与 NS16550A 等其他驱动获得一致的使用体验。
 *
 * 使用示例：
 * @code
 * @code VirtioBlkDevice<>::Create(mmio_base, dma_buf) @endcode
 * auto& dev = *dev_result;
 * dev.OpenReadWrite();
 * dev.WriteBlock(0, data);
 * dev.ReadBlock(0, buffer);
 * dev.Release();
 * @endcode
 *
 * @tparam Traits 平台环境特征类型
 * @tparam TransportT 传输层类型（默认 MmioTransport）
 * @tparam VirtqueueT Virtqueue 类型（默认 SplitVirtqueue）
 * @see BlockDevice
 * @see VirtioBlk
 */
template <typename TransportT = MmioTransport,
          typename VirtqueueT = SplitVirtqueue>
class VirtioBlkDevice
    : public BlockDeviceOps<VirtioBlkDevice<TransportT, VirtqueueT>> {
 public:
  /// 底层驱动类型别名
  using DriverType = VirtioBlk<TransportT, VirtqueueT>;

  /**
   * @brief 创建并初始化 VirtIO 块设备（统一接口版）
   *
   * 内部委托 VirtioBlk::Create() 完成设备初始化。
   *
   * @param mmio_base MMIO 设备基地址
   * @param vq_dma_buf 预分配的 DMA 缓冲区虚拟地址
   * @param queue_count 期望的队列数量（当前仅支持 1）
   * @param queue_size 每个队列的描述符数量（2 的幂，默认 128）
   * @param driver_features 额外的驱动特性位
   * @return 成功返回 VirtioBlkDevice 实例，失败返回错误
   */
  [[nodiscard]] static auto Create(uint64_t mmio_base, void* vq_dma_buf,
                                   uint16_t queue_count = 1,
                                   uint32_t queue_size = 128,
                                   uint64_t driver_features = 0)
      -> Expected<VirtioBlkDevice> {
    auto blk_result = DriverType::Create(mmio_base, vq_dma_buf, queue_count,
                                         queue_size, driver_features);
    if (!blk_result) {
      return std::unexpected(blk_result.error());
    }
    return VirtioBlkDevice(std::move(*blk_result));
  }

  /**
   * @brief 获取 DMA 缓冲区所需大小
   */
  [[nodiscard]] static constexpr auto CalcDmaSize(uint16_t queue_size = 128)
      -> size_t {
    return DriverType::CalcDmaSize(queue_size);
  }

  /// @brief 直接访问底层 VirtioBlk 驱动
  [[nodiscard]] auto GetDriver() -> DriverType& { return driver_; }
  [[nodiscard]] auto GetDriver() const -> const DriverType& { return driver_; }

  /// @name 移动/拷贝控制
  /// @{
  VirtioBlkDevice(VirtioBlkDevice&&) noexcept = default;
  auto operator=(VirtioBlkDevice&&) noexcept -> VirtioBlkDevice& = default;
  VirtioBlkDevice(const VirtioBlkDevice&) = delete;
  auto operator=(const VirtioBlkDevice&) -> VirtioBlkDevice& = delete;
  ~VirtioBlkDevice() = default;
  /// @}

 protected:
  /**
   * @brief 打开设备
   */
  auto DoOpen(OpenFlags flags) -> Expected<void> {
    if (!flags.CanRead() && !flags.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    flags_ = flags;
    return {};
  }

  /**
   * @brief 释放设备
   */
  auto DoRelease() -> Expected<void> { return {}; }

  /**
   * @brief 读取多个块
   *
   * 循环调用 VirtioBlk::Read() 逐扇区读取。
   *
   * @param block_no 起始块号
   * @param buffer 目标缓冲区
   * @param block_count 块数量
   * @return 实际读取的块数
   */
  auto DoReadBlocks(uint64_t block_no, std::span<uint8_t> buffer,
                    size_t block_count) -> Expected<size_t> {
    if (!flags_.CanRead()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }

    for (size_t i = 0; i < block_count; ++i) {
      auto result = driver_.Read(block_no + i, buffer.data() + i * kSectorSize);
      if (!result) {
        if (i > 0) {
          return i;
        }
        return std::unexpected(result.error());
      }
    }
    return block_count;
  }

  /**
   * @brief 写入多个块
   *
   * 循环调用 VirtioBlk::Write() 逐扇区写入。
   *
   * @param block_no 起始块号
   * @param data 待写入数据
   * @param block_count 块数量
   * @return 实际写入的块数
   */
  auto DoWriteBlocks(uint64_t block_no, std::span<const uint8_t> data,
                     size_t block_count) -> Expected<size_t> {
    if (!flags_.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }

    for (size_t i = 0; i < block_count; ++i) {
      auto result = driver_.Write(block_no + i, data.data() + i * kSectorSize);
      if (!result) {
        if (i > 0) {
          return i;
        }
        return std::unexpected(result.error());
      }
    }
    return block_count;
  }

  /**
   * @brief 获取块大小（扇区大小 = 512 字节）
   */
  auto DoGetBlockSize() const -> size_t { return kSectorSize; }

  /**
   * @brief 获取设备总块数（扇区数）
   */
  auto DoGetBlockCount() const -> uint64_t { return driver_.GetCapacity(); }

  /**
   * @brief VirtIO 块设备中断处理（简化版）
   *
   * 确认设备中断并设置完成标志，供同步轮询使用。
   *
   * @note 可在中断上下文中安全调用（ISR-safe）
   */
  auto DoHandleInterrupt() -> void { driver_.HandleInterrupt(); }

  /**
   * @brief VirtIO 块设备中断处理（带回调版）
   *
   * 确认设备中断，遍历 Used Ring 中已完成的请求，
   * 对每个请求调用 on_complete 回调。
   *
   * @tparam CompletionCallback 签名：void(UserData token, ErrorCode status)
   * @param on_complete 完成回调函数
   */
  template <typename CompletionCallback>
  auto DoHandleInterrupt(CompletionCallback&& on_complete) -> void {
    driver_.HandleInterrupt(static_cast<CompletionCallback&&>(on_complete));
  }

 private:
  /// @brief 只能通过 Create() 工厂方法创建
  explicit VirtioBlkDevice(DriverType driver)
      : driver_(std::move(driver)), flags_{0} {}

  /// CRTP 基类需要访问 DoXxx 方法
  template <class>
  friend class ::DeviceOperationsBase;
  template <class>
  friend class ::BlockDeviceOps;

  /// 底层 VirtioBlk 驱动实例
  DriverType driver_;
  /// 打开标志
  OpenFlags flags_{0};
};

}  // namespace detail::virtio::blk

#endif /* SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_VIRTIO_DEVICE_VIRTIO_BLK_DEVICE_HPP_ \
        */
