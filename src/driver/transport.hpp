/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef VIRTIO_DRIVER_TRANSPORT_TRANSPORT_HPP_
#define VIRTIO_DRIVER_TRANSPORT_TRANSPORT_HPP_

#include "virtio_driver/defs.h"
#include "virtio_driver/expected.hpp"
#include "virtio_driver/traits.hpp"

namespace virtio_driver {

/**
 * @brief Virtio 传输层基类（零虚表开销，C++23 Deducing this）
 *
 * 利用 C++23 Deducing this（显式对象参数，P0847）实现编译期多态，
 * 消除虚表指针开销和 CRTP static_cast 样板代码，实现零开销抽象。
 * 子类（MmioTransport、PciTransport）继承此基类并提供具体的寄存器访问实现。
 *
 * 基类仅提供通用逻辑方法（Reset、NeedsReset、IsActive、AcknowledgeInterrupt），
 * 通过 Deducing this 在编译期静态分发到子类的具体实现。
 *
 * 子类应提供以下方法（隐式接口）：
 * - IsValid() const -> bool
 * - GetDeviceId() const -> uint32_t
 * - GetVendorId() const -> uint32_t
 * - GetStatus() const -> uint32_t
 * - SetStatus(uint32_t) -> void
 * - GetDeviceFeatures() -> uint64_t
 * - SetDriverFeatures(uint64_t) -> void
 * - GetQueueNumMax(uint32_t) -> uint32_t
 * - SetQueueNum(uint32_t, uint32_t) -> void
 * - SetQueueDesc(uint32_t, uint64_t) -> void
 * - SetQueueAvail(uint32_t, uint64_t) -> void
 * - SetQueueUsed(uint32_t, uint64_t) -> void
 * - GetQueueReady(uint32_t) -> bool
 * - SetQueueReady(uint32_t, bool) -> void
 * - NotifyQueue(uint32_t) -> void
 * - GetInterruptStatus() const -> uint32_t
 * - AckInterrupt(uint32_t) -> void
 * - ReadConfigU8(uint32_t) const -> uint8_t
 * - ReadConfigU16(uint32_t) const -> uint16_t
 * - ReadConfigU32(uint32_t) const -> uint32_t
 * - ReadConfigU64(uint32_t) const -> uint64_t
 * - GetConfigGeneration() const -> uint32_t
 *
 * @tparam Traits 平台环境特征类型
 * @see virtio-v1.2#4 Virtio Transport Options
 */
template <VirtioEnvironmentTraits Traits = NullTraits>
class Transport {
 public:
  /**
   * @brief 设备状态位定义
   * @see virtio-v1.2#2.1 Device Status Field
   */
  enum DeviceStatus : uint32_t {
    /// 重置状态，驱动程序将此写入以重置设备
    kReset = 0,
    /// 表示客户操作系统已找到设备并识别为有效的 virtio 设备
    kAcknowledge = 1,
    /// 表示客户操作系统知道如何驱动该设备
    kDriver = 2,
    /// 表示驱动程序已准备好驱动设备（特性协商完成）
    kDriverOk = 4,
    /// 表示驱动程序已确认设备提供的所有功能
    kFeaturesOk = 8,
    /// 表示设备需要重置
    kDeviceNeedsReset = 64,
    /// 表示在客户机中出现问题，已放弃该设备
    kFailed = 128,
  };

  /**
   * @brief 重置设备
   * @see virtio-v1.2#2.1 Device Status Field
   */
  auto Reset(this auto&& self) -> void { self.SetStatus(kReset); }

  /**
   * @brief 检查设备是否需要重置
   *
   * @return true 表示设备需要重置
   * @see virtio-v1.2#2.1 Device Status Field
   */
  [[nodiscard]] auto NeedsReset(this auto const& self) -> bool {
    return (self.GetStatus() & kDeviceNeedsReset) != 0;
  }

  /**
   * @brief 检查设备是否已激活（DRIVER_OK 已设置）
   */
  [[nodiscard]] auto IsActive(this auto const& self) -> bool {
    return (self.GetStatus() & kDriverOk) != 0;
  }

  /**
   * @brief 确认并清除设备中断
   * @see virtio-v1.2#2.3 Notifications
   */
  auto AcknowledgeInterrupt(this auto&& self) -> void {
    auto status = self.GetInterruptStatus();
    if (status != 0) {
      self.AckInterrupt(status);
    }
  }

 protected:
  /// @name 构造/析构函数（仅允许派生类使用）
  /// @{
  Transport() = default;
  ~Transport() = default;
  Transport(Transport&&) noexcept = default;
  auto operator=(Transport&&) noexcept -> Transport& = default;
  Transport(const Transport&) = delete;
  auto operator=(const Transport&) -> Transport& = delete;
  /// @}
};

}  // namespace virtio_driver

#endif /* VIRTIO_DRIVER_TRANSPORT_TRANSPORT_HPP_ */
