/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#ifndef SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_UART_DEVICE_HPP_
#define SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_UART_DEVICE_HPP_

#include <concepts>
#include <cstdint>
#include <optional>
#include <span>

#include "driver/ops/char_device.hpp"
#include "expected.hpp"

namespace detail {

/**
 * @brief UART 底层驱动接口约束
 *
 * 定义 UART 底层驱动（NS16550A、PL011 等）必须满足的最小接口。
 * UartDevice 的 DriverType 模板参数必须满足此 concept。
 */
template <typename T>
concept UartDriver = requires(const T& driver, uint8_t ch) {
  { driver.PutChar(ch) } -> std::same_as<void>;
  { driver.TryGetChar() } -> std::same_as<std::optional<uint8_t>>;
  { driver.HasData() } -> std::same_as<bool>;
};

/**
 * @brief UART 字符设备通用 CRTP 中间层
 *
 * 将底层 UART 驱动（NS16550A、PL011 等）适配到统一的 CharDevice 接口。
 * 底层驱动类型 DriverType 需满足以下隐式接口：
 * - `PutChar(uint8_t) -> void`
 * - `TryGetChar() -> std::optional<uint8_t>`
 * - `HasData() -> bool`
 *
 * @tparam Derived 具体设备类型（CRTP）
 * @tparam DriverType 底层 UART 驱动类型
 */
template <class Derived, UartDriver DriverType>
class UartDevice : public CharDevice<Derived> {
 public:
  UartDevice() = default;

  /// @brief 直接访问底层驱动（用于中断处理等需要绕过 Device 框架的场景）
  auto GetDriver() -> DriverType& { return driver_; }

 protected:
  auto DoOpen(OpenFlags flags) -> Expected<void> {
    if (!flags.CanRead() && !flags.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kInvalidArgument});
    }
    flags_ = flags;
    return {};
  }

  auto DoCharRead(std::span<uint8_t> buffer) -> Expected<size_t> {
    if (!flags_.CanRead()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }
    for (size_t i = 0; i < buffer.size(); ++i) {
      auto ch = driver_.TryGetChar();
      if (!ch) {
        return i;
      }
      buffer[i] = *ch;
    }
    return buffer.size();
  }

  auto DoCharWrite(std::span<const uint8_t> data) -> Expected<size_t> {
    if (!flags_.CanWrite()) {
      return std::unexpected(Error{ErrorCode::kDevicePermissionDenied});
    }
    for (auto byte : data) {
      driver_.PutChar(byte);
    }
    return data.size();
  }

  auto DoPoll(PollEvents requested) -> Expected<PollEvents> {
    uint32_t ready = 0;
    if (requested.HasIn() && driver_.HasData()) {
      ready |= PollEvents::kIn;
    }
    if (requested.HasOut()) {
      ready |= PollEvents::kOut;
    }
    return PollEvents{ready};
  }

  auto DoRelease() -> Expected<void> { return {}; }

  /**
   * @brief UART 中断处理（简化版）
   *
   * 排空接收 FIFO 以清除 RX 中断。
   * 接收到的数据将被丢弃。如需保留数据，请使用带回调版本。
   *
   * @note 可在中断上下文中安全调用
   */
  auto DoHandleInterrupt() -> void {
    while (driver_.HasData()) {
      (void)driver_.TryGetChar();
    }
  }

  /**
   * @brief UART 中断处理（带回调版）
   *
   * 排空接收 FIFO，对每个接收到的字节调用 on_complete 回调。
   *
   * @tparam CompletionCallback 签名：void(uint8_t ch)
   * @param on_complete 每接收一个字节调用一次的回调函数
   */
  template <typename CompletionCallback>
  auto DoHandleInterrupt(CompletionCallback&& on_complete) -> void {
    while (driver_.HasData()) {
      auto ch = driver_.TryGetChar();
      if (ch) {
        on_complete(*ch);
      }
    }
  }

  /// @name 构造/析构函数
  /// @{
  ~UartDevice() = default;
  UartDevice(const UartDevice&) = delete;
  auto operator=(const UartDevice&) -> UartDevice& = delete;
  UartDevice(UartDevice&&) noexcept = default;
  auto operator=(UartDevice&&) noexcept -> UartDevice& = default;
  /// @}

  DriverType driver_;
  OpenFlags flags_{0};

 private:
  /// @brief CRTP 基类需要访问 DoXxx 方法
  template <class>
  friend class ::DeviceOperationsBase;
  template <class>
  friend class ::CharDevice;
};

}  // namespace detail

#endif  // SIMPLEKERNEL_SRC_DEVICE_INCLUDE_DRIVER_DETAIL_UART_DEVICE_HPP_
